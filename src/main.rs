mod cli;
mod config;
pub mod ec;
mod hardware;
mod server;
mod tui;

use anyhow::Result;
use clap::Parser;

fn main() -> Result<()> {
    let args = cli::Cli::parse();
    cli::handle_cli(args)?;
    Ok(())
}
