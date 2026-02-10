//! 缓存模块
//!
//! 提供内存缓存和 Redis 缓存支持。

#[cfg(feature = "cache-memory")]
pub mod memory;

#[cfg(feature = "cache-redis")]
pub mod redis;

use std::time::Duration;

/// 缓存 trait
#[async_trait::async_trait]
pub trait Cache: Send + Sync {
    /// 获取缓存值
    async fn get(&self, key: &str) -> Option<String>;
    
    /// 设置缓存值
    async fn set(&self, key: String, value: String, ttl: Option<Duration>);
    
    /// 删除缓存值
    async fn delete(&self, key: &str);
    
    /// 清空缓存
    async fn clear(&self);
    
    /// 检查键是否存在
    async fn exists(&self, key: &str) -> bool;
}

/// 缓存配置
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct CacheConfig {
    /// 缓存类型：memory 或 redis
    pub cache_type: String,
    
    /// 内存缓存大小（条目数）
    pub memory_size: Option<usize>,
    
    /// Redis 连接 URL
    pub redis_url: Option<String>,
    
    /// 默认 TTL（秒）
    pub default_ttl: Option<u64>,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            cache_type: "memory".to_string(),
            memory_size: Some(1000),
            redis_url: None,
            default_ttl: Some(3600), // 1小时
        }
    }
}

/// 创建缓存实例
///
/// # Errors
///
/// 如果缓存类型不支持或配置无效，返回错误
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
                Err(crate::error::Error::Config(
                    "memory cache feature is not enabled".to_string(),
                ))
            }
        }
        "redis" => {
            #[cfg(feature = "cache-redis")]
            {
                // 注意：Redis 缓存需要异步初始化，这里返回一个占位符
                // 在实际使用中，应该使用 create_cache_async 函数
                Err(crate::error::Error::Config(
                    "Redis cache requires async initialization. Use create_cache_async instead.".to_string(),
                ))
            }
            #[cfg(not(feature = "cache-redis"))]
            {
                Err(crate::error::Error::Config(
                    "redis cache feature is not enabled".to_string(),
                ))
            }
        }
        _ => Err(crate::error::Error::Config(format!(
            "unsupported cache type: {}",
            config.cache_type
        ))),
    }
}

/// 异步创建缓存实例
///
/// # Errors
///
/// 如果缓存类型不支持或配置无效，返回错误
#[cfg(feature = "cache-redis")]
pub async fn create_cache_async(config: &CacheConfig) -> Result<Box<dyn Cache>, crate::error::Error> {
    match config.cache_type.as_str() {
        "memory" => {
            let size = config.memory_size.unwrap_or(1000);
            Ok(Box::new(memory::MemoryCache::new(size)))
        }
        "redis" => {
            let url = config
                .redis_url
                .as_ref()
                .ok_or_else(|| crate::error::Error::Config("redis_url is required".to_string()))?;
            Ok(Box::new(redis::RedisCache::new(url).await?))
        }
        _ => Err(crate::error::Error::Config(format!(
            "unsupported cache type: {}",
            config.cache_type
        ))),
    }
}