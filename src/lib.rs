//! Crates Docs MCP Server
//!
//! A high-performance Rust crate documentation query MCP server, supporting multiple transport protocols and OAuth authentication.
//!
//! # Main Features
//!
//! - **Crate Documentation Query**: Fetch complete documentation for a crate from docs.rs
//! - **Crate Search**: Search Rust crates from crates.io
//! - **Item Documentation Lookup**: Find specific types, functions, or modules in a crate
//! - **Health Check**: Check server and external service status
//!
//! # Transport Protocol Support
//!
//! - `stdio`: Standard input/output (suitable for MCP client integration)
//! - `http`: HTTP transport (Streamable HTTP)
//! - `sse`: Server-Sent Events
//! - `hybrid`: Hybrid mode (HTTP + SSE)
//!
//! # Cache Support
//!
//! - **Memory Cache**: High-performance memory cache based on `moka`, supporting `TinyLFU` eviction strategy and per-entry TTL
//! - **Redis Cache**: Supports distributed deployment (requires `cache-redis` feature)
//!
//! # Example
//!
//! ```rust,no_run
//! use crates_docs::{AppConfig, CratesDocsServer};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create server with default configuration
//!     let config = AppConfig::default();
//!     let server = CratesDocsServer::new(config)?;
//!
//!     // Run HTTP server
//!     server.run_http().await?;
//!
//!     Ok(())
//! }
//! ```

#![warn(missing_docs)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::missing_panics_doc)]

pub mod cache;
pub mod cli;
pub mod config;
pub mod config_reload;
pub mod error;
pub mod metrics;
pub mod server;
pub mod tools;
pub mod utils;

pub use crate::config::{
    AppConfig, EnvAppConfig, EnvLoggingConfig, EnvServerConfig, LoggingConfig, PerformanceConfig,
    ServerConfig,
};
/// Re-export error types
pub use crate::error::{Error, Result};
/// Re-export server types
pub use crate::server::CratesDocsServer;

/// Server version
///
/// Obtained from `CARGO_PKG_VERSION` environment variable
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Server name
pub const NAME: &str = "crates-docs";

/// Initialize logging system with configuration
///
/// # Errors
/// Returns an error if logging system initialization fails
pub fn init_logging_with_config(config: &crate::config::LoggingConfig) -> Result<()> {
    use tracing_subscriber::{fmt, prelude::*, EnvFilter};

    /// Helper macro to create fmt layer with standard configuration
    macro_rules! fmt_layer {
        () => {
            fmt::layer()
                .with_target(true)
                .with_thread_ids(true)
                .with_thread_names(true)
                .compact()
        };
        ($writer:expr) => {
            fmt::layer()
                .with_writer($writer)
                .with_target(true)
                .with_thread_ids(true)
                .with_thread_names(true)
                .compact()
        };
    }

    /// Helper macro to initialize subscriber with error handling
    macro_rules! try_init {
        ($subscriber:expr) => {
            $subscriber
                .try_init()
                .map_err(|e| error::Error::initialization("logging", e.to_string()))?
        };
    }

    // Parse log level
    let level = config.level.to_lowercase();
    let level = match level.as_str() {
        "trace" | "debug" | "warn" | "error" => level.clone(),
        _ => "info".to_string(),
    };

    let filter = EnvFilter::new(level);

    // Build log layers based on configuration
    match (config.enable_console, config.enable_file, &config.file_path) {
        (true, true, Some(file_path)) => {
            // Enable both console and file logging
            let (log_dir, log_file_name) = parse_log_path(file_path);
            ensure_log_directory(&log_dir)?;
            let file_appender = tracing_appender::rolling::daily(&log_dir, log_file_name);

            try_init!(tracing_subscriber::registry()
                .with(filter)
                .with(fmt_layer!())
                .with(fmt_layer!(file_appender)));
        }

        (false, true, Some(file_path)) => {
            // Enable file logging only
            let (log_dir, log_file_name) = parse_log_path(file_path);
            ensure_log_directory(&log_dir)?;
            let file_appender = tracing_appender::rolling::daily(&log_dir, log_file_name);

            try_init!(tracing_subscriber::registry()
                .with(filter)
                .with(fmt_layer!(file_appender)));
        }

        // Default: console logging (covers all other cases)
        _ => {
            try_init!(tracing_subscriber::registry()
                .with(filter)
                .with(fmt_layer!()));
        }
    }

    Ok(())
}

/// Parse log file path into directory and file name components
fn parse_log_path(file_path: &str) -> (std::path::PathBuf, std::ffi::OsString) {
    let path = std::path::Path::new(file_path);
    let log_dir = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map_or_else(|| std::path::PathBuf::from("."), std::path::PathBuf::from);
    let log_file_name = path.file_name().map_or_else(
        || std::ffi::OsString::from("crates-docs.log"),
        std::ffi::OsString::from,
    );
    (log_dir, log_file_name)
}

/// Ensure log directory exists
fn ensure_log_directory(log_dir: &std::path::Path) -> Result<()> {
    std::fs::create_dir_all(log_dir).map_err(|e| {
        error::Error::initialization("log_directory", format!("Failed to create: {e}"))
    })
}
