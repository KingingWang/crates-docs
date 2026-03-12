//! Crates Docs MCP Server main program

mod cli;

use clap::Parser;
use cli::{run, Cli};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    run(cli).await
}
