use anyhow::Result;

use bm25_cli::indexer::{canonicalize_source, Indexer};
use bm25_cli::sources;

pub fn run(source: String) -> Result<()> {
    let source = canonicalize_source(&source);
    let mut indexer = Indexer::new()?;
    let count = indexer.remove_source(&source)?;
    indexer.commit()?;

    let sources_path = indexer.idx.sources_path.clone();
    let mut sources = sources::load(&sources_path)?;
    let before = sources.len();
    sources.retain(|s| s.uri != source);
    if sources.len() == before {
        eprintln!("Warning: {source} was not in the registered sources list");
    }
    sources::save(&sources_path, &sources)?;
    println!("Removed {count} documents for {source}");

    Ok(())
}
