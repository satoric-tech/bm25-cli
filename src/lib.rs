pub mod index;
pub mod indexer;
pub mod sources;
pub mod tokenizer;

use anyhow::{Context, Result};
use std::collections::{HashMap, HashSet};
use std::ops::Bound;
use std::path::Path;
use tantivy::collector::TopDocs;
use tantivy::query::{BooleanQuery, Occur, QueryParser, RangeQuery};
use tantivy::schema::Value;
use tantivy::{Index, IndexWriter, TantivyDocument, Term};

use index::{file_mtime, BM25Index};

pub fn read_content(path: &Path) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    if bytes[..bytes.len().min(8192)].contains(&0u8) {
        return None;
    }
    if let Ok(s) = std::str::from_utf8(&bytes) {
        return Some(s.to_owned());
    }
    if let Some((enc, bom_len)) = encoding_rs::Encoding::for_bom(&bytes) {
        let (decoded, _, _) = enc.decode(&bytes[bom_len..]);
        return Some(decoded.into_owned());
    }
    let mut detector = chardetng::EncodingDetector::new();
    detector.feed(&bytes, true);
    let enc = detector.guess(None, true);
    let (decoded, _, _) = enc.decode(&bytes);
    Some(decoded.into_owned())
}

const SKIP_DIRS: &[&str] = &["target", "node_modules"];

pub fn index_directory(
    idx: &BM25Index,
    _tantivy_index: &Index,
    writer: &mut IndexWriter,
    mtimes: &mut HashMap<String, u64>,
    root: &Path,
) -> Result<bool> {
    let root_str = root.to_string_lossy().into_owned();
    let mut found: HashSet<String> = HashSet::new();
    let mut changed = false;

    let walker = ignore::WalkBuilder::new(root)
        .hidden(false)
        .require_git(false)
        .filter_entry(|e| {
            if e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                let name = e.file_name().to_string_lossy();
                !SKIP_DIRS.contains(&name.as_ref())
            } else {
                true
            }
        })
        .build();

    for entry in walker {
        let entry = entry.context("walk error")?;
        if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
            continue;
        }

        let file_path = entry.path();
        let abs_str = file_path.to_string_lossy().into_owned();
        let mtime = match file_mtime(file_path) {
            Some(m) => m,
            None => continue,
        };

        found.insert(abs_str.clone());

        if mtimes.get(&abs_str).copied() == Some(mtime) {
            continue;
        }

        let content = match read_content(file_path) {
            Some(s) if !s.trim().is_empty() => s,
            _ => continue,
        };

        if mtimes.contains_key(&abs_str) {
            writer.delete_term(Term::from_field_text(idx.field_path, &abs_str));
        }

        let mut doc = TantivyDocument::default();
        doc.add_text(idx.field_path, &abs_str);
        doc.add_text(idx.field_content, &content);
        doc.add_u64(idx.field_mtime, mtime);
        writer.add_document(doc).context("failed to add document")?;

        mtimes.insert(abs_str, mtime);
        changed = true;
    }

    let stale: Vec<String> = mtimes
        .keys()
        .filter(|p| p.starts_with(&root_str) && !found.contains(*p))
        .cloned()
        .collect();

    for abs_str in stale {
        writer.delete_term(Term::from_field_text(idx.field_path, &abs_str));
        mtimes.remove(&abs_str);
        changed = true;
    }

    Ok(changed)
}

pub fn search_query(
    idx: &BM25Index,
    tantivy_index: &Index,
    query: &str,
    root: &Path,
    limit: usize,
) -> Result<Vec<String>> {
    search_query_fuzzy(idx, tantivy_index, query, root, limit, None)
}

pub fn search_query_fuzzy(
    idx: &BM25Index,
    tantivy_index: &Index,
    query: &str,
    root: &Path,
    limit: usize,
    fuzzy: Option<u8>,
) -> Result<Vec<String>> {
    search_query_full(idx, tantivy_index, query, root, limit, fuzzy, None)
}

pub fn search_query_full(
    idx: &BM25Index,
    tantivy_index: &Index,
    query: &str,
    root: &Path,
    limit: usize,
    fuzzy: Option<u8>,
    since: Option<u64>,
) -> Result<Vec<String>> {
    let root_str = root.to_string_lossy().into_owned();
    let reader = tantivy_index.reader().context("failed to open reader")?;
    let searcher = reader.searcher();

    let mut query_parser = QueryParser::for_index(tantivy_index, vec![idx.field_content]);
    if let Some(distance) = fuzzy {
        query_parser.set_field_fuzzy(idx.field_content, false, distance, true);
    }
    let text_query = query_parser
        .parse_query(query)
        .with_context(|| format!("failed to parse query: {query}"))?;

    let final_query: Box<dyn tantivy::query::Query> = if let Some(cutoff) = since {
        let range = RangeQuery::new(
            Bound::Included(Term::from_field_u64(idx.field_mtime, cutoff)),
            Bound::Unbounded,
        );
        Box::new(BooleanQuery::new(vec![
            (Occur::Must, text_query),
            (Occur::Must, Box::new(range)),
        ]))
    } else {
        text_query
    };

    let top_docs = searcher
        .search(
            final_query.as_ref(),
            &TopDocs::with_limit(limit).order_by_score(),
        )
        .context("search failed")?;

    let mut results = Vec::new();
    for (_, addr) in top_docs {
        let doc: TantivyDocument = searcher.doc(addr).context("failed to retrieve doc")?;
        let path_val = doc
            .get_first(idx.field_path)
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        if path_val.starts_with(&root_str) {
            results.push(path_val);
        }
    }

    Ok(results)
}
