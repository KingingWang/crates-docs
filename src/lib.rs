//! Crates Docs MCP Server
//!
//! 一个高性能的 Rust crate 文档查询 MCP 服务器，支持多种传输协议和 OAuth 认证。

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

/// 重新导出常用类型
pub use crate::error::{Error, Result};
pub use crate::server::{CratesDocsServer, ServerConfig};

/// 服务器版本信息
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// 服务器名称
pub const NAME: &str = "crates-docs";

/// 初始化日志系统（简单版本，使用布尔参数）
///
/// # Errors
/// 如果日志系统初始化失败，返回错误
#[deprecated(note = "请使用 init_logging_with_config 代替")]
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

/// 使用配置初始化日志系统
///
/// # Errors
/// 如果日志系统初始化失败，返回错误
pub fn init_logging_with_config(config: &crate::config::LoggingConfig) -> Result<()> {
    use tracing_subscriber::{fmt, prelude::*, EnvFilter};

    // 解析日志级别
    let level = match config.level.to_lowercase().as_str() {
        "trace" => "trace",
        "debug" => "debug",
        "warn" => "warn",
        "error" => "error",
        _ => "info",
    };

    let filter = EnvFilter::new(level);

    // 根据配置构建日志层
    match (config.enable_console, config.enable_file, &config.file_path) {
        // 同时启用控制台和文件日志
        (true, true, Some(file_path)) => {
            // 确定日志目录
            let log_dir = std::path::Path::new(file_path)
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or_else(|| std::path::Path::new("."));
            let log_file_name = std::path::Path::new(file_path)
                .file_name()
                .unwrap_or(std::ffi::OsStr::new("crates-docs.log"));

            // 确保目录存在
            std::fs::create_dir_all(log_dir)
                .map_err(|e| error::Error::Initialization(format!("创建日志目录失败: {e}")))?;

            // 创建文件日志层
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

        // 只启用控制台日志
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

        // 只启用文件日志
        (false, true, Some(file_path)) => {
            // 确定日志目录
            let log_dir = std::path::Path::new(file_path)
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or_else(|| std::path::Path::new("."));
            let log_file_name = std::path::Path::new(file_path)
                .file_name()
                .unwrap_or(std::ffi::OsStr::new("crates-docs.log"));

            // 确保目录存在
            std::fs::create_dir_all(log_dir)
                .map_err(|e| error::Error::Initialization(format!("创建日志目录失败: {e}")))?;

            // 创建文件日志层
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

        // 其他情况，使用默认控制台日志
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
