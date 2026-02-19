//! Configuration module

use crate::cache::CacheConfig;
use crate::server::auth::OAuthConfig;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// Application configuration
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct AppConfig {
    /// Server configuration
    pub server: ServerConfig,

    /// Cache configuration
    pub cache: CacheConfig,

    /// OAuth configuration
    pub oauth: OAuthConfig,

    /// Logging configuration
    pub logging: LoggingConfig,

    /// Performance configuration
    pub performance: PerformanceConfig,
}

/// Server configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServerConfig {
    /// Server name
    pub name: String,

    /// Server version
    pub version: String,

    /// Server description
    pub description: Option<String>,

    /// Host address
    pub host: String,

    /// Port
    pub port: u16,

    /// Transport mode
    pub transport_mode: String,

    /// Enable SSE support
    pub enable_sse: bool,

    /// Enable OAuth authentication
    pub enable_oauth: bool,

    /// Maximum concurrent connections
    pub max_connections: usize,

    /// Request timeout (seconds)
    pub request_timeout_secs: u64,

    /// Response timeout (seconds)
    pub response_timeout_secs: u64,
}

/// Logging configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LoggingConfig {
    /// Log level
    pub level: String,

    /// Log file path
    pub file_path: Option<String>,

    /// Whether to enable console logging
    pub enable_console: bool,

    /// Whether to enable file logging
    pub enable_file: bool,

    /// Maximum log file size (MB)
    pub max_file_size_mb: u64,

    /// Number of log files to retain
    pub max_files: usize,
}

/// Performance configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PerformanceConfig {
    /// HTTP client connection pool size
    pub http_client_pool_size: usize,

    /// Maximum cache size (number of entries)
    pub cache_max_size: usize,

    /// Default cache TTL (seconds)
    pub cache_default_ttl_secs: u64,

    /// Request rate limit (requests per second)
    pub rate_limit_per_second: u32,

    /// Concurrent request limit
    pub concurrent_request_limit: usize,

    /// Enable response compression
    pub enable_response_compression: bool,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            name: "crates-docs".to_string(),
            version: crate::VERSION.to_string(),
            description: Some(
                "High-performance Rust crate documentation query MCP server".to_string(),
            ),
            host: "127.0.0.1".to_string(),
            port: 8080,
            transport_mode: "hybrid".to_string(),
            enable_sse: true,
            enable_oauth: false,
            max_connections: 100,
            request_timeout_secs: 30,
            response_timeout_secs: 60,
        }
    }
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            file_path: Some("./logs/crates-docs.log".to_string()),
            enable_console: true,
            enable_file: true,
            max_file_size_mb: 100,
            max_files: 10,
        }
    }
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            http_client_pool_size: 10,
            cache_max_size: 1000,
            cache_default_ttl_secs: 3600,
            rate_limit_per_second: 100,
            concurrent_request_limit: 50,
            enable_response_compression: true,
        }
    }
}

impl AppConfig {
    /// Load configuration from file
    ///
    /// # Errors
    ///
    /// Returns an error if file does not exist, cannot be read, or format is invalid
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, crate::error::Error> {
        let content = fs::read_to_string(path)
            .map_err(|e| crate::error::Error::Config(format!("Failed to read config file: {e}")))?;

        let config: Self = toml::from_str(&content).map_err(|e| {
            crate::error::Error::Config(format!("Failed to parse config file: {e}"))
        })?;

        config.validate()?;
        Ok(config)
    }

    /// Save configuration to file
    ///
    /// # Errors
    ///
    /// Returns an error if configuration cannot be serialized, directory cannot be created, or file cannot be written
    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<(), crate::error::Error> {
        let content = toml::to_string_pretty(self).map_err(|e| {
            crate::error::Error::Config(format!("Failed to serialize configuration: {e}"))
        })?;

        // Ensure directory exists
        if let Some(parent) = path.as_ref().parent() {
            fs::create_dir_all(parent).map_err(|e| {
                crate::error::Error::Config(format!("Failed to create directory: {e}"))
            })?;
        }

        fs::write(path, content).map_err(|e| {
            crate::error::Error::Config(format!("Failed to write config file: {e}"))
        })?;

        Ok(())
    }

    /// Validate configuration
    ///
    /// # Errors
    ///
    /// Returns an error if configuration is invalid (e.g., empty hostname, invalid port, etc.)
    pub fn validate(&self) -> Result<(), crate::error::Error> {
        // Validate server configuration
        if self.server.host.is_empty() {
            return Err(crate::error::Error::Config(
                "Server host cannot be empty".to_string(),
            ));
        }

        if self.server.port == 0 {
            return Err(crate::error::Error::Config(
                "Server port cannot be 0".to_string(),
            ));
        }

        if self.server.max_connections == 0 {
            return Err(crate::error::Error::Config(
                "Maximum connections cannot be 0".to_string(),
            ));
        }

        // Validate transport mode
        let valid_modes = ["stdio", "http", "sse", "hybrid"];
        if !valid_modes.contains(&self.server.transport_mode.as_str()) {
            return Err(crate::error::Error::Config(format!(
                "Invalid transport mode: {}, valid values: {:?}",
                self.server.transport_mode, valid_modes
            )));
        }

        // Validate log level
        let valid_levels = ["trace", "debug", "info", "warn", "error"];
        if !valid_levels.contains(&self.logging.level.as_str()) {
            return Err(crate::error::Error::Config(format!(
                "Invalid log level: {}, valid values: {:?}",
                self.logging.level, valid_levels
            )));
        }

        // Validate performance configuration
        if self.performance.http_client_pool_size == 0 {
            return Err(crate::error::Error::Config(
                "HTTP client connection pool size cannot be 0".to_string(),
            ));
        }

        if self.performance.cache_max_size == 0 {
            return Err(crate::error::Error::Config(
                "Maximum cache size cannot be 0".to_string(),
            ));
        }

        // Validate OAuth configuration
        if self.server.enable_oauth {
            self.oauth.validate()?;
        }

        Ok(())
    }

    /// Load configuration from environment variables
    ///
    /// # Errors
    ///
    /// Returns an error if environment variable format is invalid or configuration validation fails
    pub fn from_env() -> Result<Self, crate::error::Error> {
        let mut config = Self::default();

        // Override configuration from environment variables
        if let Ok(name) = std::env::var("CRATES_DOCS_NAME") {
            config.server.name = name;
        }

        if let Ok(host) = std::env::var("CRATES_DOCS_HOST") {
            config.server.host = host;
        }

        if let Ok(port) = std::env::var("CRATES_DOCS_PORT") {
            config.server.port = port
                .parse()
                .map_err(|e| crate::error::Error::Config(format!("Invalid port: {e}")))?;
        }

        if let Ok(mode) = std::env::var("CRATES_DOCS_TRANSPORT_MODE") {
            config.server.transport_mode = mode;
        }

        if let Ok(level) = std::env::var("CRATES_DOCS_LOG_LEVEL") {
            config.logging.level = level;
        }

        config.validate()?;
        Ok(config)
    }

    /// Merge configuration (environment variables take precedence over file configuration)
    #[must_use]
    pub fn merge(file_config: Option<Self>, env_config: Option<Self>) -> Self {
        let mut config = Self::default();

        // First apply file configuration
        if let Some(file) = file_config {
            config = file;
        }

        // Then apply environment variable configuration (overrides file configuration)
        if let Some(env) = env_config {
            // Merge server configuration
            if env.server.name != "crates-docs" {
                config.server.name = env.server.name;
            }
            if env.server.host != "127.0.0.1" {
                config.server.host = env.server.host;
            }
            if env.server.port != 8080 {
                config.server.port = env.server.port;
            }
            if env.server.transport_mode != "hybrid" {
                config.server.transport_mode = env.server.transport_mode;
            }

            // Merge logging configuration
            if env.logging.level != "info" {
                config.logging.level = env.logging.level;
            }
        }

        config
    }
}
