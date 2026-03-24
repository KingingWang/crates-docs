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
//!
//! // 创建结构化错误
//! fn create_config_error() -> Error {
//!     Error::config("field_name", "invalid value")
//! }
//!
//! fn create_cache_error() -> Error {
//!     Error::cache("set", Some("key".to_string()), "operation failed")
//! }
//! ```

use thiserror::Error;

/// 应用程序错误类型
///
/// 包含所有可能的错误变体，使用 `thiserror` 派生宏实现 `std::error::Error`。
#[derive(Error, Debug)]
pub enum Error {
    /// HTTP 请求错误
    #[error("HTTP request failed: {method} {url} - status {status}: {message}")]
    HttpRequest {
        /// HTTP 方法
        method: String,
        /// 请求 URL
        url: String,
        /// HTTP 状态码
        status: u16,
        /// 错误消息
        message: String,
    },

    /// 缓存操作错误
    #[error("Cache operation '{operation}' failed for key '{key}': {message}")]
    Cache {
        /// 操作类型 ("get", "set", "delete", "clear")
        operation: String,
        /// 缓存键
        key: String,
        /// 错误消息
        message: String,
    },

    /// MCP 协议错误
    #[error("MCP protocol error in '{context}': {message}")]
    Mcp {
        /// 错误发生的上下文
        context: String,
        /// 错误消息
        message: String,
    },

    /// 初始化错误
    #[error("Initialization failed for '{component}': {message}")]
    Initialization {
        /// 初始化失败的组件
        component: String,
        /// 错误消息
        message: String,
    },

    /// 配置错误
    #[error("Configuration error for '{field}': {message}")]
    Config {
        /// 配置字段名
        field: String,
        /// 错误消息
        message: String,
    },

    /// 解析错误
    #[error("Parse failed for '{input}'{position}: {message}")]
    Parse {
        /// 解析的输入源
        input: String,
        /// 位置信息
        position: String,
        /// 错误消息
        message: String,
    },

    /// 认证错误
    #[error("Authentication failed for '{provider}': {message}")]
    Auth {
        /// 认证提供者
        provider: String,
        /// 错误消息
        message: String,
    },

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

impl Error {
    /// 创建 HTTP 请求错误
    ///
    /// # 参数
    ///
    /// * `method` - HTTP 方法 (GET, POST, 等)
    /// * `url` - 请求的 URL
    /// * `status` - HTTP 状态码
    /// * `message` - 错误消息
    #[must_use]
    pub fn http_request(
        method: impl Into<String>,
        url: impl Into<String>,
        status: u16,
        message: impl Into<String>,
    ) -> Self {
        Self::HttpRequest {
            method: method.into(),
            url: url.into(),
            status,
            message: message.into(),
        }
    }

    /// 创建缓存操作错误
    ///
    /// # 参数
    ///
    /// * `operation` - 操作类型 ("get", "set", "delete", "clear")
    /// * `key` - 相关的缓存键（可选）
    /// * `message` - 错误消息
    #[must_use]
    pub fn cache(
        operation: impl Into<String>,
        key: Option<String>,
        message: impl Into<String>,
    ) -> Self {
        Self::Cache {
            operation: operation.into(),
            key: key.unwrap_or_else(|| "N/A".to_string()),
            message: message.into(),
        }
    }

    /// 创建 MCP 协议错误
    ///
    /// # 参数
    ///
    /// * `context` - 错误发生的上下文
    /// * `message` - 错误消息
    #[must_use]
    pub fn mcp(context: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Mcp {
            context: context.into(),
            message: message.into(),
        }
    }

    /// 创建初始化错误
    ///
    /// # 参数
    ///
    /// * `component` - 初始化失败的组件
    /// * `message` - 错误消息
    #[must_use]
    pub fn initialization(component: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Initialization {
            component: component.into(),
            message: message.into(),
        }
    }

    /// 创建配置错误
    ///
    /// # 参数
    ///
    /// * `field` - 配置字段名
    /// * `message` - 错误消息
    #[must_use]
    pub fn config(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Config {
            field: field.into(),
            message: message.into(),
        }
    }

    /// 创建解析错误
    ///
    /// # 参数
    ///
    /// * `input` - 解析的输入源
    /// * `position` - 错误位置（可选）
    /// * `message` - 错误消息
    #[must_use]
    pub fn parse(
        input: impl Into<String>,
        position: Option<usize>,
        message: impl Into<String>,
    ) -> Self {
        Self::Parse {
            input: input.into(),
            position: position.map_or_else(String::new, |p| format!(" at position {p}")),
            message: message.into(),
        }
    }

    /// 创建认证错误
    ///
    /// # 参数
    ///
    /// * `provider` - 认证提供者
    /// * `message` - 错误消息
    #[must_use]
    pub fn auth(provider: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Auth {
            provider: provider.into(),
            message: message.into(),
        }
    }
}

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
