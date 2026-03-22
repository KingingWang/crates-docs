//! Document cache module

use crate::cache::Cache;
use std::sync::Arc;
use std::time::Duration;

/// TTL configuration for document cache
#[derive(Debug, Clone, Copy)]
pub struct DocCacheTtl {
    /// TTL for crate documentation (seconds)
    pub crate_docs_secs: u64,
    /// TTL for search results (seconds)
    pub search_results_secs: u64,
    /// TTL for item documentation (seconds)
    pub item_docs_secs: u64,
}

impl Default for DocCacheTtl {
    fn default() -> Self {
        Self {
            crate_docs_secs: 3600,    // 1 hour
            search_results_secs: 300, // 5 minutes
            item_docs_secs: 1800,     // 30 minutes
        }
    }
}

impl DocCacheTtl {
    /// Create TTL config from `CacheConfig`
    #[must_use]
    pub fn from_cache_config(config: &crate::cache::CacheConfig) -> Self {
        Self {
            crate_docs_secs: config.crate_docs_ttl_secs.unwrap_or(3600),
            search_results_secs: config.search_results_ttl_secs.unwrap_or(300),
            item_docs_secs: config.item_docs_ttl_secs.unwrap_or(1800),
        }
    }
}

/// Document cache service
#[derive(Clone)]
pub struct DocCache {
    cache: Arc<dyn Cache>,
    ttl: DocCacheTtl,
}

impl DocCache {
    /// Create a new document cache with default TTL
    pub fn new(cache: Arc<dyn Cache>) -> Self {
        Self {
            cache,
            ttl: DocCacheTtl::default(),
        }
    }

    /// Create a new document cache with custom TTL configuration
    #[must_use]
    pub fn with_ttl(cache: Arc<dyn Cache>, ttl: DocCacheTtl) -> Self {
        Self { cache, ttl }
    }

    /// Get cached document
    pub async fn get_crate_docs(&self, crate_name: &str, version: Option<&str>) -> Option<String> {
        let key = Self::crate_cache_key(crate_name, version);
        self.cache.get(&key).await
    }

    /// Set cached document
    ///
    /// # Errors
    ///
    /// Returns an error if the cache operation fails
    pub async fn set_crate_docs(
        &self,
        crate_name: &str,
        version: Option<&str>,
        content: String,
    ) -> crate::error::Result<()> {
        let key = Self::crate_cache_key(crate_name, version);
        self.cache
            .set(
                key,
                content,
                Some(Duration::from_secs(self.ttl.crate_docs_secs)),
            )
            .await
    }

    /// Get cached search results
    pub async fn get_search_results(&self, query: &str, limit: u32) -> Option<String> {
        let key = Self::search_cache_key(query, limit);
        self.cache.get(&key).await
    }

    /// Set cached search results
    ///
    /// # Errors
    ///
    /// Returns an error if the cache operation fails
    pub async fn set_search_results(
        &self,
        query: &str,
        limit: u32,
        content: String,
    ) -> crate::error::Result<()> {
        let key = Self::search_cache_key(query, limit);
        self.cache
            .set(
                key,
                content,
                Some(Duration::from_secs(self.ttl.search_results_secs)),
            )
            .await
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
    ///
    /// # Errors
    ///
    /// Returns an error if the cache operation fails
    pub async fn set_item_docs(
        &self,
        crate_name: &str,
        item_path: &str,
        version: Option<&str>,
        content: String,
    ) -> crate::error::Result<()> {
        let key = Self::item_cache_key(crate_name, item_path, version);
        self.cache
            .set(
                key,
                content,
                Some(Duration::from_secs(self.ttl.item_docs_secs)),
            )
            .await
    }

    /// Clear cache
    ///
    /// # Errors
    ///
    /// Returns an error if the cache operation fails
    pub async fn clear(&self) -> crate::error::Result<()> {
        self.cache.clear().await
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
            .await
            .expect("set_crate_docs should succeed");
        let cached = doc_cache.get_crate_docs("serde", Some("1.0")).await;
        assert_eq!(cached, Some("Test docs".to_string()));

        // 测试搜索结果缓存
        doc_cache
            .set_search_results("web framework", 10, "Search results".to_string())
            .await
            .expect("set_search_results should succeed");
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
            .await
            .expect("set_item_docs should succeed");
        let item_cached = doc_cache
            .get_item_docs("serde", "serde::Serialize", Some("1.0"))
            .await;
        assert_eq!(item_cached, Some("Item docs".to_string()));

        // 测试清理
        doc_cache.clear().await.expect("clear should succeed");
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

    #[test]
    fn test_doc_cache_ttl_default() {
        let ttl = DocCacheTtl::default();
        assert_eq!(ttl.crate_docs_secs, 3600);
        assert_eq!(ttl.search_results_secs, 300);
        assert_eq!(ttl.item_docs_secs, 1800);
    }

    #[test]
    fn test_doc_cache_ttl_from_config() {
        let config = crate::cache::CacheConfig {
            cache_type: "memory".to_string(),
            memory_size: Some(1000),
            redis_url: None,
            key_prefix: String::new(),
            default_ttl: Some(3600),
            crate_docs_ttl_secs: Some(7200),
            item_docs_ttl_secs: Some(3600),
            search_results_ttl_secs: Some(600),
        };
        let ttl = DocCacheTtl::from_cache_config(&config);
        assert_eq!(ttl.crate_docs_secs, 7200);
        assert_eq!(ttl.item_docs_secs, 3600);
        assert_eq!(ttl.search_results_secs, 600);
    }
}
