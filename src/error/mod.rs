//! Error handling module

use thiserror::Error;

/// Application error types
#[derive(Error, Debug)]
pub enum Error {
    /// Initialization error
    #[error("Initialization failed: {0}")]
    Initialization(String),

    /// Configuration error
    #[error("Configuration error: {0}")]
    Config(String),

    /// HTTP request error
    #[error("HTTP request failed: {0}")]
    HttpRequest(String),

    /// Parse error
    #[error("Parse failed: {0}")]
    Parse(String),

    /// Cache error
    #[error("Cache operation failed: {0}")]
    Cache(String),

    /// Authentication error
    #[error("Authentication failed: {0}")]
    Auth(String),

    /// MCP protocol error
    #[error("MCP protocol error: {0}")]
    Mcp(String),

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON serialization/deserialization error
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// URL parse error
    #[error("URL parse error: {0}")]
    Url(#[from] url::ParseError),

    /// Reqwest error
    #[error("HTTP client error: {0}")]
    Reqwest(#[from] reqwest::Error),

    /// Other error
    #[error("Unknown error: {0}")]
    Other(String),
}

/// Result type alias
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
