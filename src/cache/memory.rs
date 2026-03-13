//! Memory cache implementation
//!
//! Memory cache using LRU (Least Recently Used) eviction strategy.

use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Cache entry
struct CacheEntry {
    value: String,
    expires_at: Option<Instant>,
}

impl CacheEntry {
    fn new(value: String, ttl: Option<Duration>) -> Self {
        let expires_at = ttl.map(|duration| Instant::now() + duration);
        Self { value, expires_at }
    }

    fn is_expired(&self) -> bool {
        self.expires_at
            .is_some_and(|expiry| expiry <= Instant::now())
    }
}

/// Memory cache implementation
///
/// Uses LRU eviction strategy, removes least recently used entries when cache is full.
pub struct MemoryCache {
    /// LRU cache, using Mutex for thread safety
    cache: Mutex<lru::LruCache<String, CacheEntry>>,
}

impl MemoryCache {
    /// Create a new memory cache
    ///
    /// # Arguments
    /// * `max_size` - Maximum number of cache entries
    #[must_use]
    pub fn new(max_size: usize) -> Self {
        // Use non-zero type to ensure cache size is at least 1
        let cap =
            std::num::NonZeroUsize::new(max_size.max(1)).expect("cache size must be at least 1");
        Self {
            cache: Mutex::new(lru::LruCache::new(cap)),
        }
    }

    /// Safely acquire the cache lock, handling lock poisoning gracefully.
    ///
    /// When a thread panics while holding the lock, the lock becomes "poisoned".
    /// Instead of panicking, we recover the data and log a warning.
    fn acquire_lock(&self) -> std::sync::MutexGuard<'_, lru::LruCache<String, CacheEntry>> {
        self.cache.lock().unwrap_or_else(|poisoned| {
            tracing::warn!(
                "Cache lock was poisoned, recovering data. This indicates a previous panic while holding the lock."
            );
            poisoned.into_inner()
        })
    }

    /// Clean up expired entries
    fn cleanup_expired(cache: &mut lru::LruCache<String, CacheEntry>) {
        // Collect expired keys
        let expired_keys: Vec<String> = cache
            .iter()
            .filter_map(|(k, v)| {
                if v.is_expired() {
                    Some(k.clone())
                } else {
                    None
                }
            })
            .collect();

        // Remove expired entries
        for key in expired_keys {
            cache.pop(&key);
        }
    }
}

#[async_trait::async_trait]
impl super::Cache for MemoryCache {
    async fn get(&self, key: &str) -> Option<String> {
        let mut cache = self.acquire_lock();

        // First check and clean up expired entries
        Self::cleanup_expired(&mut cache);

        // Get value (LRU automatically moves it to most recently used position)
        cache.get(key).and_then(|entry| {
            if entry.is_expired() {
                None
            } else {
                Some(entry.value.clone())
            }
        })
    }

    async fn set(
        &self,
        key: String,
        value: String,
        ttl: Option<Duration>,
    ) -> crate::error::Result<()> {
        let mut cache = self.acquire_lock();

        // Clean up expired entries
        Self::cleanup_expired(&mut cache);

        // LRU automatically handles eviction
        let entry = CacheEntry::new(value, ttl);
        cache.put(key, entry);
        Ok(())
    }

    async fn delete(&self, key: &str) -> crate::error::Result<()> {
        let mut cache = self.acquire_lock();
        cache.pop(key);
        Ok(())
    }

    async fn clear(&self) -> crate::error::Result<()> {
        let mut cache = self.acquire_lock();
        cache.clear();
        Ok(())
    }

    async fn exists(&self, key: &str) -> bool {
        let mut cache = self.acquire_lock();
        Self::cleanup_expired(&mut cache);
        cache.contains(key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::Cache;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_memory_cache_basic() {
        let cache = MemoryCache::new(10);

        // 测试设置和获取
        cache
            .set("key1".to_string(), "value1".to_string(), None)
            .await
            .expect("set should succeed");
        assert_eq!(cache.get("key1").await, Some("value1".to_string()));

        // 测试删除
        cache.delete("key1").await.expect("delete should succeed");
        assert_eq!(cache.get("key1").await, None);

        // 测试清空
        cache
            .set("key2".to_string(), "value2".to_string(), None)
            .await
            .expect("set should succeed");
        cache.clear().await.expect("clear should succeed");
        assert_eq!(cache.get("key2").await, None);
    }

    #[tokio::test]
    async fn test_memory_cache_ttl() {
        let cache = MemoryCache::new(10);

        // 测试带 TTL 的缓存
        cache
            .set(
                "key1".to_string(),
                "value1".to_string(),
                Some(Duration::from_millis(100)),
            )
            .await
            .expect("set should succeed");
        assert_eq!(cache.get("key1").await, Some("value1".to_string()));

        // 等待过期
        sleep(Duration::from_millis(150)).await;
        assert_eq!(cache.get("key1").await, None);
    }

    #[tokio::test]
    async fn test_memory_cache_lru_eviction() {
        let cache = MemoryCache::new(2);

        // 填满缓存
        cache
            .set("key1".to_string(), "value1".to_string(), None)
            .await
            .expect("set should succeed");
        cache
            .set("key2".to_string(), "value2".to_string(), None)
            .await
            .expect("set should succeed");

        // 访问 key1，使其成为最近使用
        let _ = cache.get("key1").await;

        // 添加第三个条目，应该淘汰 key2（最少使用）
        cache
            .set("key3".to_string(), "value3".to_string(), None)
            .await
            .expect("set should succeed");

        // key1 应该还在（因为刚被访问）
        assert_eq!(cache.get("key1").await, Some("value1".to_string()));
        // key2 应该被淘汰
        assert_eq!(cache.get("key2").await, None);
        // key3 应该存在
        assert_eq!(cache.get("key3").await, Some("value3".to_string()));
    }

    #[tokio::test]
    async fn test_memory_cache_exists() {
        let cache = MemoryCache::new(10);

        cache
            .set("key1".to_string(), "value1".to_string(), None)
            .await
            .expect("set should succeed");
        assert!(cache.exists("key1").await);
        assert!(!cache.exists("key2").await);
    }
}
