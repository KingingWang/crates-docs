//! 缓存模块
//!
//! 提供内存缓存和 Redis 缓存支持。
//!
//! # 特性
//!
//! - **内存缓存**: 基于 `moka` 的高性能内存缓存，支持 `TinyLFU` 淘汰策略
//! - **Redis 缓存**: 支持分布式部署（需要启用 `cache-redis` feature）
//!
//! # 示例
//!
//! ```rust,no_run
//! use crates_docs::cache::{Cache, CacheConfig, create_cache};
//!
//! let config = CacheConfig::default();
//! let cache = create_cache(&config).expect("Failed to create cache");
//! ```

#[cfg(feature = "cache-memory")]
pub mod memory;

#[cfg(feature = "cache-redis")]
pub mod redis;

use std::time::Duration;

/// 缓存 trait
///
/// 定义缓存操作的基本接口，支持异步读写、TTL 过期和批量清理。
///
/// # 实现
///
/// - `memory::MemoryCache`: 内存缓存实现
/// - `redis::RedisCache`: Redis 缓存实现（需要 `cache-redis` feature）
#[async_trait::async_trait]
pub trait Cache: Send + Sync {
    /// 获取缓存值
    ///
    /// # 参数
    ///
    /// * `key` - 缓存键
    ///
    /// # 返回值
    ///
    /// 如果键存在且未过期，返回缓存值；否则返回 `None`
    async fn get(&self, key: &str) -> Option<String>;

    /// 设置缓存值
    ///
    /// # 参数
    ///
    /// * `key` - 缓存键
    /// * `value` - 缓存值
    /// * `ttl` - 可选的过期时间
    ///
    /// # 错误
    ///
    /// 如果缓存操作失败，返回错误
    async fn set(
        &self,
        key: String,
        value: String,
        ttl: Option<Duration>,
    ) -> crate::error::Result<()>;

    /// 删除缓存值
    ///
    /// # 参数
    ///
    /// * `key` - 缓存键
    ///
    /// # 错误
    ///
    /// 如果缓存操作失败，返回错误
    async fn delete(&self, key: &str) -> crate::error::Result<()>;

    /// 清除所有缓存条目
    ///
    /// 仅清除配置了前缀的缓存条目。
    ///
    /// # 错误
    ///
    /// 如果缓存操作失败，返回错误
    async fn clear(&self) -> crate::error::Result<()>;

    /// 检查键是否存在
    ///
    /// # 参数
    ///
    /// * `key` - 缓存键
    ///
    /// # 返回值
    ///
    /// 如果键存在返回 `true`，否则返回 `false`
    async fn exists(&self, key: &str) -> bool;
}

/// 缓存配置
///
/// 配置缓存类型、大小、TTL 等参数。
///
/// # 字段
///
/// - `cache_type`: 缓存类型，`"memory"` 或 `"redis"`
/// - `memory_size`: 内存缓存大小（条目数）
/// - `redis_url`: Redis 连接 URL
/// - `key_prefix`: 键前缀（用于隔离不同服务的缓存）
/// - `default_ttl`: 默认 TTL（秒）
/// - `crate_docs_ttl_secs`: crate 文档缓存 TTL（秒）
/// - `item_docs_ttl_secs`: 项目文档缓存 TTL（秒）
/// - `search_results_ttl_secs`: 搜索结果缓存 TTL（秒）
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct CacheConfig {
    /// 缓存类型：`memory` 或 `redis`
    pub cache_type: String,

    /// 内存缓存大小（条目数）
    pub memory_size: Option<usize>,

    /// Redis 连接 URL
    pub redis_url: Option<String>,

    /// Redis 缓存键前缀（用于隔离不同服务的缓存条目）
    #[serde(default = "default_key_prefix")]
    pub key_prefix: String,

    /// 默认 TTL（秒）
    pub default_ttl: Option<u64>,

    /// crate 文档缓存 TTL（秒）
    #[serde(default = "default_crate_docs_ttl")]
    pub crate_docs_ttl_secs: Option<u64>,

    /// 项目文档缓存 TTL（秒）
    #[serde(default = "default_item_docs_ttl")]
    pub item_docs_ttl_secs: Option<u64>,

    /// 搜索结果缓存 TTL（秒）
    #[serde(default = "default_search_results_ttl")]
    pub search_results_ttl_secs: Option<u64>,
}

/// 默认 crate 文档 TTL（1 小时）
#[must_use]
pub fn default_crate_docs_ttl() -> Option<u64> {
    Some(3600)
}

/// 默认项目文档 TTL（30 分钟）
#[must_use]
pub fn default_item_docs_ttl() -> Option<u64> {
    Some(1800)
}

/// 默认搜索结果 TTL（5 分钟）
#[must_use]
pub fn default_search_results_ttl() -> Option<u64> {
    Some(300)
}

/// 默认键前缀
#[must_use]
pub fn default_key_prefix() -> String {
    String::new()
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            cache_type: "memory".to_string(),
            memory_size: Some(1000),
            redis_url: None,
            key_prefix: String::new(),
            default_ttl: Some(3600), // 1 hour
            crate_docs_ttl_secs: default_crate_docs_ttl(),
            item_docs_ttl_secs: default_item_docs_ttl(),
            search_results_ttl_secs: default_search_results_ttl(),
        }
    }
}

/// 创建缓存实例
///
/// # 参数
///
/// * `config` - 缓存配置
///
/// # 错误
///
/// 如果缓存类型不支持或配置无效，返回错误
///
/// # 示例
///
/// ```rust,no_run
/// use crates_docs::cache::{CacheConfig, create_cache};
///
/// let config = CacheConfig::default();
/// let cache = create_cache(&config).expect("Failed to create cache");
/// ```
pub fn create_cache(config: &CacheConfig) -> Result<Box<dyn Cache>, crate::error::Error> {
    match config.cache_type.as_str() {
        "memory" => {
            #[cfg(feature = "cache-memory")]
            {
                let size = config.memory_size.unwrap_or(1000);
                Ok(Box::new(memory::MemoryCache::new(size)))
            }
            #[cfg(not(feature = "cache-memory"))]
            {
                Err(crate::error::Error::config(
                    "cache_type",
                    "memory cache feature is not enabled",
                ))
            }
        }
        "redis" => {
            #[cfg(feature = "cache-redis")]
            {
                // Note: Redis cache requires async initialization, this returns a placeholder
                // In practice, use the create_cache_async function
                Err(crate::error::Error::config(
                    "cache_type",
                    "Redis cache requires async initialization. Use create_cache_async instead.",
                ))
            }
            #[cfg(not(feature = "cache-redis"))]
            {
                Err(crate::error::Error::config(
                    "cache_type",
                    "redis cache feature is not enabled",
                ))
            }
        }
        _ => Err(crate::error::Error::config(
            "cache_type",
            format!("unsupported cache type: {}", config.cache_type),
        )),
    }
}

/// 异步创建缓存实例
///
/// 支持 Redis 缓存的异步初始化。
///
/// # 参数
///
/// * `config` - 缓存配置
///
/// # 错误
///
/// 如果缓存类型不支持或配置无效，返回错误
///
/// # 示例
///
/// ```rust,no_run
/// use crates_docs::cache::{CacheConfig, create_cache_async};
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let config = CacheConfig::default();
///     let cache = create_cache_async(&config).await?;
///     Ok(())
/// }
/// ```
#[cfg(feature = "cache-redis")]
pub async fn create_cache_async(
    config: &CacheConfig,
) -> Result<Box<dyn Cache>, crate::error::Error> {
    match config.cache_type.as_str() {
        "memory" => {
            let size = config.memory_size.unwrap_or(1000);
            Ok(Box::new(memory::MemoryCache::new(size)))
        }
        "redis" => {
            let url = config
                .redis_url
                .as_ref()
                .ok_or_else(|| crate::error::Error::config("redis_url", "redis_url is required"))?;
            Ok(Box::new(
                redis::RedisCache::new(url, config.key_prefix.clone()).await?,
            ))
        }
        _ => Err(crate::error::Error::config(
            "cache_type",
            format!("unsupported cache type: {}", config.cache_type),
        )),
    }
}
