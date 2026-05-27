mod cli;
mod commands;
mod output;
mod text_intake;

use clap::Parser;

fn main() -> anyhow::Result<()> {
    commands::run(cli::Cli::parse())
}
