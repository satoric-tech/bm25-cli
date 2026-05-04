use anyhow::{Context, Result};
use rayon::prelude::*;
use std::collections::{HashMap, HashSet};
use std::ops::Bound;
use tantivy::collector::TopDocs;
use tantivy::query::RangeQuery;
use tantivy::schema::Value;
use tantivy::{Index, IndexWriter, TantivyDocument, Term};

use crate::index::{file_mtime, BM25Index};
use crate::read_content;

const SKIP_DIRS: &[&str] = &["target", "node_modules"];
const COMMIT_BATCH: usize = 500;

pub fn num_cpus() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
}

pub struct IndexOptions {
    pub no_ignore: bool,
    pub max_filesize: Option<u64>,
    pub num_threads: usize,
    pub force_recrawl: bool,
}

impl Default for IndexOptions {
    fn default() -> Self {
        Self {
            no_ignore: false,
            max_filesize: None,
            num_threads: num_cpus(),
            force_recrawl: false,
        }
    }
}

pub struct Indexer {
    pub idx: BM25Index,
    pub tantivy_index: Index,
    writer: Option<IndexWriter>,
    pub index_locked: bool,
    pub mtimes: HashMap<String, u64>,
    changed: bool,
}

impl Indexer {
    pub fn new() -> Result<Self> {
        let idx = BM25Index::open_global()?;
        let tantivy_index = idx.open_or_create_tantivy()?;
        let mtimes = idx.load_mtimes()?;
        Ok(Self {
            idx,
            tantivy_index,
            writer: None,
            index_locked: false,
            mtimes,
            changed: false,
        })
    }

    fn try_acquire_writer(&mut self) -> Result<()> {
        if self.writer.is_none() && !self.index_locked {
            match self.tantivy_index.writer(128_000_000) {
                Ok(w) => self.writer = Some(w),
                Err(tantivy::TantivyError::LockFailure(_, _)) => self.index_locked = true,
                Err(e) => return Err(e).context("failed to create index writer"),
            }
        }
        Ok(())
    }

    pub fn index_source(&mut self, uri: &str, opts: &IndexOptions) -> Result<()> {
        if is_glob(uri) {
            let base = glob_base_dir(uri);
            self.index_directory(&base, opts)?;
        } else {
            self.index_directory(uri, opts)?;
        }
        Ok(())
    }

    fn index_directory(&mut self, path: &str, opts: &IndexOptions) -> Result<()> {
        let search_root = std::path::PathBuf::from(path)
            .canonicalize()
            .with_context(|| format!("path not found: {path}"))?;
        let root_str = search_root.to_string_lossy().into_owned();
        let num_threads = opts.num_threads;
        let no_ignore = opts.no_ignore;
        let max_filesize = opts.max_filesize;
        let force = opts.force_recrawl;

        let walker = ignore::WalkBuilder::new(&search_root)
            .hidden(false)
            .require_git(false)
            .ignore(!no_ignore)
            .git_ignore(!no_ignore)
            .git_global(!no_ignore)
            .git_exclude(!no_ignore)
            .threads(num_threads)
            .filter_entry(|e| {
                if e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    let name = e.file_name().to_string_lossy();
                    !SKIP_DIRS.contains(&name.as_ref())
                } else {
                    true
                }
            })
            .build_parallel();

        let found_mutex = std::sync::Mutex::new(HashSet::<String>::new());
        let to_index_mutex: std::sync::Mutex<Vec<(String, u64, bool)>> =
            std::sync::Mutex::new(Vec::new());
        let mtimes = &self.mtimes;

        walker.run(|| {
            Box::new(|entry| {
                let entry = match entry {
                    Ok(e) => e,
                    Err(_) => return ignore::WalkState::Continue,
                };
                if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
                    return ignore::WalkState::Continue;
                }
                let file_path = entry.path();
                let abs_str = file_path.to_string_lossy().into_owned();
                let mtime = match file_mtime(file_path) {
                    Some(m) => m,
                    None => return ignore::WalkState::Continue,
                };
                let needs_delete = mtimes.contains_key(&abs_str);
                let already_current = !force && mtimes.get(&abs_str).copied() == Some(mtime);
                found_mutex.lock().unwrap().insert(abs_str.clone());
                if !already_current {
                    to_index_mutex
                        .lock()
                        .unwrap()
                        .push((abs_str, mtime, needs_delete));
                }
                ignore::WalkState::Continue
            })
        });

        let found = found_mutex.into_inner().unwrap();
        let to_index = to_index_mutex.into_inner().unwrap();

        let read: Vec<(String, u64, bool, String)> = to_index
            .into_par_iter()
            .filter_map(|(p, mtime, del)| {
                if let Some(max) = max_filesize {
                    if std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0) > max {
                        return None;
                    }
                }
                let content = read_content(std::path::Path::new(&p))?;
                if content.trim().is_empty() {
                    return None;
                }
                Some((p, mtime, del, content))
            })
            .collect();

        self.try_acquire_writer()?;
        if self.index_locked {
            return Ok(());
        }

        let mut batch = 0usize;
        for (abs_str, mtime, needs_delete, content) in read {
            let w = self.writer.as_mut().unwrap();
            if needs_delete {
                w.delete_term(Term::from_field_text(self.idx.field_path, &abs_str));
            }
            let mut doc = TantivyDocument::default();
            doc.add_text(self.idx.field_path, &abs_str);
            doc.add_text(self.idx.field_content, &content);
            doc.add_u64(self.idx.field_mtime, mtime);
            w.add_document(doc).context("failed to add document")?;
            self.mtimes.insert(abs_str, mtime);
            self.changed = true;
            batch += 1;
            if batch % COMMIT_BATCH == 0 {
                w.commit().context("failed to flush intermediate batch")?;
            }
        }

        let stale: Vec<String> = self
            .mtimes
            .keys()
            .filter(|p| p.starts_with(&root_str) && !found.contains(*p))
            .cloned()
            .collect();

        for abs_str in stale {
            let w = self.writer.as_mut().unwrap();
            w.delete_term(Term::from_field_text(self.idx.field_path, &abs_str));
            self.mtimes.remove(&abs_str);
            self.changed = true;
        }

        Ok(())
    }

    pub fn remove_source(&mut self, uri: &str) -> Result<usize> {
        self.try_acquire_writer()?;
        if self.index_locked {
            anyhow::bail!("index is locked by another process, cannot remove source");
        }

        let paths_to_delete: Vec<String> = {
            let reader = self
                .tantivy_index
                .reader()
                .context("failed to open reader")?;
            let searcher = reader.searcher();
            let lo = Term::from_field_text(self.idx.field_path, uri);
            let hi = Term::from_field_text(self.idx.field_path, &format!("{uri}\u{FFFF}"));
            let prefix_q = RangeQuery::new(Bound::Included(lo), Bound::Excluded(hi));
            searcher
                .search(&prefix_q, &TopDocs::with_limit(1_000_000).order_by_score())
                .context("failed to search docs to remove")?
                .into_iter()
                .filter_map(|(_, addr)| {
                    let doc: TantivyDocument = searcher.doc(addr).ok()?;
                    doc.get_first(self.idx.field_path)
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                })
                .collect()
        };

        let count = paths_to_delete.len();
        let w = self.writer.as_mut().unwrap();
        for path in &paths_to_delete {
            w.delete_term(Term::from_field_text(self.idx.field_path, path));
            self.mtimes.remove(path);
        }

        if count > 0 {
            self.changed = true;
        }
        Ok(count)
    }

    pub fn commit(&mut self) -> Result<()> {
        if self.changed {
            if let Some(mut w) = self.writer.take() {
                w.commit().context("failed to commit index")?;
            }
            self.idx.save_mtimes(&self.mtimes)?;
        } else {
            self.writer.take();
        }
        Ok(())
    }
}

pub fn is_glob(s: &str) -> bool {
    s.contains('*') || s.contains('?') || s.contains('[') || s.contains('{')
}

pub fn glob_base_dir(pattern: &str) -> String {
    let first_glob = pattern
        .char_indices()
        .find(|(_, c)| matches!(c, '*' | '?' | '[' | '{'))
        .map(|(i, _)| i);
    let base = if let Some(idx) = first_glob {
        let prefix = &pattern[..idx];
        prefix.trim_end_matches('/').trim_end_matches('\\')
    } else {
        pattern
    };
    if base.is_empty() { "." } else { base }.to_string()
}

pub fn canonicalize_source(s: &str) -> String {
    if is_glob(s) {
        if std::path::Path::new(s).is_absolute() {
            return s.to_string();
        }
        let cwd = std::env::current_dir().unwrap_or_default();
        return cwd.join(s).to_string_lossy().into_owned();
    }
    std::path::PathBuf::from(s)
        .canonicalize()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| s.to_string())
}
