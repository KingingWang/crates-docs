//! Crates Docs MCP Server main program

use clap::Parser;
use crates_docs::cli::{run, Cli};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    run(cli).await
}
