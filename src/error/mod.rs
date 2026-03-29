//! Error handling module
//!
//! Defines application error types and result type alias.
//!
//! # Error Types
//!
//! Provides various error variants covering initialization, configuration, HTTP requests, cache, etc.
//!
//! # Example
//!
//! ```rust
//! use crates_docs::error::{Error, Result};
//!
//! fn may_fail() -> Result<String> {
//!     // Operation that may fail
//!     Ok("success".to_string())
//! }
//!
//! // Create structured error
//! fn create_config_error() -> Error {
//!     Error::config("field_name", "invalid value")
//! }
//!
//! fn create_cache_error() -> Error {
//!     Error::cache("set", Some("key".to_string()), "operation failed")
//! }
//! ```

use thiserror::Error;

/// Application error type
///
/// Contains all possible error variants, implements `std::error::Error` via `thiserror` derive macro.
#[derive(Error, Debug)]
pub enum Error {
    /// HTTP request error
    #[error("HTTP request failed: {method} {url} - status {status}: {message}")]
    HttpRequest {
        /// HTTP method
        method: String,
        /// Request URL
        url: String,
        /// HTTP status code
        status: u16,
        /// Error message
        message: String,
    },

    /// Cache operation error
    #[error("Cache operation '{operation}' failed for key '{key}': {message}")]
    Cache {
        /// Operation type ("get", "set", "delete", "clear")
        operation: String,
        /// Cache key
        key: String,
        /// Error message
        message: String,
    },

    /// MCP protocol error
    #[error("MCP protocol error in '{context}': {message}")]
    Mcp {
        /// Context where error occurred
        context: String,
        /// Error message
        message: String,
    },

    /// Initialization error
    #[error("Initialization failed for '{component}': {message}")]
    Initialization {
        /// Component that failed initialization
        component: String,
        /// Error message
        message: String,
    },

    /// Configuration error
    #[error("Configuration error for '{field}': {message}")]
    Config {
        /// Configuration field name
        field: String,
        /// Error message
        message: String,
    },

    /// Parse error
    #[error("Parse failed for '{input}'{position}: {message}")]
    Parse {
        /// Input source being parsed
        input: String,
        /// Position information
        position: String,
        /// Error message
        message: String,
    },

    /// Authentication error
    #[error("Authentication failed for '{provider}': {message}")]
    Auth {
        /// Authentication provider
        provider: String,
        /// Error message
        message: String,
    },

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON serialization/deserialization error
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// URL parse error
    #[error("URL parse error: {0}")]
    Url(#[from] url::ParseError),

    /// Reqwest HTTP client error
    #[error("HTTP client error: {0}")]
    Reqwest(#[from] reqwest::Error),

    /// Other error
    #[error("Unknown error: {0}")]
    Other(String),
}

/// Result type alias
///
/// `Result<T>` is shorthand for `std::result::Result<T, Error>`.
pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    /// Create HTTP request error
    ///
    /// # Arguments
    ///
    /// * `method` - HTTP method (GET, POST, etc.)
    /// * `url` - Request URL
    /// * `status` - HTTP status code
    /// * `message` - Error message
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

    /// Create cache operation error
    ///
    /// # Arguments
    ///
    /// * `operation` - Operation type ("get", "set", "delete", "clear")
    /// * `key` - Related cache key (optional)
    /// * `message` - Error message
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

    /// Create MCP protocol error
    ///
    /// # Arguments
    ///
    /// * `context` - Context where error occurred
    /// * `message` - Error message
    #[must_use]
    pub fn mcp(context: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Mcp {
            context: context.into(),
            message: message.into(),
        }
    }

    /// Create initialization error
    ///
    /// # Arguments
    ///
    /// * `component` - Component that failed initialization
    /// * `message` - Error message
    #[must_use]
    pub fn initialization(component: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Initialization {
            component: component.into(),
            message: message.into(),
        }
    }

    /// Create configuration error
    ///
    /// # Arguments
    ///
    /// * `field` - Configuration field name
    /// * `message` - Error message
    #[must_use]
    pub fn config(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Config {
            field: field.into(),
            message: message.into(),
        }
    }

    /// Create parse error
    ///
    /// # Arguments
    ///
    /// * `input` - Input source being parsed
    /// * `position` - Error position (optional)
    /// * `message` - Error message
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

    /// Create authentication error
    ///
    /// # Arguments
    ///
    /// * `provider` - Authentication provider
    /// * `message` - Error message
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
