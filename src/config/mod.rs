//! 配置模块

use crate::cache::CacheConfig;
use crate::server::auth::OAuthConfig;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::fs;

/// 应用程序配置
#[derive(Debug, Clone, Deserialize, Serialize)]
#[derive(Default)]
pub struct AppConfig {
    /// 服务器配置
    pub server: ServerConfig,
    
    /// 缓存配置
    pub cache: CacheConfig,
    
    /// OAuth 配置
    pub oauth: OAuthConfig,
    
    /// 日志配置
    pub logging: LoggingConfig,
    
    /// 性能配置
    pub performance: PerformanceConfig,
}

/// 服务器配置
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServerConfig {
    /// 服务器名称
    pub name: String,
    
    /// 服务器版本
    pub version: String,
    
    /// 服务器描述
    pub description: Option<String>,
    
    /// 主机地址
    pub host: String,
    
    /// 端口
    pub port: u16,
    
    /// 传输模式
    pub transport_mode: String,
    
    /// 启用 SSE 支持
    pub enable_sse: bool,
    
    /// 启用 OAuth 认证
    pub enable_oauth: bool,
    
    /// 最大并发连接数
    pub max_connections: usize,
    
    /// 请求超时时间（秒）
    pub request_timeout_secs: u64,
    
    /// 响应超时时间（秒）
    pub response_timeout_secs: u64,
}

/// 日志配置
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LoggingConfig {
    /// 日志级别
    pub level: String,
    
    /// 日志文件路径
    pub file_path: Option<String>,
    
    /// 是否启用控制台日志
    pub enable_console: bool,
    
    /// 是否启用文件日志
    pub enable_file: bool,
    
    /// 日志文件最大大小（MB）
    pub max_file_size_mb: u64,
    
    /// 保留的日志文件数量
    pub max_files: usize,
}

/// 性能配置
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PerformanceConfig {
    /// HTTP 客户端连接池大小
    pub http_client_pool_size: usize,
    
    /// 缓存最大大小（条目数）
    pub cache_max_size: usize,
    
    /// 缓存默认 TTL（秒）
    pub cache_default_ttl_secs: u64,
    
    /// 请求速率限制（每秒请求数）
    pub rate_limit_per_second: u32,
    
    /// 并发请求限制
    pub concurrent_request_limit: usize,
    
    /// 启用响应压缩
    pub enable_response_compression: bool,
}


impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            name: "crates-docs".to_string(),
            version: crate::VERSION.to_string(),
            description: Some("高性能 Rust crate 文档查询 MCP 服务器".to_string()),
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
    /// 从文件加载配置
    ///
    /// # Errors
    ///
    /// 如果文件不存在、无法读取或格式无效，返回错误
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, crate::error::Error> {
        let content = fs::read_to_string(path)
            .map_err(|e| crate::error::Error::Config(format!("读取配置文件失败: {e}")))?;
        
        let config: Self = toml::from_str(&content)
            .map_err(|e| crate::error::Error::Config(format!("解析配置文件失败: {e}")))?;
        
        config.validate()?;
        Ok(config)
    }
    
    /// 保存配置到文件
    ///
    /// # Errors
    ///
    /// 如果无法序列化配置、创建目录或写入文件，返回错误
    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<(), crate::error::Error> {
        let content = toml::to_string_pretty(self)
            .map_err(|e| crate::error::Error::Config(format!("序列化配置失败: {e}")))?;
        
        // 确保目录存在
        if let Some(parent) = path.as_ref().parent() {
            fs::create_dir_all(parent)
                .map_err(|e| crate::error::Error::Config(format!("创建目录失败: {e}")))?;
        }
        
        fs::write(path, content)
            .map_err(|e| crate::error::Error::Config(format!("写入配置文件失败: {e}")))?;
        
        Ok(())
    }
    
    /// 验证配置
    ///
    /// # Errors
    ///
    /// 如果配置无效（如空主机名、无效端口等），返回错误
    pub fn validate(&self) -> Result<(), crate::error::Error> {
        // 验证服务器配置
        if self.server.host.is_empty() {
            return Err(crate::error::Error::Config("服务器主机不能为空".to_string()));
        }
        
        if self.server.port == 0 {
            return Err(crate::error::Error::Config("服务器端口不能为0".to_string()));
        }
        
        if self.server.max_connections == 0 {
            return Err(crate::error::Error::Config("最大连接数不能为0".to_string()));
        }
        
        // 验证传输模式
        let valid_modes = ["stdio", "http", "sse", "hybrid"];
        if !valid_modes.contains(&self.server.transport_mode.as_str()) {
            return Err(crate::error::Error::Config(format!(
                "无效的传输模式: {}，有效值: {:?}",
                self.server.transport_mode, valid_modes
            )));
        }
        
        // 验证日志级别
        let valid_levels = ["trace", "debug", "info", "warn", "error"];
        if !valid_levels.contains(&self.logging.level.as_str()) {
            return Err(crate::error::Error::Config(format!(
                "无效的日志级别: {}，有效值: {:?}",
                self.logging.level, valid_levels
            )));
        }
        
        // 验证性能配置
        if self.performance.http_client_pool_size == 0 {
            return Err(crate::error::Error::Config("HTTP客户端连接池大小不能为0".to_string()));
        }
        
        if self.performance.cache_max_size == 0 {
            return Err(crate::error::Error::Config("缓存最大大小不能为0".to_string()));
        }
        
        // 验证 OAuth 配置
        if self.server.enable_oauth {
            self.oauth.validate()?;
        }
        
        Ok(())
    }
    
    /// 从环境变量加载配置
    ///
    /// # Errors
    ///
    /// 如果环境变量格式无效或配置验证失败，返回错误
    pub fn from_env() -> Result<Self, crate::error::Error> {
        let mut config = Self::default();
        
        // 从环境变量覆盖配置
        if let Ok(name) = std::env::var("CRATES_DOCS_NAME") {
            config.server.name = name;
        }
        
        if let Ok(host) = std::env::var("CRATES_DOCS_HOST") {
            config.server.host = host;
        }
        
        if let Ok(port) = std::env::var("CRATES_DOCS_PORT") {
            config.server.port = port.parse()
                .map_err(|e| crate::error::Error::Config(format!("无效的端口: {e}")))?;
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
    
    /// 合并配置（环境变量优先于文件配置）
    #[must_use] 
    pub fn merge(file_config: Option<Self>, env_config: Option<Self>) -> Self {
        let mut config = Self::default();
        
        // 首先应用文件配置
        if let Some(file) = file_config {
            config = file;
        }
        
        // 然后应用环境变量配置（覆盖文件配置）
        if let Some(env) = env_config {
            // 合并服务器配置
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
            
            // 合并日志配置
            if env.logging.level != "info" {
                config.logging.level = env.logging.level;
            }
        }
        
        config
    }
}