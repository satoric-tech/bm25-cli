use anyhow::{Context, Result};
use std::path::PathBuf;
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::{IndexRecordOption, SchemaBuilder, TextFieldIndexing, TextOptions, Value, STORED};
use tantivy::{Index, IndexWriter, TantivyDocument};

use bm25_cli::tokenizer::{self, Filter, TOKENIZER_NAME};
use tantivy::tokenizer::Language;

pub struct RunArgs {
    pub query: String,
    pub path: PathBuf,
    pub limit: usize,
    pub no_ignore: bool,
    pub json: bool,
    pub tokenizer: String,
    pub filters: Vec<Filter>,
    pub lang: Option<Language>,
}

pub fn run(args: RunArgs) -> Result<()> {
    let mut builder = SchemaBuilder::new();
    let field_path = builder.add_text_field("path", STORED);
    let content_options = TextOptions::default().set_indexing_options(
        TextFieldIndexing::default()
            .set_tokenizer(TOKENIZER_NAME)
            .set_index_option(IndexRecordOption::WithFreqs),
    );
    let field_content = builder.add_text_field("content", content_options);
    let schema = builder.build();

    let index = Index::create_in_ram(schema);
    index.tokenizers().register(TOKENIZER_NAME, tokenizer::build(&args.tokenizer, &args.filters, args.lang));

    let mut writer: IndexWriter = index.writer(16_000_000)?;

    let walker = ignore::WalkBuilder::new(&args.path)
        .hidden(true)
        .git_ignore(!args.no_ignore)
        .ignore(!args.no_ignore)
        .require_git(false)
        .build();

    for entry in walker {
        let entry = entry.context("walk error")?;
        if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
            continue;
        }
        let file_path = entry.path();
        let path_str = file_path.to_string_lossy().into_owned();
        let bytes = match std::fs::read(file_path) {
            Ok(b) => b,
            Err(_) => continue,
        };
        if is_binary(&bytes) {
            continue;
        }
        let content = String::from_utf8_lossy(&bytes).into_owned();
        if content.trim().is_empty() {
            continue;
        }
        let mut doc = TantivyDocument::default();
        doc.add_text(field_path, &path_str);
        doc.add_text(field_content, &content);
        writer.add_document(doc).context("failed to add document")?;
    }
    writer.commit()?;

    let reader = index.reader()?;
    let searcher = reader.searcher();

    let query_parser = QueryParser::for_index(&index, vec![field_content]);
    let query = query_parser
        .parse_query(&args.query)
        .with_context(|| format!("failed to parse query: {}", args.query))?;

    let candidates = searcher
        .search(&query, &TopDocs::with_limit(args.limit).order_by_score())
        .context("search failed")?;

    for (score, doc_addr) in candidates {
        let doc: TantivyDocument = searcher.doc(doc_addr).context("failed to retrieve doc")?;
        let path = doc
            .get_first(field_path)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        if args.json {
            println!(
                r#"{{"score":{score:.4},"path":{}}}"#,
                serde_json::to_string(&path)?
            );
        } else {
            println!("{score:.4}  {path}");
        }
    }

    Ok(())
}

fn is_binary(bytes: &[u8]) -> bool {
    bytes[..bytes.len().min(8192)].contains(&0u8)
}
