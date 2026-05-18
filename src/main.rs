mod args;
mod commands;
mod html;

use anyhow::Result;
use args::Cli;
use bm25_cli::tokenizer::{parse_filter, parse_lang};
use clap::Parser;
use std::io::{IsTerminal, Read};
use std::path::Path;

fn main() -> Result<()> {
    let cli = Cli::parse();

    let filters: Vec<_> = cli
        .filter
        .iter()
        .filter_map(|s| parse_filter(s))
        .collect();
    let lang = cli.lang.as_deref().and_then(parse_lang);

    if let Some(ref target) = cli.path_or_url {
        if target.starts_with("http://") || target.starts_with("https://") {
            let input = html::fetch_and_convert(target)?;
            return commands::stdin::run(commands::stdin::RunArgs {
                query: cli.query,
                input,
                limit: cli.limit,
                json: cli.json,
                tokenizer: cli.tokenizer,
                filters,
                lang,
                min_chunk: cli.min,
                max_chunk: cli.max,
            });
        }

        let path = Path::new(target);
        if path.is_dir() {
            return commands::dir::run(commands::dir::RunArgs {
                query: cli.query,
                path: path.to_path_buf(),
                limit: cli.limit,
                no_ignore: cli.no_ignore,
                json: cli.json,
                tokenizer: cli.tokenizer,
                filters,
                lang,
            });
        }

        let input = std::fs::read_to_string(path)
            .map_err(|e| anyhow::anyhow!("failed to read '{target}': {e}"))?;
        return commands::stdin::run(commands::stdin::RunArgs {
            query: cli.query,
            input,
            limit: cli.limit,
            json: cli.json,
            tokenizer: cli.tokenizer,
            filters,
            lang,
            min_chunk: cli.min,
            max_chunk: cli.max,
        });
    }

    let stdin = std::io::stdin();
    if stdin.is_terminal() {
        use clap::CommandFactory;
        Cli::command().print_help()?;
        println!();
        return Ok(());
    }
    let mut s = String::new();
    stdin.lock().read_to_string(&mut s)?;
    let input = if cli.html {
        html::convert(&s, "https://example.com")?
    } else {
        s
    };

    commands::stdin::run(commands::stdin::RunArgs {
        query: cli.query,
        input,
        limit: cli.limit,
        json: cli.json,
        tokenizer: cli.tokenizer,
        filters,
        lang,
        min_chunk: cli.min,
        max_chunk: cli.max,
    })
}
