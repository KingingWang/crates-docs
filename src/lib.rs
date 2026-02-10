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

/// 初始化日志系统
///
/// # Errors
/// 如果日志系统初始化失败，返回错误
pub fn init_logging(debug: bool) -> Result<()> {
    use tracing_subscriber::{EnvFilter, fmt, prelude::*};

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
