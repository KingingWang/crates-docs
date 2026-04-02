//! Cache module
//!
//! Provides memory cache and Redis cache support.
//!
//! # Features
//!
//! - **Memory cache**: High-performance memory cache based on `moka`, supporting `TinyLFU` eviction strategy
//! - **Redis cache**: Supports distributed deployment (requires `cache-redis` feature)
//!
//! # Examples
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

use std::sync::Arc;
use std::time::Duration;

/// Default memory cache capacity
///
/// # Value
///
/// 1000 entries
///
/// # Rationale
///
/// Provides good balance between memory usage and cache hit rate for typical workloads.
/// Configurable via `CacheConfig::memory_size`.
const DEFAULT_MEMORY_CACHE_SIZE: usize = 1000;

/// Default crate documentation TTL in seconds
///
/// # Value
///
/// 3600 seconds (1 hour)
///
/// # Rationale
///
/// Reused from ttl.rs for consistency. Crate documentation changes infrequently.
/// Configurable via `CacheConfig::crate_docs_ttl_secs`.
const DEFAULT_CRATE_DOCS_TTL_SECS: u64 = 3600;

/// Default item documentation TTL in seconds
///
/// # Value
///
/// 1800 seconds (30 minutes)
///
/// # Rationale
///
/// Reused from ttl.rs for consistency. Item documentation changes moderately often.
/// Configurable via `CacheConfig::item_docs_ttl_secs`.
const DEFAULT_ITEM_DOCS_TTL_SECS: u64 = 1800;

/// Default search results TTL in seconds
///
/// # Value
///
/// 300 seconds (5 minutes)
///
/// # Rationale
///
/// Reused from ttl.rs for consistency. Search results change frequently.
/// Configurable via `CacheConfig::search_results_ttl_secs`.
const DEFAULT_SEARCH_RESULTS_TTL_SECS: u64 = 300;

/// Cache trait
///
/// Defines basic cache operation interface, supporting async read/write, TTL expiration, and bulk cleanup.
///
/// # Implementations
///
/// - `memory::MemoryCache`: Memory cache implementation
/// - `redis::RedisCache`: Redis cache implementation (requires `cache-redis` feature)
#[async_trait::async_trait]
pub trait Cache: Send + Sync {
    /// Get cache value
    ///
    /// # Arguments
    ///
    /// * `key` - Cache key
    ///
    /// # Returns
    ///
    /// If key exists and not expired, returns `Arc<String>` to avoid cloning; otherwise returns `None`
    async fn get(&self, key: &str) -> Option<Arc<String>>;

    /// Set cache value
    ///
    /// # Arguments
    ///
    /// * `key` - Cache key
    /// * `value` - Cache value
    /// * `ttl` - Optional expiration time
    ///
    /// # Errors
    ///
    /// Returns error if cache operation fails
    async fn set(
        &self,
        key: String,
        value: String,
        ttl: Option<Duration>,
    ) -> crate::error::Result<()>;

    /// Delete cache value
    ///
    /// # Arguments
    ///
    /// * `key` - Cache key
    ///
    /// # Errors
    ///
    /// Returns error if cache operation fails
    async fn delete(&self, key: &str) -> crate::error::Result<()>;

    /// Clear all cache entries
    ///
    /// Clears only cache entries with configured prefix.
    ///
    /// # Errors
    ///
    /// Returns error if cache operation fails
    async fn clear(&self) -> crate::error::Result<()>;

    /// Check if key exists
    ///
    /// # Arguments
    ///
    /// * `key` - Cache key
    ///
    /// # Returns
    ///
    /// Returns `true` if key exists, otherwise `false`
    async fn exists(&self, key: &str) -> bool;
}

/// Cache configuration
///
/// Configure cache type, size, TTL, and other parameters.
///
/// # Fields
///
/// - `cache_type`: Cache type, `"memory"` or `"redis"`
/// - `memory_size`: Memory cache size(number of entries)
/// - `redis_url`: Redis connection URL
/// - `key_prefix`: Key prefix (used to isolate caches of different services)
/// - `default_ttl`: Default TTL (seconds)
/// - `crate_docs_ttl_secs`: Crate document cache TTL (seconds)
/// - `item_docs_ttl_secs`: Item document cache TTL (seconds)
/// - `search_results_ttl_secs`: Search result cache TTL (seconds)
///
/// # Hot reload support
///
/// ## Hot reload supported fields ✅
///
/// TTL-related fields can be dynamically updated at runtime (affecting newly written cache entries):
/// - `default_ttl`: Default TTL (seconds)
/// - `crate_docs_ttl_secs`: Crate document cache TTL (seconds)
/// - `item_docs_ttl_secs`: Item document cache TTL (seconds)
/// - `search_results_ttl_secs`: Search result cache TTL (seconds)
///
/// ## Hot reload NOT supported fields ❌
///
/// The following fields require server restart to take effect:
/// - `cache_type`: Cache type (involves cache instance creation)
/// - `memory_size`: Memory cache size(initialization parameter)
/// - `redis_url`: Redis connection URL(connection pool initialization)
/// - `key_prefix`: Cache key prefix(initialization parameter)
///
/// Reason: These configurations involve initialization of cache backend (memory/Redis) and connection pool creation.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct CacheConfig {
    /// Cache type: `memory` or `redis`
    pub cache_type: String,

    /// Memory cache size(number of entries)
    pub memory_size: Option<usize>,

    /// Redis connection URL
    pub redis_url: Option<String>,

    /// Redis cache key prefix (used to isolate caches of different services)
    #[serde(default = "default_key_prefix")]
    pub key_prefix: String,

    /// Default TTL (seconds)
    pub default_ttl: Option<u64>,

    /// Crate document cache TTL (seconds)
    #[serde(default = "default_crate_docs_ttl")]
    pub crate_docs_ttl_secs: Option<u64>,

    /// Item document cache TTL (seconds)
    #[serde(default = "default_item_docs_ttl")]
    pub item_docs_ttl_secs: Option<u64>,

    /// Search result cache TTL (seconds)
    #[serde(default = "default_search_results_ttl")]
    pub search_results_ttl_secs: Option<u64>,
}

/// Default crate document TTL (1 hour)
#[must_use]
pub fn default_crate_docs_ttl() -> Option<u64> {
    Some(DEFAULT_CRATE_DOCS_TTL_SECS)
}

/// Default item document TTL (30 minutes)
#[must_use]
pub fn default_item_docs_ttl() -> Option<u64> {
    Some(DEFAULT_ITEM_DOCS_TTL_SECS)
}

/// Default search result TTL (5 minutes)
#[must_use]
pub fn default_search_results_ttl() -> Option<u64> {
    Some(DEFAULT_SEARCH_RESULTS_TTL_SECS)
}

/// Default key prefix
#[must_use]
pub fn default_key_prefix() -> String {
    String::new()
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            cache_type: "memory".to_string(),
            memory_size: Some(DEFAULT_MEMORY_CACHE_SIZE),
            redis_url: None,
            key_prefix: String::new(),
            default_ttl: Some(DEFAULT_CRATE_DOCS_TTL_SECS),
            crate_docs_ttl_secs: default_crate_docs_ttl(),
            item_docs_ttl_secs: default_item_docs_ttl(),
            search_results_ttl_secs: default_search_results_ttl(),
        }
    }
}

/// Create cache instance
///
/// # Arguments
///
/// * `config` - Cache configuration
///
/// # Errors
///
/// Returns error if cache type is not supported or configuration is invalid
///
/// # Examples
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
                let size = config.memory_size.unwrap_or(DEFAULT_MEMORY_CACHE_SIZE);
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

/// Async create cache instance
///
/// Supports async initialization for Redis cache.
///
/// # Arguments
///
/// * `config` - Cache configuration
///
/// # Errors
///
/// Returns error if cache type is not supported or configuration is invalid
///
/// # Examples
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
            let size = config.memory_size.unwrap_or(DEFAULT_MEMORY_CACHE_SIZE);
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
