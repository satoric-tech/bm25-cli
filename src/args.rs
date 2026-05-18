use clap::Parser;

#[derive(Debug, Parser)]
#[command(name = "bm25")]
#[command(about = "BM25 search over stdin, a file, a directory, or a URL")]
#[command(long_about = "Rank results by BM25 relevance for QUERY.\n\nWith no URI, reads from standard input and ranks paragraphs.\nPass a file to search its paragraphs, a directory to rank files,\nor an http/https URL to fetch and search that page.\nUse --html to extract article content from piped HTML before searching.")]
#[command(version)]
pub struct Cli {
    /// Search query
    pub query: String,

    /// File path, directory, or URL to search
    #[arg(value_name = "URI")]
    pub path_or_url: Option<String>,

    // ── Search ────────────────────────────────────────────────────────────

    /// Tokenizer: simple (default), whitespace, raw
    #[arg(long, value_name = "NAME", default_value = "simple", help_heading = "Search")]
    pub tokenizer: String,

    /// Filters to apply: stem, ascii-fold, remove-long (comma-separated or repeated)
    #[arg(long, value_name = "FILTER", value_delimiter = ',', help_heading = "Search")]
    pub filter: Vec<String>,

    /// Stemmer language for the stem filter (e.g. english, french, german)
    #[arg(long, value_name = "LANG", help_heading = "Search")]
    pub lang: Option<String>,

    // ── Chunking ──────────────────────────────────────────────────────────

    /// Min chunk size in chars — skip shorter chunks
    #[arg(long, value_name = "N", default_value = "64", help_heading = "Chunking")]
    pub min: usize,

    /// Max chunk size in chars — re-split on newline above this
    #[arg(long, value_name = "N", default_value = "2048", help_heading = "Chunking")]
    pub max: usize,

    // ── Input ─────────────────────────────────────────────────────────────

    /// Treat piped stdin as HTML: extract article content before searching
    #[arg(long, help_heading = "Input")]
    pub html: bool,

    /// Do not respect .gitignore / .ignore rules (directory mode only)
    #[arg(long, help_heading = "Input")]
    pub no_ignore: bool,

    // ── Output ────────────────────────────────────────────────────────────

    /// Stop after NUM results
    #[arg(short = 'm', long = "max-count", value_name = "NUM", default_value = "25", help_heading = "Output")]
    pub limit: usize,

    /// Output results as JSON lines
    #[arg(long, help_heading = "Output")]
    pub json: bool,
}
