//! 配置模块
//!
//! 提供应用程序配置管理，支持从文件加载、环境变量和默认值。
//!
//! # 配置来源优先级
//!
//! 1. 环境变量（最高优先级）
//! 2. 配置文件
//! 3. 默认值（最低优先级）
//!
//! # 支持的配置格式
//!
//! - TOML 配置文件
//! - 环境变量（前缀 `CRATES_DOCS_`）
//!
//! # 示例
//!
//! ```rust,no_run
//! use crates_docs::config::AppConfig;
//!
//! // 从文件加载配置
//! let config = AppConfig::from_file("config.toml").expect("Failed to load config");
//!
//! // 从环境变量加载配置
//! let config = AppConfig::from_env().expect("Failed to load config from env");
//!
//! // 使用默认配置
//! let config = AppConfig::default();
//! ```

use crate::cache::CacheConfig;
use crate::server::auth::{AuthConfig, OAuthConfig};
use rust_mcp_sdk::schema::{Icon, IconTheme};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// 应用程序配置
///
/// 包含服务器、缓存、认证、日志和性能配置。
///
/// # 字段
///
/// - `server`: 服务器配置
/// - `cache`: 缓存配置
/// - `auth`: 认证配置（OAuth 和 API Key）
/// - `logging`: 日志配置
/// - `performance`: 性能配置
///
/// # 热重载支持
///
/// 以下配置项支持热重载（运行时无需重启）：
/// - `logging` 部分：所有字段
/// - `auth` 部分：所有字段（包括 API Key 和 OAuth）
/// - `cache` 部分：TTL 相关字段（`default_ttl`, `crate_docs_ttl_secs`, `item_docs_ttl_secs`, `search_results_ttl_secs`）
/// - `performance` 部分：`rate_limit_per_second`, `concurrent_request_limit`, `enable_metrics`, `enable_response_compression`
///
/// 以下配置项**不支持**热重载（需要重启服务器）：
/// - `server` 部分：所有字段（host, port, `transport_mode`, `max_connections` 等）
/// - `cache` 部分：`cache_type`, `memory_size`, `redis_url`（缓存初始化参数）
/// - `performance` 部分：`http_client_*`, `cache_max_size`, `cache_default_ttl_secs`, `metrics_port`
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct AppConfig {
    /// 服务器配置
    pub server: ServerConfig,

    /// 缓存配置
    pub cache: CacheConfig,

    /// 认证配置（OAuth 和 API Key）
    #[serde(default)]
    pub auth: AuthConfig,

    /// OAuth 配置（向后兼容，优先使用 auth.oauth）
    #[serde(default)]
    pub oauth: OAuthConfig,

    /// 日志配置
    pub logging: LoggingConfig,

    /// 性能配置
    pub performance: PerformanceConfig,
}

/// 服务器配置
///
/// # 热重载支持
///
/// ⚠️ **不支持热重载** - 服务器配置项改变后需要重启服务器才能生效。
///
/// 原因：这些配置涉及服务器监听套接字、传输层初始化等核心参数，
/// 运行时更改可能导致连接中断或状态不一致。
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

/// 日志配置
///
/// # 热重载支持
///
/// ✅ **支持热重载** - 所有日志配置项都可以在运行时动态更新。
///
/// 支持热重载的字段：
/// - `level`: 日志级别（trace/debug/info/warn/error）
/// - `file_path`: 日志文件路径
/// - `enable_console`: 控制台日志开关
/// - `enable_file`: 文件日志开关
/// - `max_file_size_mb`: 日志文件最大大小
/// - `max_files`: 保留的日志文件数量
///
/// 注意：文件日志路径更改后，新日志会写入新文件，但不会自动关闭旧文件句柄。
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

/// 性能配置
///
/// # 热重载支持
///
/// ## 支持热重载的字段 ✅
///
/// 以下字段可以在运行时动态更新：
/// - `rate_limit_per_second`: 请求速率限制（每秒请求数）
/// - `concurrent_request_limit`: 并发请求限制
/// - `enable_metrics`: Prometheus 指标收集开关
/// - `enable_response_compression`: 响应压缩开关
///
/// ## 不支持热重载的字段 ❌
///
/// 以下字段需要重启服务器才能生效：
/// - `http_client_*`: HTTP 客户端配置（连接池大小、超时等）
/// - `cache_max_size`: 缓存最大大小
/// - `cache_default_ttl_secs`: 缓存默认 TTL
/// - `metrics_port`: 指标服务端口
///
/// 原因：这些配置涉及底层连接池、缓存实例的初始化参数。
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
            enable_file: false, // 默认仅输出到控制台
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
                .map_err(|e| crate::error::Error::config("port", format!("Invalid port: {e}")))?;
        }

        if let Ok(mode) = std::env::var("CRATES_DOCS_TRANSPORT_MODE") {
            config.server.transport_mode = mode;
        }

        if let Ok(level) = std::env::var("CRATES_DOCS_LOG_LEVEL") {
            config.logging.level = level;
        }

        if let Ok(enable_console) = std::env::var("CRATES_DOCS_ENABLE_CONSOLE") {
            config.logging.enable_console = enable_console.parse().unwrap_or(true);
        }

        if let Ok(enable_file) = std::env::var("CRATES_DOCS_ENABLE_FILE") {
            config.logging.enable_file = enable_file.parse().unwrap_or(true);
        }

        #[cfg(feature = "api-key")]
        {
            if let Ok(enabled) = std::env::var("CRATES_DOCS_API_KEY_ENABLED") {
                config.auth.api_key.enabled = enabled.parse().unwrap_or(false);
            }

            if let Ok(keys) = std::env::var("CRATES_DOCS_API_KEYS") {
                config.auth.api_key.keys = keys
                    .split(',')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(ToOwned::to_owned)
                    .collect();
            }

            if let Ok(header_name) = std::env::var("CRATES_DOCS_API_KEY_HEADER") {
                config.auth.api_key.header_name = header_name;
            }

            if let Ok(query_param_name) = std::env::var("CRATES_DOCS_API_KEY_QUERY_PARAM_NAME") {
                config.auth.api_key.query_param_name = query_param_name;
            }

            if let Ok(allow_query_param) = std::env::var("CRATES_DOCS_API_KEY_ALLOW_QUERY") {
                config.auth.api_key.allow_query_param = allow_query_param.parse().unwrap_or(false);
            }

            if let Ok(key_prefix) = std::env::var("CRATES_DOCS_API_KEY_PREFIX") {
                config.auth.api_key.key_prefix = key_prefix;
            }
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

            #[cfg(feature = "api-key")]
            {
                let default_api_key = crate::server::auth::ApiKeyConfig::default();

                if env.auth.api_key.enabled != default_api_key.enabled {
                    config.auth.api_key.enabled = env.auth.api_key.enabled;
                }

                if env.auth.api_key.keys != default_api_key.keys {
                    config.auth.api_key.keys = env.auth.api_key.keys;
                }

                if env.auth.api_key.header_name != default_api_key.header_name {
                    config.auth.api_key.header_name = env.auth.api_key.header_name;
                }

                if env.auth.api_key.query_param_name != default_api_key.query_param_name {
                    config.auth.api_key.query_param_name = env.auth.api_key.query_param_name;
                }

                if env.auth.api_key.allow_query_param != default_api_key.allow_query_param {
                    config.auth.api_key.allow_query_param = env.auth.api_key.allow_query_param;
                }

                if env.auth.api_key.key_prefix != default_api_key.key_prefix {
                    config.auth.api_key.key_prefix = env.auth.api_key.key_prefix;
                }
            }
        }

        config
    }
}
