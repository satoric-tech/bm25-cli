use anyhow::Result;
use std::ops::Bound;
use std::time::SystemTime;
use tantivy::collector::Count;
use tantivy::query::RangeQuery;
use tantivy::Term;

use bm25_cli::indexer::{canonicalize_source, glob_base_dir, is_glob, num_cpus, IndexOptions, Indexer};
use bm25_cli::sources;

pub fn run(
    sources: Vec<String>,
    _all: bool,
    no_ignore: bool,
    max_filesize: Option<u64>,
    jobs: i32,
) -> Result<()> {
    let num_threads = if jobs <= 0 { num_cpus() } else { jobs as usize };
    rayon::ThreadPoolBuilder::new()
        .num_threads(num_threads)
        .build_global()
        .ok();

    let mut indexer = Indexer::new()?;
    let sources_path = indexer.idx.sources_path.clone();
    let mut registered = sources::load(&sources_path)?;

    let now = SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let targets: Vec<String> = if !sources.is_empty() {
        sources.iter().map(|s| canonicalize_source(s)).collect()
    } else if _all {
        if registered.is_empty() {
            anyhow::bail!(
                "no registered sources — pass a source to index it, e.g. bm25 sync ./src"
            );
        }
        registered.iter().map(|s| s.uri.clone()).collect()
    } else {
        anyhow::bail!(
            "no sources specified — pass sources to index or use --all to re-index everything"
        );
    };

    let mut failures: Vec<(String, anyhow::Error)> = Vec::new();
    let mut succeeded: Vec<String> = Vec::new();

    for uri in &targets {
        eprintln!("Syncing {uri}…");

        let existing = registered.iter().find(|s| &s.uri == uri);
        let opts = IndexOptions {
            no_ignore: existing.map(|s| s.no_ignore).unwrap_or(no_ignore),
            max_filesize: existing.and_then(|s| s.max_filesize).or(max_filesize),
            num_threads,
            force_recrawl: true,
        };

        match indexer.index_source(uri, &opts) {
            Ok(()) => {
                if let Some(s) = registered.iter_mut().find(|s| &s.uri == uri) {
                    s.last_synced = now;
                } else {
                    let reg_uri = if is_glob(uri) {
                        canonicalize_source(&glob_base_dir(uri))
                    } else {
                        uri.clone()
                    };
                    let covered = !is_glob(uri)
                        && registered.iter().any(|s| {
                            std::path::PathBuf::from(&reg_uri).starts_with(&s.uri)
                        });
                    if !covered {
                        registered.retain(|s| s.uri != reg_uri);
                        registered.push(sources::Source {
                            uri: reg_uri,
                            added_at: now,
                            last_synced: now,
                            no_ignore,
                            max_filesize,
                        });
                    }
                }
                succeeded.push(uri.clone());
            }
            Err(e) => {
                eprintln!("  error: {e:#}");
                failures.push((uri.clone(), e));
            }
        }
    }

    indexer.commit()?;
    sources::save(&sources_path, &registered)?;

    if !succeeded.is_empty() {
        let reader = indexer
            .tantivy_index
            .reader()
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        let searcher = reader.searcher();
        let field = indexer.idx.field_path;

        for uri in &succeeded {
            let lo = Term::from_field_text(field, uri.as_str());
            let hi = Term::from_field_text(field, &format!("{uri}\u{FFFF}"));
            let range = RangeQuery::new(Bound::Included(lo), Bound::Excluded(hi));
            let count = searcher.search(&range, &Count).unwrap_or(0);
            eprintln!("  {count} docs");
        }
    }

    if !failures.is_empty() {
        eprintln!("\n{} error(s):", failures.len());
        for (uri, _) in &failures {
            eprintln!("  failed: {uri}");
        }
        anyhow::bail!("sync completed with errors");
    }

    Ok(())
}
