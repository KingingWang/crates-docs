//! 内存缓存实现

use std::time::{Duration, Instant};
use parking_lot::RwLock;
use std::collections::HashMap;

/// 缓存条目
struct CacheEntry {
    value: String,
    expires_at: Option<Instant>,
}

impl CacheEntry {
    fn is_expired(&self) -> bool {
        self.expires_at.is_some_and(|expiry| expiry <= Instant::now())
    }
}

/// 内存缓存实现
pub struct MemoryCache {
    cache: RwLock<HashMap<String, CacheEntry>>,
    max_size: usize,
}

impl MemoryCache {
    /// 创建新的内存缓存
    #[must_use] 
    pub fn new(max_size: usize) -> Self {
        Self {
            cache: RwLock::new(HashMap::with_capacity(max_size)),
            max_size,
        }
    }

    /// 清理过期条目
    fn cleanup(&self) {
        let mut cache = self.cache.write();
        cache.retain(|_, entry| !entry.is_expired());
    }

    /// 如果缓存已满，移除最旧的条目
    fn evict_if_full(&self) {
        let mut cache = self.cache.write();
        if cache.len() >= self.max_size {
            // 简单策略：移除第一个条目
            if let Some(key) = cache.keys().next().cloned() {
                cache.remove(&key);
            }
        }
    }
}

#[async_trait::async_trait]
impl super::Cache for MemoryCache {
    async fn get(&self, key: &str) -> Option<String> {
        let cache = self.cache.read();
        cache.get(key).and_then(|entry| {
            if entry.is_expired() {
                None
            } else {
                Some(entry.value.clone())
            }
        })
    }

    async fn set(&self, key: String, value: String, ttl: Option<Duration>) {
        self.cleanup();
        self.evict_if_full();

        let expires_at = ttl.map(|duration| Instant::now() + duration);
        
        let entry = CacheEntry {
            value,
            expires_at,
        };

        let mut cache = self.cache.write();
        cache.insert(key, entry);
    }

    async fn delete(&self, key: &str) {
        let mut cache = self.cache.write();
        cache.remove(key);
    }

    async fn clear(&self) {
        let mut cache = self.cache.write();
        cache.clear();
    }

    async fn exists(&self, key: &str) -> bool {
        let cache = self.cache.read();
        cache.get(key).is_some_and(|entry| !entry.is_expired())
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
        cache.set("key1".to_string(), "value1".to_string(), None).await;
        assert_eq!(cache.get("key1").await, Some("value1".to_string()));
        
        // 测试删除
        cache.delete("key1").await;
        assert_eq!(cache.get("key1").await, None);
        
        // 测试清空
        cache.set("key2".to_string(), "value2".to_string(), None).await;
        cache.clear().await;
        assert_eq!(cache.get("key2").await, None);
    }

    #[tokio::test]
    async fn test_memory_cache_ttl() {
        let cache = MemoryCache::new(10);
        
        // 测试带 TTL 的缓存
        cache.set(
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
    async fn test_memory_cache_eviction() {
        let cache = MemoryCache::new(2);
        
        // 填满缓存
        cache.set("key1".to_string(), "value1".to_string(), None).await;
        cache.set("key2".to_string(), "value2".to_string(), None).await;
        
        // 添加第三个条目，应该触发淘汰
        cache.set("key3".to_string(), "value3".to_string(), None).await;
        
        // 缓存中应该只有 2 个条目
        let cache_size = {
            let cache_read = cache.cache.read();
            cache_read.len()
        };
        assert_eq!(cache_size, 2);
    }
}