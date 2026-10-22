use std::process::ExitCode;

use clap::Parser;
use kitty_markdown_helper::cli::{Cli, Command};

fn main() -> ExitCode {
    let cli = Cli::parse();
    let command = cli.command.unwrap_or(Command::Serve { cache_dir: None });
    let name = match command {
        Command::Serve { .. } => "serve",
        Command::Render(_) => "render",
        Command::Compare { .. } => "compare",
        Command::Measure { .. } => "measure",
    };
    eprintln!("kitty-markdown-helper: `{name}` is not implemented yet");
    ExitCode::from(2)
}
