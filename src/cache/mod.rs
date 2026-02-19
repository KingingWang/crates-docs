//! Cache module
//!
//! Provides memory cache and Redis cache support.

#[cfg(feature = "cache-memory")]
pub mod memory;

#[cfg(feature = "cache-redis")]
pub mod redis;

use std::time::Duration;

/// Cache trait
#[async_trait::async_trait]
pub trait Cache: Send + Sync {
    /// Get cache value
    async fn get(&self, key: &str) -> Option<String>;

    /// Set cache value
    async fn set(&self, key: String, value: String, ttl: Option<Duration>);

    /// Delete cache value
    async fn delete(&self, key: &str);

    /// Clear cache
    async fn clear(&self);

    /// Check if key exists
    async fn exists(&self, key: &str) -> bool;
}

/// Cache configuration
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct CacheConfig {
    /// Cache type: memory or redis
    pub cache_type: String,

    /// Memory cache size (number of entries)
    pub memory_size: Option<usize>,

    /// Redis connection URL
    pub redis_url: Option<String>,

    /// Default TTL (seconds)
    pub default_ttl: Option<u64>,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            cache_type: "memory".to_string(),
            memory_size: Some(1000),
            redis_url: None,
            default_ttl: Some(3600), // 1 hour
        }
    }
}

/// Create cache instance
///
/// # Errors
///
/// Returns an error if cache type is not supported or configuration is invalid
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
                // Note: Redis cache requires async initialization, this returns a placeholder
                // In practice, use the create_cache_async function
                Err(crate::error::Error::Config(
                    "Redis cache requires async initialization. Use create_cache_async instead."
                        .to_string(),
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

/// Create cache instance asynchronously
///
/// # Errors
///
/// Returns an error if cache type is not supported or configuration is invalid
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
                .ok_or_else(|| crate::error::Error::Config("redis_url is required".to_string()))?;
            Ok(Box::new(redis::RedisCache::new(url).await?))
        }
        _ => Err(crate::error::Error::Config(format!(
            "unsupported cache type: {}",
            config.cache_type
        ))),
    }
}
