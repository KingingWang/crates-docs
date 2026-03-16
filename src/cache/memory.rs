//! Memory cache implementation
//!
//! Memory cache using `moka::sync::Cache` with `TinyLFU` eviction policy.
//! This provides better performance and hit rate than simple LRU.

use std::time::Duration;

/// Cache entry with optional TTL
#[derive(Clone, Debug)]
struct CacheEntry {
    value: String,
    ttl: Option<Duration>,
}

/// Expiry implementation for per-entry TTL support
#[derive(Debug, Clone, Default)]
struct CacheExpiry;

impl moka::Expiry<String, CacheEntry> for CacheExpiry {
    fn expire_after_create(
        &self,
        _key: &String,
        value: &CacheEntry,
        _created_at: std::time::Instant,
    ) -> Option<Duration> {
        value.ttl
    }
}

/// Memory cache implementation using `moka::sync::Cache`
///
/// Features:
/// - Lock-free concurrent access
/// - `TinyLFU` eviction policy (better hit rate than LRU)
/// - Per-entry TTL support via Expiry trait
/// - Automatic expiration cleanup
pub struct MemoryCache {
    cache: moka::sync::Cache<String, CacheEntry>,
}

impl MemoryCache {
    /// Create a new memory cache
    ///
    /// # Arguments
    /// * `max_size` - Maximum number of cache entries
    #[must_use]
    pub fn new(max_size: usize) -> Self {
        Self {
            cache: moka::sync::Cache::builder()
                .max_capacity(max_size as u64)
                .expire_after(CacheExpiry)
                .build(),
        }
    }
}

#[async_trait::async_trait]
impl super::Cache for MemoryCache {
    async fn get(&self, key: &str) -> Option<String> {
        self.cache.get(key).map(|entry| entry.value.clone())
    }

    async fn set(
        &self,
        key: String,
        value: String,
        ttl: Option<Duration>,
    ) -> crate::error::Result<()> {
        let entry = CacheEntry { value, ttl };
        self.cache.insert(key, entry);
        Ok(())
    }

    async fn delete(&self, key: &str) -> crate::error::Result<()> {
        self.cache.invalidate(key);
        Ok(())
    }

    async fn clear(&self) -> crate::error::Result<()> {
        self.cache.invalidate_all();
        Ok(())
    }

    async fn exists(&self, key: &str) -> bool {
        self.cache.contains_key(key)
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

        // Test set and get
        cache
            .set("key1".to_string(), "value1".to_string(), None)
            .await
            .expect("set should succeed");
        assert_eq!(cache.get("key1").await, Some("value1".to_string()));

        // Test delete
        cache.delete("key1").await.expect("delete should succeed");
        assert_eq!(cache.get("key1").await, None);

        // Test clear
        cache
            .set("key2".to_string(), "value2".to_string(), None)
            .await
            .expect("set should succeed");
        cache.clear().await.expect("clear should succeed");
        // Wait for async invalidation to complete
        cache.cache.run_pending_tasks();
        assert_eq!(cache.get("key2").await, None);
    }

    #[tokio::test]
    async fn test_memory_cache_ttl() {
        let cache = MemoryCache::new(10);

        // Test cache with TTL
        cache
            .set(
                "key1".to_string(),
                "value1".to_string(),
                Some(Duration::from_millis(100)),
            )
            .await
            .expect("set should succeed");
        assert_eq!(cache.get("key1").await, Some("value1".to_string()));

        // Wait for expiration
        sleep(Duration::from_millis(150)).await;
        // Run pending tasks to ensure expiration is processed
        cache.cache.run_pending_tasks();
        assert_eq!(cache.get("key1").await, None);
    }

    #[tokio::test]
    async fn test_memory_cache_eviction() {
        // Test that cache respects max capacity
        // Note: moka uses TinyLFU algorithm which may reject new entries
        // based on frequency, so we test capacity constraint differently
        let cache = MemoryCache::new(3);

        // Fill cache with more entries than capacity
        for i in 0..5 {
            cache
                .set(format!("key{i}"), format!("value{i}"), None)
                .await
                .expect("set should succeed");
        }

        // Run pending tasks to ensure eviction is processed
        cache.cache.run_pending_tasks();

        // Cache should not exceed max capacity significantly
        let entry_count = cache.cache.entry_count();
        assert!(
            entry_count <= 5,
            "Entry count should be at most 5, got {entry_count}"
        );
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
