use clap::{Parser, Subcommand};

pub fn parse_filesize(s: &str) -> Result<u64, String> {
    let s = s.trim();
    let (num, shift) = if let Some(n) = s.strip_suffix("GB").or_else(|| s.strip_suffix('G')) {
        (n, 30)
    } else if let Some(n) = s.strip_suffix("MB").or_else(|| s.strip_suffix('M')) {
        (n, 20)
    } else if let Some(n) = s.strip_suffix("KB").or_else(|| s.strip_suffix('K')) {
        (n, 10)
    } else {
        (s, 0)
    };
    num.trim()
        .parse::<u64>()
        .map(|n| n << shift)
        .map_err(|_| format!("invalid size '{s}' — use bytes or a suffix: 10K, 5M, 2G"))
}

#[derive(Debug, Parser)]
#[command(name = "bm25")]
#[command(about = "BM25 full-text search over local files")]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,

    /// Search query
    pub query: Option<String>,

    /// Paths or globs to search
    #[arg(num_args = 0..)]
    pub paths: Vec<String>,

    /// Search across all registered sources
    #[arg(long)]
    pub all: bool,

    /// Force re-indexing of all sources
    #[arg(long)]
    pub force: bool,

    /// Show relevance scores
    #[arg(long)]
    pub score: bool,

    /// Output results as JSON lines ({path, score, context?})
    #[arg(long)]
    pub json: bool,

    /// Do not respect .gitignore / .ignore rules
    #[arg(long)]
    pub no_ignore: bool,

    /// Maximum number of results
    #[arg(short = 'l', long, default_value = "25")]
    pub limit: usize,

    /// Show a highlighted excerpt of matching content (width in chars)
    #[arg(short = 'c', long, value_name = "CHARS")]
    pub context: Option<usize>,

    /// Enable fuzzy matching at edit distance 1 or 2
    #[arg(short = 'f', long, value_name = "DISTANCE")]
    pub fuzzy: Option<u8>,

    /// Only show files modified within this window (e.g. 7d, 24h, 2w, 2024-01-01)
    #[arg(short = 's', long, value_name = "WHEN")]
    pub since: Option<String>,

    /// Skip files larger than this size (e.g. 1M, 500K, 2G)
    #[arg(short = 'm', long, value_name = "SIZE", value_parser = parse_filesize)]
    pub max_filesize: Option<u64>,

    /// Number of threads (-1 for all CPUs)
    #[arg(
        short = 'j',
        long,
        value_name = "N",
        default_value = "-1",
        allow_negative_numbers = true
    )]
    pub jobs: i32,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Remove a source and purge its documents from the index
    Remove {
        /// Path to remove
        source: String,
    },
    /// Index or re-index sources
    Sync {
        /// Paths or globs to index
        #[arg(num_args = 0..)]
        sources: Vec<String>,

        /// Sync all registered sources
        #[arg(long)]
        all: bool,

        /// Do not respect .gitignore / .ignore rules
        #[arg(long)]
        no_ignore: bool,

        /// Skip files larger than this size (e.g. 1M, 500K, 2G)
        #[arg(short = 'm', long, value_name = "SIZE", value_parser = parse_filesize)]
        max_filesize: Option<u64>,

        /// Number of threads (-1 for all CPUs)
        #[arg(
            short = 'j',
            long,
            value_name = "N",
            default_value = "-1",
            allow_negative_numbers = true
        )]
        jobs: i32,
    },
    /// List registered sources
    List,
}
