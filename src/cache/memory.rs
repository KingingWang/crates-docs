//! 内存缓存实现
//!
//! 使用 LRU（最近最少使用）淘汰策略的内存缓存。

use std::sync::Mutex;
use std::time::{Duration, Instant};

/// 缓存条目
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

/// 内存缓存实现
///
/// 使用 LRU 淘汰策略，当缓存满时移除最近最少使用的条目。
pub struct MemoryCache {
    /// LRU 缓存，使用 Mutex 实现线程安全
    cache: Mutex<lru::LruCache<String, CacheEntry>>,
}

impl MemoryCache {
    /// 创建新的内存缓存
    ///
    /// # Arguments
    /// * `max_size` - 最大缓存条目数
    #[must_use]
    pub fn new(max_size: usize) -> Self {
        // 使用 non-zero 类型确保缓存大小至少为 1
        let cap =
            std::num::NonZeroUsize::new(max_size.max(1)).expect("cache size must be at least 1");
        Self {
            cache: Mutex::new(lru::LruCache::new(cap)),
        }
    }

    /// 清理过期条目
    fn cleanup_expired(cache: &mut lru::LruCache<String, CacheEntry>) {
        // 收集过期的键
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

        // 移除过期条目
        for key in expired_keys {
            cache.pop(&key);
        }
    }
}

#[async_trait::async_trait]
impl super::Cache for MemoryCache {
    async fn get(&self, key: &str) -> Option<String> {
        let mut cache = self.cache.lock().expect("cache lock poisoned");

        // 先检查并清理过期条目
        Self::cleanup_expired(&mut cache);

        // 获取值（LRU 会自动将其移到最近使用的位置）
        cache.get(key).and_then(|entry| {
            if entry.is_expired() {
                None
            } else {
                Some(entry.value.clone())
            }
        })
    }

    async fn set(&self, key: String, value: String, ttl: Option<Duration>) {
        let mut cache = self.cache.lock().expect("cache lock poisoned");

        // 清理过期条目
        Self::cleanup_expired(&mut cache);

        // LRU 会自动处理淘汰
        let entry = CacheEntry::new(value, ttl);
        cache.put(key, entry);
    }

    async fn delete(&self, key: &str) {
        let mut cache = self.cache.lock().expect("cache lock poisoned");
        cache.pop(key);
    }

    async fn clear(&self) {
        let mut cache = self.cache.lock().expect("cache lock poisoned");
        cache.clear();
    }

    async fn exists(&self, key: &str) -> bool {
        let mut cache = self.cache.lock().expect("cache lock poisoned");
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
            .await;
        assert_eq!(cache.get("key1").await, Some("value1".to_string()));

        // 测试删除
        cache.delete("key1").await;
        assert_eq!(cache.get("key1").await, None);

        // 测试清空
        cache
            .set("key2".to_string(), "value2".to_string(), None)
            .await;
        cache.clear().await;
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
            .await;
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
            .await;
        cache
            .set("key2".to_string(), "value2".to_string(), None)
            .await;

        // 访问 key1，使其成为最近使用
        let _ = cache.get("key1").await;

        // 添加第三个条目，应该淘汰 key2（最少使用）
        cache
            .set("key3".to_string(), "value3".to_string(), None)
            .await;

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
            .await;
        assert!(cache.exists("key1").await);
        assert!(!cache.exists("key2").await);
    }
}
