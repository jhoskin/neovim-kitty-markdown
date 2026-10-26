use std::process::ExitCode;

use clap::Parser;
use kitty_markdown_helper::cli::{self, Cli, Command};

fn main() -> ExitCode {
    let args = Cli::parse();
    let command = args.command.unwrap_or(Command::Serve { cache_dir: None });
    match cli::run(command) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("{}", err.stderr_line());
            ExitCode::FAILURE
        }
    }
}
