//! Crates Docs MCP Server
//!
//! A high-performance Rust crate documentation query MCP server with support for multiple transport protocols and OAuth authentication.

#![warn(missing_docs)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::missing_panics_doc)]

pub mod cache;
pub mod config;
pub mod error;
pub mod server;
pub mod tools;
pub mod utils;

/// Re-export common types
pub use crate::error::{Error, Result};
pub use crate::server::{CratesDocsServer, ServerConfig};

/// Server version information
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Server name
pub const NAME: &str = "crates-docs";

/// Initialize the logging system (simple version using boolean parameter)
///
/// # Errors
/// Returns an error if logging system initialization fails
#[deprecated(note = "Please use init_logging_with_config instead")]
pub fn init_logging(debug: bool) -> Result<()> {
    use tracing_subscriber::{fmt, prelude::*, EnvFilter};

    let filter = if debug {
        EnvFilter::new("debug")
    } else {
        EnvFilter::new("info")
    };

    let fmt_layer = fmt::layer()
        .with_target(true)
        .with_thread_ids(true)
        .with_thread_names(true)
        .compact();

    tracing_subscriber::registry()
        .with(filter)
        .with(fmt_layer)
        .try_init()
        .map_err(|e| error::Error::Initialization(e.to_string()))?;

    Ok(())
}

/// Initialize logging system with configuration
///
/// # Errors
/// Returns an error if logging system initialization fails
pub fn init_logging_with_config(config: &crate::config::LoggingConfig) -> Result<()> {
    use tracing_subscriber::{fmt, prelude::*, EnvFilter};

    // Parse log level
    let level = match config.level.to_lowercase().as_str() {
        "trace" => "trace",
        "debug" => "debug",
        "warn" => "warn",
        "error" => "error",
        _ => "info",
    };

    let filter = EnvFilter::new(level);

    // Build log layers based on configuration
    match (config.enable_console, config.enable_file, &config.file_path) {
        // Enable both console and file logging
        (true, true, Some(file_path)) => {
            // Determine log directory
            let log_dir = std::path::Path::new(file_path)
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or_else(|| std::path::Path::new("."));
            let log_file_name = std::path::Path::new(file_path)
                .file_name()
                .unwrap_or(std::ffi::OsStr::new("crates-docs.log"));

            // Ensure directory exists
            std::fs::create_dir_all(log_dir).map_err(|e| {
                error::Error::Initialization(format!("Failed to create log directory: {e}"))
            })?;

            // Create file log layer
            let file_appender = tracing_appender::rolling::daily(log_dir, log_file_name);

            tracing_subscriber::registry()
                .with(filter)
                .with(
                    fmt::layer()
                        .with_target(true)
                        .with_thread_ids(true)
                        .with_thread_names(true)
                        .compact(),
                )
                .with(
                    fmt::layer()
                        .with_writer(file_appender)
                        .with_target(true)
                        .with_thread_ids(true)
                        .with_thread_names(true)
                        .compact(),
                )
                .try_init()
                .map_err(|e| error::Error::Initialization(e.to_string()))?;
        }

        // Enable console logging only
        (true, _, _) | (false, false, _) => {
            tracing_subscriber::registry()
                .with(filter)
                .with(
                    fmt::layer()
                        .with_target(true)
                        .with_thread_ids(true)
                        .with_thread_names(true)
                        .compact(),
                )
                .try_init()
                .map_err(|e| error::Error::Initialization(e.to_string()))?;
        }

        // Enable file logging only
        (false, true, Some(file_path)) => {
            // Determine log directory
            let log_dir = std::path::Path::new(file_path)
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or_else(|| std::path::Path::new("."));
            let log_file_name = std::path::Path::new(file_path)
                .file_name()
                .unwrap_or(std::ffi::OsStr::new("crates-docs.log"));

            // Ensure directory exists
            std::fs::create_dir_all(log_dir).map_err(|e| {
                error::Error::Initialization(format!("Failed to create log directory: {e}"))
            })?;

            // Create file log layer
            let file_appender = tracing_appender::rolling::daily(log_dir, log_file_name);

            tracing_subscriber::registry()
                .with(filter)
                .with(
                    fmt::layer()
                        .with_writer(file_appender)
                        .with_target(true)
                        .with_thread_ids(true)
                        .with_thread_names(true)
                        .compact(),
                )
                .try_init()
                .map_err(|e| error::Error::Initialization(e.to_string()))?;
        }

        // Other cases, use default console logging
        _ => {
            tracing_subscriber::registry()
                .with(filter)
                .with(
                    fmt::layer()
                        .with_target(true)
                        .with_thread_ids(true)
                        .with_thread_names(true)
                        .compact(),
                )
                .try_init()
                .map_err(|e| error::Error::Initialization(e.to_string()))?;
        }
    }

    Ok(())
}
