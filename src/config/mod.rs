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

// HTTP Client defaults

/// Default HTTP client connection pool size (10 connections)
const DEFAULT_HTTP_CLIENT_POOL_SIZE: usize = 10;
/// Default HTTP client pool idle timeout in seconds (90 seconds)
const DEFAULT_HTTP_CLIENT_POOL_IDLE_TIMEOUT_SECS: u64 = 90;
/// Default HTTP client connection timeout in seconds (10 seconds)
const DEFAULT_HTTP_CLIENT_CONNECT_TIMEOUT_SECS: u64 = 10;
/// Default HTTP client request timeout in seconds (30 seconds)
const DEFAULT_HTTP_CLIENT_TIMEOUT_SECS: u64 = 30;
/// Default HTTP client read timeout in seconds (30 seconds)
const DEFAULT_HTTP_CLIENT_READ_TIMEOUT_SECS: u64 = 30;
/// Default HTTP client max retry attempts (3 retries)
const DEFAULT_HTTP_CLIENT_MAX_RETRIES: u32 = 3;
/// Default HTTP client retry initial delay in milliseconds (100ms)
const DEFAULT_HTTP_CLIENT_RETRY_INITIAL_DELAY_MS: u64 = 100;
/// Default HTTP client retry max delay in milliseconds (10 seconds)
const DEFAULT_HTTP_CLIENT_RETRY_MAX_DELAY_MS: u64 = 10_000;

// Server defaults

/// Default server port (8080)
const DEFAULT_SERVER_PORT: u16 = 8080;
/// Default server max concurrent connections (100 connections)
const DEFAULT_SERVER_MAX_CONNECTIONS: usize = 100;
/// Default request timeout in seconds (30 seconds)
const DEFAULT_REQUEST_TIMEOUT_SECS: u64 = 30;
/// Default response timeout in seconds (60 seconds)
const DEFAULT_RESPONSE_TIMEOUT_SECS: u64 = 60;

// Cache/Rate limit defaults

/// Default cache max size in number of entries (1000 entries)
const DEFAULT_CACHE_MAX_SIZE: usize = 1000;
/// Default cache TTL in seconds (1 hour = 3600 seconds)
const DEFAULT_CACHE_DEFAULT_TTL_SECS: u64 = 3600;
/// Default rate limit per second (100 requests)
const DEFAULT_RATE_LIMIT_PER_SECOND: u32 = 100;
/// Default concurrent request limit (50 requests)
const DEFAULT_CONCURRENT_REQUEST_LIMIT: usize = 50;

// File upload defaults

/// Default max log file size in MB (100 MB)
const DEFAULT_MAX_FILE_SIZE_MB: u64 = 100;
/// Default number of log files to retain (10 files)
const DEFAULT_MAX_FILES: usize = 10;

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
    #[serde(default)]
    pub server: ServerConfig,

    /// Cache configuration
    #[serde(default)]
    pub cache: CacheConfig,

    /// Authentication configuration (OAuth and API Key)
    #[serde(default)]
    pub auth: AuthConfig,

    /// OAuth configuration (backwards compatible, prefer using auth.oauth)
    #[serde(default)]
    pub oauth: OAuthConfig,

    /// Logging configuration
    #[serde(default)]
    pub logging: LoggingConfig,

    /// Performance configuration
    #[serde(default)]
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
    #[serde(default = "default_server_name")]
    pub name: String,

    /// Server version
    #[serde(default = "default_version")]
    pub version: String,

    /// Server description
    #[serde(default = "default_server_description")]
    pub description: Option<String>,

    /// Server icons
    #[serde(default = "default_icons")]
    pub icons: Vec<Icon>,

    /// Website URL
    #[serde(default = "default_server_website_url")]
    pub website_url: Option<String>,

    /// Host address
    #[serde(default = "default_server_host")]
    pub host: String,

    /// Port
    #[serde(default = "default_server_port")]
    pub port: u16,

    /// Transport mode
    #[serde(default = "default_server_transport_mode")]
    pub transport_mode: String,

    /// Enable SSE support
    #[serde(default = "default_server_enable_sse")]
    pub enable_sse: bool,

    /// Enable OAuth authentication
    #[serde(default = "default_server_enable_oauth")]
    pub enable_oauth: bool,

    /// Maximum concurrent connections
    #[serde(default = "default_server_max_connections")]
    pub max_connections: usize,

    /// Request timeout (seconds)
    #[serde(default = "default_server_request_timeout_secs")]
    pub request_timeout_secs: u64,

    /// Response timeout (seconds)
    #[serde(default = "default_server_response_timeout_secs")]
    pub response_timeout_secs: u64,

    /// Allowed hosts for CORS (e.g., `["localhost", "127.0.0.1"]`)
    #[serde(default = "default_server_allowed_hosts")]
    pub allowed_hosts: Vec<String>,

    /// Allowed origins for CORS (e.g., `["http://localhost:*"]`)
    /// Use `"*"` only in development, specify exact origins in production
    #[serde(default = "default_server_allowed_origins")]
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

// --- Per-field default helpers (single source of truth: the struct Default impls) ---
fn default_server_name() -> String {
    ServerConfig::default().name
}

fn default_server_description() -> Option<String> {
    ServerConfig::default().description
}

fn default_server_website_url() -> Option<String> {
    ServerConfig::default().website_url
}

fn default_server_host() -> String {
    ServerConfig::default().host
}

fn default_server_port() -> u16 {
    ServerConfig::default().port
}

fn default_server_transport_mode() -> String {
    ServerConfig::default().transport_mode
}

fn default_server_enable_sse() -> bool {
    ServerConfig::default().enable_sse
}

fn default_server_enable_oauth() -> bool {
    ServerConfig::default().enable_oauth
}

fn default_server_max_connections() -> usize {
    ServerConfig::default().max_connections
}

fn default_server_request_timeout_secs() -> u64 {
    ServerConfig::default().request_timeout_secs
}

fn default_server_response_timeout_secs() -> u64 {
    ServerConfig::default().response_timeout_secs
}

fn default_server_allowed_hosts() -> Vec<String> {
    ServerConfig::default().allowed_hosts
}

fn default_server_allowed_origins() -> Vec<String> {
    ServerConfig::default().allowed_origins
}
fn default_logging_level() -> String {
    LoggingConfig::default().level
}

fn default_logging_file_path() -> Option<String> {
    LoggingConfig::default().file_path
}

fn default_logging_enable_console() -> bool {
    LoggingConfig::default().enable_console
}

fn default_logging_enable_file() -> bool {
    LoggingConfig::default().enable_file
}

fn default_logging_max_file_size_mb() -> u64 {
    LoggingConfig::default().max_file_size_mb
}

fn default_logging_max_files() -> usize {
    LoggingConfig::default().max_files
}
fn default_perf_http_client_pool_size() -> usize {
    PerformanceConfig::default().http_client_pool_size
}

fn default_perf_http_client_pool_idle_timeout_secs() -> u64 {
    PerformanceConfig::default().http_client_pool_idle_timeout_secs
}

fn default_perf_http_client_connect_timeout_secs() -> u64 {
    PerformanceConfig::default().http_client_connect_timeout_secs
}

fn default_perf_http_client_timeout_secs() -> u64 {
    PerformanceConfig::default().http_client_timeout_secs
}

fn default_perf_http_client_read_timeout_secs() -> u64 {
    PerformanceConfig::default().http_client_read_timeout_secs
}

fn default_perf_http_client_max_retries() -> u32 {
    PerformanceConfig::default().http_client_max_retries
}

fn default_perf_http_client_retry_initial_delay_ms() -> u64 {
    PerformanceConfig::default().http_client_retry_initial_delay_ms
}

fn default_perf_http_client_retry_max_delay_ms() -> u64 {
    PerformanceConfig::default().http_client_retry_max_delay_ms
}

fn default_perf_cache_max_size() -> usize {
    PerformanceConfig::default().cache_max_size
}

fn default_perf_cache_default_ttl_secs() -> u64 {
    PerformanceConfig::default().cache_default_ttl_secs
}

fn default_perf_rate_limit_per_second() -> u32 {
    PerformanceConfig::default().rate_limit_per_second
}

fn default_perf_concurrent_request_limit() -> usize {
    PerformanceConfig::default().concurrent_request_limit
}

fn default_perf_enable_response_compression() -> bool {
    PerformanceConfig::default().enable_response_compression
}

fn default_perf_enable_metrics() -> bool {
    PerformanceConfig::default().enable_metrics
}

fn default_perf_metrics_port() -> u16 {
    PerformanceConfig::default().metrics_port
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
    #[serde(default = "default_logging_level")]
    pub level: String,

    /// Log file path
    #[serde(default = "default_logging_file_path")]
    pub file_path: Option<String>,

    /// Whether to enable console logging
    #[serde(default = "default_logging_enable_console")]
    pub enable_console: bool,

    /// Whether to enable file logging
    #[serde(default = "default_logging_enable_file")]
    pub enable_file: bool,

    /// Maximum log file size (MB)
    #[serde(default = "default_logging_max_file_size_mb")]
    pub max_file_size_mb: u64,

    /// Number of log files to retain
    #[serde(default = "default_logging_max_files")]
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
    #[serde(default = "default_perf_http_client_pool_size")]
    pub http_client_pool_size: usize,

    /// HTTP client pool idle timeout (seconds)
    #[serde(default = "default_perf_http_client_pool_idle_timeout_secs")]
    pub http_client_pool_idle_timeout_secs: u64,

    /// HTTP client connection timeout (seconds)
    #[serde(default = "default_perf_http_client_connect_timeout_secs")]
    pub http_client_connect_timeout_secs: u64,

    /// HTTP client request timeout (seconds)
    #[serde(default = "default_perf_http_client_timeout_secs")]
    pub http_client_timeout_secs: u64,

    /// HTTP client read timeout (seconds)
    #[serde(default = "default_perf_http_client_read_timeout_secs")]
    pub http_client_read_timeout_secs: u64,

    /// HTTP client max retry attempts
    #[serde(default = "default_perf_http_client_max_retries")]
    pub http_client_max_retries: u32,

    /// HTTP client retry initial delay (milliseconds)
    #[serde(default = "default_perf_http_client_retry_initial_delay_ms")]
    pub http_client_retry_initial_delay_ms: u64,

    /// HTTP client retry max delay (milliseconds)
    #[serde(default = "default_perf_http_client_retry_max_delay_ms")]
    pub http_client_retry_max_delay_ms: u64,

    /// Maximum cache size (number of entries)
    #[serde(default = "default_perf_cache_max_size")]
    pub cache_max_size: usize,

    /// Default cache TTL (seconds)
    #[serde(default = "default_perf_cache_default_ttl_secs")]
    pub cache_default_ttl_secs: u64,

    /// Request rate limit (requests per second)
    #[serde(default = "default_perf_rate_limit_per_second")]
    pub rate_limit_per_second: u32,

    /// Concurrent request limit
    #[serde(default = "default_perf_concurrent_request_limit")]
    pub concurrent_request_limit: usize,

    /// Enable response compression
    #[serde(default = "default_perf_enable_response_compression")]
    pub enable_response_compression: bool,

    /// Enable Prometheus metrics
    #[serde(default = "default_perf_enable_metrics")]
    pub enable_metrics: bool,

    /// Metrics endpoint port (0 = use server port)
    #[serde(default = "default_perf_metrics_port")]
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
            port: DEFAULT_SERVER_PORT,
            transport_mode: "hybrid".to_string(),
            enable_sse: true,
            enable_oauth: false,
            max_connections: DEFAULT_SERVER_MAX_CONNECTIONS,
            request_timeout_secs: DEFAULT_REQUEST_TIMEOUT_SECS,
            response_timeout_secs: DEFAULT_RESPONSE_TIMEOUT_SECS,
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
            max_file_size_mb: DEFAULT_MAX_FILE_SIZE_MB,
            max_files: DEFAULT_MAX_FILES,
        }
    }
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            http_client_pool_size: DEFAULT_HTTP_CLIENT_POOL_SIZE,
            http_client_pool_idle_timeout_secs: DEFAULT_HTTP_CLIENT_POOL_IDLE_TIMEOUT_SECS,
            http_client_connect_timeout_secs: DEFAULT_HTTP_CLIENT_CONNECT_TIMEOUT_SECS,
            http_client_timeout_secs: DEFAULT_HTTP_CLIENT_TIMEOUT_SECS,
            http_client_read_timeout_secs: DEFAULT_HTTP_CLIENT_READ_TIMEOUT_SECS,
            http_client_max_retries: DEFAULT_HTTP_CLIENT_MAX_RETRIES,
            http_client_retry_initial_delay_ms: DEFAULT_HTTP_CLIENT_RETRY_INITIAL_DELAY_MS,
            http_client_retry_max_delay_ms: DEFAULT_HTTP_CLIENT_RETRY_MAX_DELAY_MS,
            cache_max_size: DEFAULT_CACHE_MAX_SIZE,
            cache_default_ttl_secs: DEFAULT_CACHE_DEFAULT_TTL_SECS,
            rate_limit_per_second: DEFAULT_RATE_LIMIT_PER_SECOND,
            concurrent_request_limit: DEFAULT_CONCURRENT_REQUEST_LIMIT,
            enable_response_compression: true,
            enable_metrics: true,
            metrics_port: 0,
        }
    }
}

/// Environment variable configuration for server
///
/// All fields are `Option<T>` to distinguish between "not set from environment"
/// and "explicitly set from environment".
///
/// # Semantics
///
/// - `None` - The environment variable was not set; use the config file or default value
/// - `Some(value)` - The environment variable was explicitly set to `value`
///
/// # Example
///
/// ```rust,ignore
/// // CRATES_DOCS_HOST not set
/// let config = EnvServerConfig::from_env(); // host == None, use default
///
/// // CRATES_DOCS_HOST=127.0.0.1
/// let config = EnvServerConfig::from_env(); // host == Some("127.0.0.1")
/// ```
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
/// All fields are `Option<T>` to distinguish between "not set from environment"
/// and "explicitly set from environment".
///
/// # Semantics
///
/// - `None` - The environment variable was not set; use the config file or default value
/// - `Some(value)` - The environment variable was explicitly set to `value`
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
/// All fields are `Option<T>` to distinguish between "not set from environment"
/// and "explicitly set from environment".
///
/// # Semantics
///
/// - `None` - The environment variable was not set; use the config file or default value
/// - `Some(value)` - The environment variable was explicitly set to `value`
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

        // Validate cache configuration.
        //
        // Note: the live in-memory cache is sized from `cache.memory_size`
        // (see `create_cache`), NOT `performance.cache_max_size`. A
        // `memory_size` of 0 builds a zero-capacity cache that evicts every
        // entry immediately, silently disabling caching, so reject it here.
        let valid_cache_types = ["memory", "redis"];
        if !valid_cache_types.contains(&self.cache.cache_type.as_str()) {
            return Err(crate::error::Error::config(
                "cache.cache_type",
                format!(
                    "Invalid cache type: {}, valid values: {:?}",
                    self.cache.cache_type, valid_cache_types
                ),
            ));
        }
        if self.cache.cache_type == "memory" && self.cache.memory_size == Some(0) {
            return Err(crate::error::Error::config(
                "cache.memory_size",
                "cannot be 0 (this would disable the cache); omit it to use the default",
            ));
        }

        // Validate OAuth configuration
        if self.server.enable_oauth {
            self.oauth.validate()?;
        }

        // Validate the unified auth configuration (OAuth + API key). Each
        // sub-validator short-circuits when its section is disabled, so this is
        // safe to call unconditionally and catches misconfigured API key
        // settings (e.g. empty header_name/key_prefix) that were previously
        // never validated.
        self.auth.validate()?;

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
