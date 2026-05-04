use anyhow::{Context, Result};
use std::collections::HashSet;
use std::ops::Bound;
use std::path::PathBuf;
use std::time::SystemTime;
use tantivy::collector::TopDocs;
use tantivy::query::{BooleanQuery, EnableScoring, Occur, QueryParser, RangeQuery};
use tantivy::schema::Value;
use tantivy::snippet::SnippetGenerator;
use tantivy::{Executor, TantivyDocument, Term};

use bm25_cli::indexer::{canonicalize_source, glob_base_dir, is_glob, num_cpus, IndexOptions, Indexer};
use bm25_cli::sources;

type SourceFilter = Box<dyn Fn(&str) -> bool + Send + Sync>;

pub struct RunArgs {
    pub query: String,
    pub paths: Vec<String>,
    pub all: bool,
    pub limit: usize,
    pub show_score: bool,
    pub context_chars: Option<usize>,
    pub json: bool,
    pub fuzzy: Option<u8>,
    pub since: Option<String>,
    pub max_filesize: Option<u64>,
    pub no_ignore: bool,
    pub jobs: i32,
    pub force: bool,
}

fn parse_since(s: &str) -> Result<u64> {
    let now = SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .context("system time error")?
        .as_secs();

    if let Some(n) = s.strip_suffix('d') {
        let days: u64 = n
            .parse()
            .with_context(|| format!("invalid --since value: {s}"))?;
        return Ok(now.saturating_sub(days * 86_400));
    }
    if let Some(n) = s.strip_suffix('h') {
        let hours: u64 = n
            .parse()
            .with_context(|| format!("invalid --since value: {s}"))?;
        return Ok(now.saturating_sub(hours * 3_600));
    }
    if let Some(n) = s.strip_suffix('w') {
        let weeks: u64 = n
            .parse()
            .with_context(|| format!("invalid --since value: {s}"))?;
        return Ok(now.saturating_sub(weeks * 604_800));
    }

    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() == 3 {
        if let (Ok(y), Ok(m), Ok(d)) = (
            parts[0].parse::<i64>(),
            parts[1].parse::<u64>(),
            parts[2].parse::<u64>(),
        ) {
            let days_since_epoch = days_from_epoch(y, m, d);
            return Ok((days_since_epoch * 86_400) as u64);
        }
    }

    anyhow::bail!("invalid --since value: {s}. Use e.g. 7d, 24h, 2w, or 2024-01-01")
}

fn days_from_epoch(year: i64, month: u64, day: u64) -> i64 {
    let m = month as i64;
    let d = day as i64;
    let y = if m <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn render_snippet(snippet: &tantivy::snippet::Snippet) -> String {
    let fragment = snippet.fragment();
    let mut out = String::from("  ...");
    let mut last = 0;
    for range in snippet.highlighted() {
        out.push_str(&fragment[last..range.start]);
        out.push_str("\x1b[1m");
        out.push_str(&fragment[range.start..range.end]);
        out.push_str("\x1b[0m");
        last = range.end;
    }
    out.push_str(&fragment[last..]);
    out.push_str("...");
    out
}

fn is_covered(registered: &[sources::Source], path: &str) -> bool {
    if is_glob(path) {
        return false;
    }
    let Ok(abs) = PathBuf::from(path).canonicalize() else {
        return false;
    };
    registered.iter().any(|s| {
        if is_glob(&s.uri) {
            return false;
        }
        PathBuf::from(&s.uri)
            .canonicalize()
            .map(|p| abs.starts_with(&p))
            .unwrap_or(false)
    })
}

fn resolve_source(path_str: &str) -> Result<(SourceFilter, Option<String>)> {
    if is_glob(path_str) {
        let matches: HashSet<String> = glob::glob(path_str)
            .with_context(|| format!("invalid glob pattern: {path_str}"))?
            .filter_map(|e| e.ok())
            .filter(|p| p.is_file())
            .map(|p| p.canonicalize().unwrap_or(p))
            .map(|p| p.to_string_lossy().into_owned())
            .collect();
        Ok((Box::new(move |p: &str| matches.contains(p)), None))
    } else {
        let root = PathBuf::from(path_str)
            .canonicalize()
            .with_context(|| format!("path not found: {path_str}"))?;
        let root_str = root.to_string_lossy().into_owned();
        let prefix = root_str.clone();
        Ok((
            Box::new(move |p: &str| p.starts_with(&root_str)),
            Some(prefix),
        ))
    }
}

pub fn run(args: RunArgs) -> Result<()> {
    let RunArgs {
        query,
        paths,
        all,
        limit,
        show_score,
        context_chars,
        json,
        fuzzy,
        since,
        max_filesize,
        no_ignore,
        jobs,
        force,
    } = args;
    anyhow::ensure!(
        !(fuzzy.is_some() && context_chars.is_some()),
        "--fuzzy and --context cannot be used together"
    );

    let num_threads = if jobs <= 0 { num_cpus() } else { jobs as usize };
    rayon::ThreadPoolBuilder::new()
        .num_threads(num_threads)
        .build_global()
        .ok();

    let since_cutoff: Option<u64> = since.as_deref().map(parse_since).transpose()?;

    let opts = IndexOptions {
        no_ignore,
        max_filesize,
        num_threads,
        force_recrawl: force,
    };

    let paths: Vec<String> = if all {
        let tmp_indexer = Indexer::new()?;
        let registered = sources::load(&tmp_indexer.idx.sources_path)?;
        if registered.is_empty() {
            anyhow::bail!("no registered sources — run a query first to register one");
        }
        registered.into_iter().map(|s| s.uri).collect()
    } else if paths.is_empty() {
        vec![canonicalize_source(".")]
    } else {
        paths.iter().map(|p| canonicalize_source(p)).collect()
    };

    let mut indexer = Indexer::new()?;
    for path in &paths {
        indexer.index_source(path, &opts)?;
    }
    indexer.commit()?;

    let now = SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let sources_path = indexer.idx.sources_path.clone();
    let mut registered = sources::load(&sources_path)?;
    let mut changed = false;
    for path in &paths {
        let uri = if is_glob(path) {
            canonicalize_source(&glob_base_dir(path))
        } else {
            canonicalize_source(path)
        };
        if is_covered(&registered, &uri) {
            continue;
        }
        registered.retain(|s| s.uri != uri);
        registered.push(sources::Source {
            uri,
            added_at: now,
            last_synced: now,
            no_ignore,
            max_filesize,
        });
        changed = true;
    }
    if changed {
        sources::save(&sources_path, &registered)?;
    }

    let mut result_filters: Vec<SourceFilter> = Vec::new();
    let mut path_prefixes: Vec<String> = Vec::new();

    for path_str in &paths {
        let (filter, prefix) = resolve_source(path_str)?;
        result_filters.push(filter);
        if let Some(p) = prefix {
            path_prefixes.push(p);
        }
    }

    let reader = indexer
        .tantivy_index
        .reader()
        .context("failed to open reader")?;
    let searcher = reader.searcher();
    let idx = &indexer.idx;

    let mut query_parser = QueryParser::for_index(&indexer.tantivy_index, vec![idx.field_content]);
    if let Some(distance) = fuzzy {
        anyhow::ensure!(
            distance <= 2,
            "--fuzzy distance must be 1 or 2, got {distance}"
        );
        query_parser.set_field_fuzzy(idx.field_content, false, distance, true);
    }
    let text_query = query_parser
        .parse_query(&query)
        .with_context(|| format!("failed to parse query: {query}"))?;

    let mut must_clauses: Vec<(Occur, Box<dyn tantivy::query::Query>)> =
        vec![(Occur::Must, text_query)];

    if let Some(cutoff) = since_cutoff {
        let range = RangeQuery::new(
            Bound::Included(Term::from_field_u64(idx.field_mtime, cutoff)),
            Bound::Unbounded,
        );
        must_clauses.push((Occur::Must, Box::new(range)));
    }

    if !path_prefixes.is_empty() {
        let should_clauses: Vec<(Occur, Box<dyn tantivy::query::Query>)> = path_prefixes
            .iter()
            .map(|prefix| {
                let lo = Term::from_field_text(idx.field_path, prefix.as_str());
                let hi = Term::from_field_text(idx.field_path, &format!("{prefix}\u{FFFF}"));
                let range = RangeQuery::new(Bound::Included(lo), Bound::Excluded(hi));
                (
                    Occur::Should,
                    Box::new(range) as Box<dyn tantivy::query::Query>,
                )
            })
            .collect();
        must_clauses.push((Occur::Must, Box::new(BooleanQuery::new(should_clauses))));
    }

    let final_query: Box<dyn tantivy::query::Query> = if must_clauses.len() == 1 {
        must_clauses.remove(0).1
    } else {
        Box::new(BooleanQuery::new(must_clauses))
    };

    let mut context_gen = if let Some(chars) = context_chars {
        let mut gen = SnippetGenerator::create(&searcher, final_query.as_ref(), idx.field_content)
            .context("failed to create context generator")?;
        gen.set_max_num_chars(chars);
        Some(gen)
    } else {
        None
    };

    let executor = if num_threads > 1 {
        Executor::multi_thread(num_threads, "bm25-search-").context("failed to create executor")?
    } else {
        Executor::single_thread()
    };

    let top_docs = searcher
        .search_with_executor(
            final_query.as_ref(),
            &TopDocs::with_limit(limit * 10).order_by_score(),
            &executor,
            EnableScoring::enabled_from_searcher(&searcher),
        )
        .context("search failed")?;

    struct ResultEntry {
        path: String,
        score: f32,
        doc: TantivyDocument,
    }

    let mut results: Vec<ResultEntry> = Vec::new();
    for (score, doc_address) in top_docs {
        let doc: TantivyDocument = searcher
            .doc(doc_address)
            .context("failed to retrieve document")?;
        let path_val = doc
            .get_first(idx.field_path)
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        if result_filters.iter().any(|f| f(&path_val)) {
            results.push(ResultEntry {
                path: path_val,
                score,
                doc,
            });
        }
    }

    for entry in results.into_iter().take(limit) {
        let score = entry.score;
        let path_val = entry.path;
        let context = context_gen
            .as_mut()
            .map(|gen| gen.snippet_from_doc(&entry.doc));

        if json {
            let mut obj = format!(r#"{{"path":{path_val:?},"score":{score:.4}"#);
            if let Some(ref s) = context {
                if !s.is_empty() {
                    obj.push_str(&format!(r#","context":{:?}"#, s.fragment()));
                }
            }
            obj.push('}');
            println!("{obj}");
        } else {
            if show_score {
                println!("{path_val:<60} {score:.4}");
            } else {
                println!("{path_val}");
            }
            if let Some(ref s) = context {
                if !s.is_empty() {
                    println!("{}", render_snippet(s));
                }
            }
        }
    }

    Ok(())
}
