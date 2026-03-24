//! Crates Docs MCP Server
//!
//! 一个高性能的 Rust crate 文档查询 MCP 服务器，支持多种传输协议和 OAuth 认证。
//!
//! # 主要功能
//!
//! - **Crate 文档查询**: 从 docs.rs 获取 crate 的完整文档
//! - **Crate 搜索**: 从 crates.io 搜索 Rust crate
//! - **项目文档查找**: 查找 crate 中的特定类型、函数或模块
//! - **健康检查**: 检查服务器和外部服务状态
//!
//! # 传输协议支持
//!
//! - `stdio`: 标准输入输出（适合 MCP 客户端集成）
//! - `http`: HTTP 传输（Streamable HTTP）
//! - `sse`: Server-Sent Events
//! - `hybrid`: 混合模式（HTTP + SSE）
//!
//! # 缓存支持
//!
//! - **内存缓存**: 基于 `moka` 的高性能内存缓存，支持 `TinyLFU` 淘汰策略和按条目 TTL
//! - **Redis 缓存**: 支持分布式部署（需要启用 `cache-redis` feature）
//!
//! # 示例
//!
//! ```rust,no_run
//! use crates_docs::{AppConfig, CratesDocsServer};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // 使用默认配置创建服务器
//!     let config = AppConfig::default();
//!     let server = CratesDocsServer::new(config)?;
//!
//!     // 运行 HTTP 服务器
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
pub mod error;
pub mod metrics;
pub mod server;
pub mod tools;
pub mod utils;

pub use crate::config::{AppConfig, ServerConfig};
/// 重新导出错误类型
pub use crate::error::{Error, Result};
/// 重新导出服务器类型
pub use crate::server::CratesDocsServer;

/// 服务器版本号
///
/// 从 `CARGO_PKG_VERSION` 环境变量获取
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// 服务器名称
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
        .map_err(|e| error::Error::initialization("logging", e.to_string()))?;

    Ok(())
}

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
            let (log_dir, log_file_name) = parse_log_path(file_path);
            ensure_log_directory(&log_dir)?;
            let file_appender = tracing_appender::rolling::daily(&log_dir, log_file_name);

            try_init!(tracing_subscriber::registry()
                .with(filter)
                .with(fmt_layer!())
                .with(fmt_layer!(file_appender)));
        }

        // Enable console logging only
        (true, _, _) | (false, false, _) => {
            try_init!(tracing_subscriber::registry()
                .with(filter)
                .with(fmt_layer!()));
        }

        // Enable file logging only
        (false, true, Some(file_path)) => {
            let (log_dir, log_file_name) = parse_log_path(file_path);
            ensure_log_directory(&log_dir)?;
            let file_appender = tracing_appender::rolling::daily(&log_dir, log_file_name);

            try_init!(tracing_subscriber::registry()
                .with(filter)
                .with(fmt_layer!(file_appender)));
        }

        // Other cases, use default console logging
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
