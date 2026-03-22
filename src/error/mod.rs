//! 错误处理模块
//!
//! 定义应用程序错误类型和结果类型别名。
//!
//! # 错误类型
//!
//! 提供多种错误变体，涵盖初始化、配置、HTTP 请求、缓存等场景。
//!
//! # 示例
//!
//! ```rust
//! use crates_docs::error::{Error, Result};
//!
//! fn may_fail() -> Result<String> {
//!     // 可能失败的操作
//!     Ok("success".to_string())
//! }
//! ```

use thiserror::Error;

/// 应用程序错误类型
///
/// 包含所有可能的错误变体，使用 `thiserror` 派生宏实现 `std::error::Error`。
#[derive(Error, Debug)]
pub enum Error {
    /// 初始化错误
    #[error("Initialization failed: {0}")]
    Initialization(String),

    /// 配置错误
    #[error("Configuration error: {0}")]
    Config(String),

    /// HTTP 请求错误
    #[error("HTTP request failed: {0}")]
    HttpRequest(String),

    /// 解析错误
    #[error("Parse failed: {0}")]
    Parse(String),

    /// 缓存错误
    #[error("Cache operation failed: {0}")]
    Cache(String),

    /// 认证错误
    #[error("Authentication failed: {0}")]
    Auth(String),

    /// MCP 协议错误
    #[error("MCP protocol error: {0}")]
    Mcp(String),

    /// IO 错误
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON 序列化/反序列化错误
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// URL 解析错误
    #[error("URL parse error: {0}")]
    Url(#[from] url::ParseError),

    /// Reqwest HTTP 客户端错误
    #[error("HTTP client error: {0}")]
    Reqwest(#[from] reqwest::Error),

    /// 其他错误
    #[error("Unknown error: {0}")]
    Other(String),
}

/// 结果类型别名
///
/// `Result<T>` 是 `std::result::Result<T, Error>` 的简写。
pub type Result<T> = std::result::Result<T, Error>;

impl From<Box<dyn std::error::Error + Send + Sync>> for Error {
    fn from(err: Box<dyn std::error::Error + Send + Sync>) -> Self {
        Error::Other(err.to_string())
    }
}

impl From<anyhow::Error> for Error {
    fn from(err: anyhow::Error) -> Self {
        Error::Other(err.to_string())
    }
}
