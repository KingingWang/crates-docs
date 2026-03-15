//! CLI module
//!
//! Command-line interface for the Crates Docs MCP Server.

mod commands;
mod config_cmd;
mod health_cmd;
mod serve_cmd;
mod test_cmd;
mod version_cmd;

use clap::Parser;
use std::path::PathBuf;

pub use commands::Commands;
pub use config_cmd::run_config_command;
pub use health_cmd::run_health_command;
pub use serve_cmd::run_serve_command;
pub use test_cmd::run_test_command;
pub use version_cmd::run_version_command;

/// CLI configuration
#[derive(Parser)]
#[command(name = "crates-docs")]
#[command(version = env!("CARGO_PKG_VERSION"))]
#[command(about = "High-performance Rust crate documentation query MCP server", long_about = None)]
pub struct Cli {
    /// CLI command to execute
    #[command(subcommand)]
    pub command: Commands,

    /// Configuration file path
    #[arg(short, long, global = true, default_value = "config.toml")]
    pub config: PathBuf,

    /// Enable debug logging
    #[arg(short, long, global = true)]
    pub debug: bool,

    /// Enable verbose output
    #[arg(short, long, global = true)]
    pub verbose: bool,
}

/// Run the CLI application
pub async fn run(cli: Cli) -> Result<(), Box<dyn std::error::Error>> {
    match cli.command {
        Commands::Serve {
            mode,
            host,
            port,
            enable_oauth,
            oauth_client_id,
            oauth_client_secret,
            oauth_redirect_uri,
        } => {
            run_serve_command(
                &cli.config,
                cli.debug,
                mode,
                host,
                port,
                enable_oauth,
                oauth_client_id,
                oauth_client_secret,
                oauth_redirect_uri,
            )
            .await?;
        }
        Commands::Config { output, force } => {
            run_config_command(&output, force)?;
        }
        Commands::Test {
            tool,
            crate_name,
            item_path,
            query,
            version,
            limit,
            format,
        } => {
            run_test_command(
                &tool,
                crate_name.as_deref(),
                item_path.as_deref(),
                query.as_deref(),
                version.as_deref(),
                limit,
                &format,
            )
            .await?;
        }
        Commands::Health {
            check_type,
            verbose,
        } => {
            run_health_command(&check_type, verbose).await?;
        }
        Commands::Version => {
            run_version_command();
        }
    }

    Ok(())
}
