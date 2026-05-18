use anyhow::{Context, Result};
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::{
    IndexRecordOption, NumericOptions, SchemaBuilder, TextFieldIndexing, TextOptions, Value,
};
use tantivy::tokenizer::Language;
use tantivy::{Index, IndexWriter, TantivyDocument};

use bm25_cli::tokenizer::{self, Filter, TOKENIZER_NAME};

const WINDOW: usize = 256;

pub struct RunArgs {
    pub query: String,
    pub input: String,
    pub limit: usize,
    pub json: bool,
    pub tokenizer: String,
    pub filters: Vec<Filter>,
    pub lang: Option<Language>,
    pub min_chunk: usize,
    pub max_chunk: usize,
}

pub fn run(args: RunArgs) -> Result<()> {
    let windows = paragraph_chunks(&args.input, args.min_chunk, args.max_chunk);

    if windows.is_empty() {
        return Ok(());
    }

    let content_options = TextOptions::default()
        .set_indexing_options(
            TextFieldIndexing::default()
                .set_tokenizer(TOKENIZER_NAME)
                .set_index_option(IndexRecordOption::WithFreqs),
        )
        .set_stored();

    let mut builder = SchemaBuilder::new();
    let field_content = builder.add_text_field("content", content_options);
    let field_pos = builder.add_u64_field("pos", NumericOptions::default().set_stored());
    let schema = builder.build();

    let index = Index::create_in_ram(schema);
    index
        .tokenizers()
        .register(TOKENIZER_NAME, tokenizer::build(&args.tokenizer, &args.filters, args.lang));

    let mut writer: IndexWriter = index.writer(16_000_000)?;
    for (pos, text) in &windows {
        let mut doc = TantivyDocument::default();
        doc.add_text(field_content, text);
        doc.add_u64(field_pos, *pos as u64);
        writer.add_document(doc)?;
    }
    writer.commit()?;

    let reader = index.reader()?;
    let searcher = reader.searcher();

    let query_parser = QueryParser::for_index(&index, vec![field_content]);
    let query = query_parser
        .parse_query(&args.query)
        .with_context(|| format!("failed to parse query: {}", args.query))?;

    let candidates = searcher
        .search(&query, &TopDocs::with_limit(args.limit * 8).order_by_score())
        .context("search failed")?;

    let mut selected: Vec<usize> = Vec::new();
    let mut first = true;

    for (score, doc_addr) in candidates {
        if selected.len() >= args.limit {
            break;
        }

        let doc: TantivyDocument = searcher.doc(doc_addr).context("failed to retrieve doc")?;
        let pos = doc
            .get_first(field_pos)
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;

        if selected.iter().any(|&p| pos.abs_diff(p) < WINDOW) {
            continue;
        }
        selected.push(pos);

        let text = doc
            .get_first(field_content)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        if args.json {
            println!(
                r#"{{"score":{score:.4},"text":{}}}"#,
                serde_json::to_string(&text)?
            );
        } else {
            if !first {
                println!();
            }
            first = false;
            println!("score: {score:.4}");
            println!("{text}");
        }
    }

    Ok(())
}

fn paragraph_chunks(text: &str, min: usize, max: usize) -> Vec<(usize, &str)> {
    let mut result = Vec::new();
    let mut offset = 0;

    for part in text.split("\n\n") {
        let part_start = offset;
        offset += part.len() + 2;

        let trimmed = part.trim();
        if trimmed.is_empty() {
            continue;
        }

        if trimmed.len() > max {
            let mut line_pos = part_start;
            for line in part.split('\n') {
                let line_trimmed = line.trim();
                if line_trimmed.len() >= min {
                    result.push((line_pos, line_trimmed));
                }
                line_pos += line.len() + 1;
            }
        } else if trimmed.len() >= min {
            result.push((part_start, trimmed));
        }
    }

    result
}
