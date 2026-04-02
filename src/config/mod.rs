//! Configuration module
//!
//! Provides application configuration management, supports loading from files, environment variables, and default values.
//!
//! # Configuration Source Priority
//!
//! 1. Environment variables (highest priority)
//! 2. Configuration file
//! 3. Default values (lowest priority)
//!
//! # Supported Configuration Formats
//!
//! - TOML configuration file
//! - Environment variables (prefix `CRATES_DOCS_`)
//!
//! # Examples
//!
//! ```rust,no_run
//! use crates_docs::config::AppConfig;
//!
//! // Load configuration from file
//! let config = AppConfig::from_file("config.toml").expect("Failed to load config");
//!
//! // Load configuration from environment variables
//! let config = AppConfig::from_env().expect("Failed to load config from env");
//!
//! // Use default configuration
//! let config = AppConfig::default();
//! ```

use crate::cache::CacheConfig;
use crate::server::auth::{AuthConfig, OAuthConfig};
use rust_mcp_sdk::schema::{Icon, IconTheme};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// Application configuration
///
/// Contains server, cache, authentication, logging, and performance configuration.
///
/// # Fields
///
/// - `server`: Server configuration
/// - `cache`: Cache configuration
/// - `auth`: Authentication configuration (OAuth and API Key)
/// - `logging`: Logging configuration
/// - `performance`: Performance configuration
///
/// # Hot Reload Support
///
/// The following configuration items support hot reload (runtime update without restart):
/// - `logging` section: All fields
/// - `auth` section: All fields (including API Key and OAuth)
/// - `cache` section: TTL-related fields (`default_ttl`, `crate_docs_ttl_secs`, `item_docs_ttl_secs`, `search_results_ttl_secs`)
/// - `performance` section: `rate_limit_per_second`, `concurrent_request_limit`, `enable_metrics`, `enable_response_compression`
///
/// The following configuration items **do not** support hot reload (require server restart):
/// - `server` section: All fields (host, port, `transport_mode`, `max_connections`, etc.)
/// - `cache` section: `cache_type`, `memory_size`, `redis_url` (cache initialization parameters)
/// - `performance` section: `http_client_*`, `cache_max_size`, `cache_default_ttl_secs`, `metrics_port`
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct AppConfig {
    /// Server configuration
    pub server: ServerConfig,

    /// Cache configuration
    pub cache: CacheConfig,

    /// Authentication configuration (OAuth and API Key)
    #[serde(default)]
    pub auth: AuthConfig,

    /// OAuth configuration (backwards compatible, prefer using auth.oauth)
    #[serde(default)]
    pub oauth: OAuthConfig,

    /// Logging configuration
    pub logging: LoggingConfig,

    /// Performance configuration
    pub performance: PerformanceConfig,
}

/// Server configuration
///
/// # Hot Reload Support
///
/// ⚠️ **Does not support hot reload** - Server configuration changes require server restart to take effect.
///
/// Reason: These configurations involve server listening socket, transport layer initialization and other core parameters,
/// runtime changes may cause connection interruption or state inconsistency.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServerConfig {
    /// Server name
    pub name: String,

    /// Server version
    #[serde(default = "default_version")]
    pub version: String,

    /// Server description
    pub description: Option<String>,

    /// Server icons
    #[serde(default = "default_icons")]
    pub icons: Vec<Icon>,

    /// Website URL
    pub website_url: Option<String>,

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

    /// Allowed hosts for CORS (e.g., `["localhost", "127.0.0.1"]`)
    pub allowed_hosts: Vec<String>,

    /// Allowed origins for CORS (e.g., `["http://localhost:*"]`)
    /// Use `"*"` only in development, specify exact origins in production
    pub allowed_origins: Vec<String>,
}

/// Default server version from Cargo.toml
fn default_version() -> String {
    crate::VERSION.to_string()
}

/// Default icons for the server
fn default_icons() -> Vec<Icon> {
    vec![
        Icon {
            src: "https://docs.rs/static/favicon-32x32.png".to_string(),
            mime_type: Some("image/png".to_string()),
            sizes: vec!["32x32".to_string()],
            theme: Some(IconTheme::Light),
        },
        Icon {
            src: "https://docs.rs/static/favicon-32x32.png".to_string(),
            mime_type: Some("image/png".to_string()),
            sizes: vec!["32x32".to_string()],
            theme: Some(IconTheme::Dark),
        },
    ]
}

/// Logging configuration
///
/// # Hot Reload Support
///
/// ✅ **Supports hot reload** - All logging configuration items can be dynamically updated at runtime.
///
/// Hot reload supported fields:
/// - `level`: Log level (trace/debug/info/warn/error)
/// - `file_path`: Log file path
/// - `enable_console`: Console logging toggle
/// - `enable_file`: File logging toggle
/// - `max_file_size_mb`: Maximum log file size
/// - `max_files`: Number of log files to retain
///
/// Note: After file logging path changes, new logs will be written to the new file, but old file handles will not be automatically closed.
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
///
/// # Hot Reload Support
///
/// ## Hot reload supported fields ✅
///
/// The following fields can be dynamically updated at runtime:
/// - `rate_limit_per_second`: Request rate limit (requests per second)
/// - `concurrent_request_limit`: Concurrent request limit
/// - `enable_metrics`: Prometheus metrics collection toggle
/// - `enable_response_compression`: Response compression toggle
///
/// ## Hot reload not supported fields ❌
///
/// The following fields require server restart to take effect:
/// - `http_client_*`: HTTP client configuration (pool size, timeouts, etc.)
/// - `cache_max_size`: Cache maximum size
/// - `cache_default_ttl_secs`: Cache default TTL
/// - `metrics_port`: Metrics server port
///
/// Reason: These configurations involve underlying connection pool, cache instance initialization parameters.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PerformanceConfig {
    /// HTTP client connection pool size
    pub http_client_pool_size: usize,

    /// HTTP client pool idle timeout (seconds)
    pub http_client_pool_idle_timeout_secs: u64,

    /// HTTP client connection timeout (seconds)
    pub http_client_connect_timeout_secs: u64,

    /// HTTP client request timeout (seconds)
    pub http_client_timeout_secs: u64,

    /// HTTP client read timeout (seconds)
    pub http_client_read_timeout_secs: u64,

    /// HTTP client max retry attempts
    pub http_client_max_retries: u32,

    /// HTTP client retry initial delay (milliseconds)
    pub http_client_retry_initial_delay_ms: u64,

    /// HTTP client retry max delay (milliseconds)
    pub http_client_retry_max_delay_ms: u64,

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

    /// Enable Prometheus metrics
    pub enable_metrics: bool,

    /// Metrics endpoint port (0 = use server port)
    pub metrics_port: u16,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            name: "crates-docs".to_string(),
            version: crate::VERSION.to_string(),
            description: Some(
                "High-performance Rust crate documentation query MCP server".to_string(),
            ),
            icons: default_icons(),
            website_url: Some("https://github.com/KingingWang/crates-docs".to_string()),
            host: "127.0.0.1".to_string(),
            port: 8080,
            transport_mode: "hybrid".to_string(),
            enable_sse: true,
            enable_oauth: false,
            max_connections: 100,
            request_timeout_secs: 30,
            response_timeout_secs: 60,
            // Secure defaults: only allow localhost by default
            allowed_hosts: vec!["localhost".to_string(), "127.0.0.1".to_string()],
            allowed_origins: vec!["http://localhost:*".to_string()],
        }
    }
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            file_path: Some("./logs/crates-docs.log".to_string()),
            enable_console: true,
            enable_file: false, // Default: console output only
            max_file_size_mb: 100,
            max_files: 10,
        }
    }
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            http_client_pool_size: 10,
            http_client_pool_idle_timeout_secs: 90,
            http_client_connect_timeout_secs: 10,
            http_client_timeout_secs: 30,
            http_client_read_timeout_secs: 30,
            http_client_max_retries: 3,
            http_client_retry_initial_delay_ms: 100,
            http_client_retry_max_delay_ms: 10000,
            cache_max_size: 1000,
            cache_default_ttl_secs: 3600,
            rate_limit_per_second: 100,
            concurrent_request_limit: 50,
            enable_response_compression: true,
            enable_metrics: true,
            metrics_port: 0,
        }
    }
}

/// Environment variable configuration for server
///
/// All fields are `Option<T>` to distinguish between "not set" and "explicitly set"
#[derive(Debug, Clone, Default)]
pub struct EnvServerConfig {
    /// Server name
    pub name: Option<String>,
    /// Host address
    pub host: Option<String>,
    /// Port
    pub port: Option<u16>,
    /// Transport mode
    pub transport_mode: Option<String>,
}

/// Environment variable configuration for logging
///
/// All fields are `Option<T>` to distinguish between "not set" and "explicitly set"
#[derive(Debug, Clone, Default)]
pub struct EnvLoggingConfig {
    /// Log level
    pub level: Option<String>,
    /// Whether to enable console logging
    pub enable_console: Option<bool>,
    /// Whether to enable file logging
    pub enable_file: Option<bool>,
}

/// Environment variable configuration for API key (when feature enabled)
///
/// All fields are `Option<T>` to distinguish between "not set" and "explicitly set"
#[cfg(feature = "api-key")]
#[derive(Debug, Clone, Default)]
pub struct EnvApiKeyConfig {
    /// Whether API key authentication is enabled
    pub enabled: Option<bool>,
    /// List of valid API keys
    pub keys: Option<Vec<String>>,
    /// Header name for API key
    pub header_name: Option<String>,
    /// Query parameter name for API key
    pub query_param_name: Option<String>,
    /// Whether to allow API key in query parameters
    pub allow_query_param: Option<bool>,
    /// API key prefix
    pub key_prefix: Option<String>,
}

/// Environment variable configuration
///
/// Uses `Option<T>` for all fields to properly distinguish between
/// "not set" and "explicitly set to default value".
#[derive(Debug, Clone, Default)]
pub struct EnvAppConfig {
    /// Server configuration from environment
    pub server: EnvServerConfig,
    /// Logging configuration from environment
    pub logging: EnvLoggingConfig,
    /// API key configuration from environment
    #[cfg(feature = "api-key")]
    pub auth_api_key: EnvApiKeyConfig,
}

impl AppConfig {
    /// Load configuration from file
    ///
    /// # Errors
    ///
    /// Returns an error if file does not exist, cannot be read, or format is invalid
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, crate::error::Error> {
        let content = fs::read_to_string(path).map_err(|e| {
            crate::error::Error::config("file", format!("Failed to read config file: {e}"))
        })?;

        let config: Self = toml::from_str(&content).map_err(|e| {
            crate::error::Error::parse("config", None, format!("Failed to parse config file: {e}"))
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
            crate::error::Error::config(
                "serialization",
                format!("Failed to serialize configuration: {e}"),
            )
        })?;

        // Ensure directory exists
        if let Some(parent) = path.as_ref().parent() {
            fs::create_dir_all(parent).map_err(|e| {
                crate::error::Error::config("directory", format!("Failed to create directory: {e}"))
            })?;
        }

        fs::write(path, content).map_err(|e| {
            crate::error::Error::config("file", format!("Failed to write config file: {e}"))
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
            return Err(crate::error::Error::config("host", "cannot be empty"));
        }

        if self.server.port == 0 {
            return Err(crate::error::Error::config("port", "cannot be 0"));
        }

        if self.server.max_connections == 0 {
            return Err(crate::error::Error::config(
                "max_connections",
                "cannot be 0",
            ));
        }

        // Validate transport mode
        let valid_modes = ["stdio", "http", "sse", "hybrid"];
        if !valid_modes.contains(&self.server.transport_mode.as_str()) {
            return Err(crate::error::Error::config(
                "transport_mode",
                format!(
                    "Invalid transport mode: {}, valid values: {:?}",
                    self.server.transport_mode, valid_modes
                ),
            ));
        }

        // Validate log level
        let valid_levels = ["trace", "debug", "info", "warn", "error"];

        if !valid_levels.contains(&self.logging.level.as_str()) {
            return Err(crate::error::Error::config(
                "log_level",
                format!(
                    "Invalid log level: {}, valid values: {:?}",
                    self.logging.level, valid_levels
                ),
            ));
        }

        // Validate performance configuration
        if self.performance.http_client_pool_size == 0 {
            return Err(crate::error::Error::config(
                "http_client_pool_size",
                "cannot be 0",
            ));
        }

        if self.performance.http_client_pool_idle_timeout_secs == 0 {
            return Err(crate::error::Error::config(
                "http_client_pool_idle_timeout_secs",
                "cannot be 0",
            ));
        }

        if self.performance.http_client_connect_timeout_secs == 0 {
            return Err(crate::error::Error::config(
                "http_client_connect_timeout_secs",
                "cannot be 0",
            ));
        }

        if self.performance.http_client_timeout_secs == 0 {
            return Err(crate::error::Error::config(
                "http_client_timeout_secs",
                "cannot be 0",
            ));
        }

        if self.performance.cache_max_size == 0 {
            return Err(crate::error::Error::config("cache_max_size", "cannot be 0"));
        }

        // Validate OAuth configuration
        if self.server.enable_oauth {
            self.oauth.validate()?;
        }

        Ok(())
    }

    /// Load configuration from environment variables
    ///
    /// Returns an `EnvAppConfig` where all fields are `Option<T>`, allowing
    /// the caller to distinguish between "not set" and "explicitly set".
    ///
    /// # Errors
    ///
    /// Returns an error if environment variable format is invalid (e.g., non-numeric port)
    pub fn from_env() -> Result<EnvAppConfig, crate::error::Error> {
        let mut config = EnvAppConfig::default();

        // Load server configuration from environment variables
        if let Ok(name) = std::env::var("CRATES_DOCS_NAME") {
            config.server.name = Some(name);
        }

        if let Ok(host) = std::env::var("CRATES_DOCS_HOST") {
            config.server.host = Some(host);
        }

        if let Ok(port) = std::env::var("CRATES_DOCS_PORT") {
            config.server.port =
                Some(port.parse().map_err(|e| {
                    crate::error::Error::config("port", format!("Invalid port: {e}"))
                })?);
        }

        if let Ok(mode) = std::env::var("CRATES_DOCS_TRANSPORT_MODE") {
            config.server.transport_mode = Some(mode);
        }

        // Load logging configuration from environment variables
        if let Ok(level) = std::env::var("CRATES_DOCS_LOG_LEVEL") {
            config.logging.level = Some(level);
        }

        if let Ok(enable_console) = std::env::var("CRATES_DOCS_ENABLE_CONSOLE") {
            config.logging.enable_console = enable_console.parse().ok();
        }

        if let Ok(enable_file) = std::env::var("CRATES_DOCS_ENABLE_FILE") {
            config.logging.enable_file = enable_file.parse().ok();
        }

        #[cfg(feature = "api-key")]
        {
            if let Ok(enabled) = std::env::var("CRATES_DOCS_API_KEY_ENABLED") {
                config.auth_api_key.enabled = enabled.parse().ok();
            }

            if let Ok(keys) = std::env::var("CRATES_DOCS_API_KEYS") {
                config.auth_api_key.keys = Some(
                    keys.split(',')
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(ToOwned::to_owned)
                        .collect(),
                );
            }

            if let Ok(header_name) = std::env::var("CRATES_DOCS_API_KEY_HEADER") {
                config.auth_api_key.header_name = Some(header_name);
            }

            if let Ok(query_param_name) = std::env::var("CRATES_DOCS_API_KEY_QUERY_PARAM_NAME") {
                config.auth_api_key.query_param_name = Some(query_param_name);
            }

            if let Ok(allow_query_param) = std::env::var("CRATES_DOCS_API_KEY_ALLOW_QUERY") {
                config.auth_api_key.allow_query_param = allow_query_param.parse().ok();
            }

            if let Ok(key_prefix) = std::env::var("CRATES_DOCS_API_KEY_PREFIX") {
                config.auth_api_key.key_prefix = Some(key_prefix);
            }
        }

        Ok(config)
    }

    /// Merge configuration (environment variables take precedence over file configuration)
    ///
    /// Uses `Option<T>` semantics from `EnvAppConfig` to determine which values
    /// were explicitly set via environment variables. This eliminates fragile
    /// hardcoded default comparisons.
    #[must_use]
    pub fn merge(file_config: Option<Self>, env_config: Option<EnvAppConfig>) -> Self {
        let mut config = Self::default();

        // First apply file configuration
        if let Some(file) = file_config {
            config = file;
        }

        // Then apply environment variable configuration (overrides file configuration)
        // Uses Option::is_some() to check if value was explicitly set
        if let Some(env) = env_config {
            // Merge server configuration - only override if explicitly set
            if let Some(name) = env.server.name {
                config.server.name = name;
            }
            if let Some(host) = env.server.host {
                config.server.host = host;
            }
            if let Some(port) = env.server.port {
                config.server.port = port;
            }
            if let Some(transport_mode) = env.server.transport_mode {
                config.server.transport_mode = transport_mode;
            }

            // Merge logging configuration - only override if explicitly set
            if let Some(level) = env.logging.level {
                config.logging.level = level;
            }
            if let Some(enable_console) = env.logging.enable_console {
                config.logging.enable_console = enable_console;
            }
            if let Some(enable_file) = env.logging.enable_file {
                config.logging.enable_file = enable_file;
            }

            #[cfg(feature = "api-key")]
            {
                if let Some(enabled) = env.auth_api_key.enabled {
                    config.auth.api_key.enabled = enabled;
                }
                if let Some(keys) = env.auth_api_key.keys {
                    config.auth.api_key.keys = keys;
                }
                if let Some(header_name) = env.auth_api_key.header_name {
                    config.auth.api_key.header_name = header_name;
                }
                if let Some(query_param_name) = env.auth_api_key.query_param_name {
                    config.auth.api_key.query_param_name = query_param_name;
                }
                if let Some(allow_query_param) = env.auth_api_key.allow_query_param {
                    config.auth.api_key.allow_query_param = allow_query_param;
                }
                if let Some(key_prefix) = env.auth_api_key.key_prefix {
                    config.auth.api_key.key_prefix = key_prefix;
                }
            }
        }

        config
    }
}
