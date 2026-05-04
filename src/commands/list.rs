use anyhow::Result;
use std::ops::Bound;
use std::time::SystemTime;
use tantivy::collector::Count;
use tantivy::query::RangeQuery;
use tantivy::Term;

use bm25_cli::index::BM25Index;
use bm25_cli::sources;

pub fn run() -> Result<()> {
    let idx = BM25Index::open_global()?;
    let tantivy_index = idx.open_or_create_tantivy()?;
    let reader = tantivy_index.reader()?;
    let searcher = reader.searcher();
    let sources = sources::load(&idx.sources_path)?;

    if sources.is_empty() {
        println!("No sources registered. Run a query to add one.");
        return Ok(());
    }

    for (i, source) in sources.iter().enumerate() {
        if i > 0 {
            println!();
        }

        let lo = Term::from_field_text(idx.field_path, &source.uri);
        let hi = Term::from_field_text(idx.field_path, &format!("{}\u{FFFF}", source.uri));
        let range = RangeQuery::new(Bound::Included(lo), Bound::Excluded(hi));
        let count = searcher.search(&range, &Count).unwrap_or(0);

        println!("{}", source.uri);
        println!("  {count} docs");
        println!("  added {}", age(source.added_at));
        println!("  last synced {}", age(source.last_synced));
        if source.no_ignore {
            println!("  --no-ignore");
        }
        if let Some(max) = source.max_filesize {
            println!("  --max-filesize {max}");
        }
    }

    Ok(())
}

fn age(ts: u64) -> String {
    let now = SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let secs = now.saturating_sub(ts);
    if secs < 60 {
        "just now".to_string()
    } else if secs < 3_600 {
        format!("{} min ago", secs / 60)
    } else if secs < 86_400 {
        format!("{} hours ago", secs / 3_600)
    } else {
        format!("{} days ago", secs / 86_400)
    }
}
