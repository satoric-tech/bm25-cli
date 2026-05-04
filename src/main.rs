mod args;
mod commands;

use anyhow::Result;
use args::{Cli, Command};
use clap::Parser;

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Some(Command::Remove { source }) => commands::remove::run(source),
        Some(Command::Sync {
            sources,
            all,
            no_ignore,
            max_filesize,
            jobs,
        }) => commands::sync::run(sources, all, no_ignore, max_filesize, jobs),
        Some(Command::List) => commands::list::run(),
        None => {
            let Some(query) = cli.query else {
                use clap::CommandFactory;
                Cli::command().print_help()?;
                println!();
                return Ok(());
            };
            commands::search::run(commands::search::RunArgs {
                query,
                paths: cli.paths,
                all: cli.all,
                limit: cli.limit,
                show_score: cli.score,
                context_chars: cli.context,
                json: cli.json,
                fuzzy: cli.fuzzy,
                since: cli.since,
                max_filesize: cli.max_filesize,
                no_ignore: cli.no_ignore,
                jobs: cli.jobs,
                force: cli.force,
            })
        }
    }
}
