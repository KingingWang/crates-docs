//! 错误处理模块

use thiserror::Error;

/// 应用程序错误类型
#[derive(Error, Debug)]
pub enum Error {
    /// 初始化错误
    #[error("初始化失败: {0}")]
    Initialization(String),

    /// 配置错误
    #[error("配置错误: {0}")]
    Config(String),

    /// HTTP 请求错误
    #[error("HTTP 请求失败: {0}")]
    HttpRequest(String),

    /// 解析错误
    #[error("解析失败: {0}")]
    Parse(String),

    /// 缓存错误
    #[error("缓存操作失败: {0}")]
    Cache(String),

    /// 认证错误
    #[error("认证失败: {0}")]
    Auth(String),

    /// MCP 协议错误
    #[error("MCP 协议错误: {0}")]
    Mcp(String),

    /// IO 错误
    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),

    /// JSON 序列化/反序列化错误
    #[error("JSON 错误: {0}")]
    Json(#[from] serde_json::Error),

    /// URL 解析错误
    #[error("URL 解析错误: {0}")]
    Url(#[from] url::ParseError),

    /// Reqwest 错误
    #[error("HTTP 客户端错误: {0}")]
    Reqwest(#[from] reqwest::Error),

    /// 其他错误
    #[error("未知错误: {0}")]
    Other(String),
}

/// 结果类型别名
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
