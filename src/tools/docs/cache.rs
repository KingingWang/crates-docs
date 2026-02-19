//! Document cache module

use crate::cache::Cache;
use std::sync::Arc;
use std::time::Duration;

/// Document cache service
#[derive(Clone)]
pub struct DocCache {
    cache: Arc<dyn Cache>,
}

impl DocCache {
    /// Create a new document cache
    pub fn new(cache: Arc<dyn Cache>) -> Self {
        Self { cache }
    }

    /// Get cached document
    pub async fn get_crate_docs(&self, crate_name: &str, version: Option<&str>) -> Option<String> {
        let key = Self::crate_cache_key(crate_name, version);
        self.cache.get(&key).await
    }

    /// Set cached document
    pub async fn set_crate_docs(&self, crate_name: &str, version: Option<&str>, content: String) {
        let key = Self::crate_cache_key(crate_name, version);
        self.cache
            .set(key, content, Some(Duration::from_secs(3600)))
            .await;
    }

    /// Get cached search results
    pub async fn get_search_results(&self, query: &str, limit: u32) -> Option<String> {
        let key = Self::search_cache_key(query, limit);
        self.cache.get(&key).await
    }

    /// Set cached search results
    pub async fn set_search_results(&self, query: &str, limit: u32, content: String) {
        let key = Self::search_cache_key(query, limit);
        self.cache
            .set(key, content, Some(Duration::from_secs(300)))
            .await; // 5 minutes cache
    }

    /// Get cached item documentation
    pub async fn get_item_docs(
        &self,
        crate_name: &str,
        item_path: &str,
        version: Option<&str>,
    ) -> Option<String> {
        let key = Self::item_cache_key(crate_name, item_path, version);
        self.cache.get(&key).await
    }

    /// Set cached item documentation
    pub async fn set_item_docs(
        &self,
        crate_name: &str,
        item_path: &str,
        version: Option<&str>,
        content: String,
    ) {
        let key = Self::item_cache_key(crate_name, item_path, version);
        self.cache
            .set(key, content, Some(Duration::from_secs(1800)))
            .await; // 30 minutes cache
    }

    /// Clear cache
    pub async fn clear(&self) {
        self.cache.clear().await;
    }

    /// Build crate cache key
    fn crate_cache_key(crate_name: &str, version: Option<&str>) -> String {
        if let Some(ver) = version {
            format!("crate:{crate_name}:{ver}")
        } else {
            format!("crate:{crate_name}")
        }
    }

    /// Build search cache key
    fn search_cache_key(query: &str, limit: u32) -> String {
        format!("search:{query}:{limit}")
    }

    /// Build item cache key
    fn item_cache_key(crate_name: &str, item_path: &str, version: Option<&str>) -> String {
        if let Some(ver) = version {
            format!("item:{crate_name}:{ver}:{item_path}")
        } else {
            format!("item:{crate_name}:{item_path}")
        }
    }
}

impl Default for DocCache {
    fn default() -> Self {
        let cache = Arc::new(crate::cache::memory::MemoryCache::new(1000));
        Self::new(cache)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::memory::MemoryCache;

    #[tokio::test]
    async fn test_doc_cache() {
        let memory_cache = MemoryCache::new(100);
        let cache = Arc::new(memory_cache);
        let doc_cache = DocCache::new(cache);

        // 测试 crate 文档缓存
        doc_cache
            .set_crate_docs("serde", Some("1.0"), "Test docs".to_string())
            .await;
        let cached = doc_cache.get_crate_docs("serde", Some("1.0")).await;
        assert_eq!(cached, Some("Test docs".to_string()));

        // 测试搜索结果缓存
        doc_cache
            .set_search_results("web framework", 10, "Search results".to_string())
            .await;
        let search_cached = doc_cache.get_search_results("web framework", 10).await;
        assert_eq!(search_cached, Some("Search results".to_string()));

        // 测试项目文档缓存
        doc_cache
            .set_item_docs(
                "serde",
                "serde::Serialize",
                Some("1.0"),
                "Item docs".to_string(),
            )
            .await;
        let item_cached = doc_cache
            .get_item_docs("serde", "serde::Serialize", Some("1.0"))
            .await;
        assert_eq!(item_cached, Some("Item docs".to_string()));

        // 测试清理
        doc_cache.clear().await;
        let cleared = doc_cache.get_crate_docs("serde", Some("1.0")).await;
        assert_eq!(cleared, None);
    }

    #[test]
    fn test_cache_key_generation() {
        assert_eq!(DocCache::crate_cache_key("serde", None), "crate:serde");
        assert_eq!(
            DocCache::crate_cache_key("serde", Some("1.0")),
            "crate:serde:1.0"
        );

        assert_eq!(
            DocCache::search_cache_key("web framework", 10),
            "search:web framework:10"
        );

        assert_eq!(
            DocCache::item_cache_key("serde", "Serialize", None),
            "item:serde:Serialize"
        );
        assert_eq!(
            DocCache::item_cache_key("serde", "Serialize", Some("1.0")),
            "item:serde:1.0:Serialize"
        );
    }
}
