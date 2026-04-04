//! Document cache module
//!
//! Provides document-specific cache service with support for independent TTL configuration
//! for crate docs, search results, and item docs.
//!
//! # Cache key format
//!
//! - Crate documentation: `crate:{name}` or `crate:{name}:{version}`
//! - Search results: `search:{query}:{limit}`
//! - Item documentation: `item:{crate}:{path}` or `item:{crate}:{version}:{path}`
//!
//! # Examples
//!
//! ```rust,no_run
//! use std::sync::Arc;
//! use crates_docs::tools::docs::cache::{DocCache, DocCacheTtl};
//! use crates_docs::cache::memory::MemoryCache;
//!
//! let cache = Arc::new(MemoryCache::new(1000));
//! let doc_cache = DocCache::new(cache);
//! ```

mod key;
mod stats;
mod ttl;

use crate::cache::Cache;
use std::sync::Arc;

// Re-export public types
pub use key::CacheKeyGenerator;
pub use stats::CacheStats;
pub use ttl::DocCacheTtl;

/// Document cache service
///
/// Provides document-specific cache operations, supports crate docs, search results, and item docs.
///
/// # Fields
///
/// - `cache`: Underlying cache instance
/// - `ttl`: TTL configuration
/// - `stats`: Cache statistics
#[derive(Clone)]
pub struct DocCache {
    cache: Arc<dyn Cache>,
    ttl: DocCacheTtl,
    stats: CacheStats,
}

impl DocCache {
    /// Create new document cache (with default TTL)
    ///
    /// # Arguments
    ///
    /// * `cache` - cache instance
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use std::sync::Arc;
    /// use crates_docs::tools::docs::cache::DocCache;
    /// use crates_docs::cache::memory::MemoryCache;
    ///
    /// let cache = Arc::new(MemoryCache::new(1000));
    /// let doc_cache = DocCache::new(cache);
    /// ```
    pub fn new(cache: Arc<dyn Cache>) -> Self {
        Self {
            cache,
            ttl: DocCacheTtl::default(),
            stats: CacheStats::new(),
        }
    }

    /// Create new document cache (with custom TTL)
    ///
    /// # Arguments
    ///
    /// * `cache` - cache instance
    /// * `ttl` - TTL configuration
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use std::sync::Arc;
    /// use crates_docs::tools::docs::cache::{DocCache, DocCacheTtl};
    /// use crates_docs::cache::memory::MemoryCache;
    ///
    /// let cache = Arc::new(MemoryCache::new(1000));
    /// let ttl = DocCacheTtl::with_jitter(
    ///     7200,  // crate_docs_secs
    ///     600,   // search_results_secs
    ///     3600,  // item_docs_secs
    ///     0.1,   // jitter_ratio
    /// );
    /// let doc_cache = DocCache::with_ttl(cache, ttl);
    /// ```
    pub fn with_ttl(cache: Arc<dyn Cache>, ttl: DocCacheTtl) -> Self {
        Self {
            cache,
            ttl,
            stats: CacheStats::new(),
        }
    }

    /// Get cached crate documentation
    ///
    /// # Arguments
    ///
    /// * `crate_name` - crate name
    /// * `version` - Optional version
    ///
    /// # Returns
    ///
    /// Returns document content if cache hit; otherwise returns `None`
    pub async fn get_crate_docs(&self, crate_name: &str, version: Option<&str>) -> Option<String> {
        let key = CacheKeyGenerator::crate_cache_key(crate_name, version);
        let result = self.cache.get(&key).await;
        if result.is_some() {
            self.stats.record_hit();
        } else {
            self.stats.record_miss();
        }
        // Convert Arc<String> to String for API compatibility
        result.map(|arc| (*arc).clone())
    }

    /// Set crate document cache
    ///
    /// # Arguments
    ///
    /// * `crate_name` - crate name
    /// * `version` - Optional version
    /// * `content` - Document content
    ///
    /// # Errors
    ///
    /// Returns error if cache operation fails
    pub async fn set_crate_docs(
        &self,
        crate_name: &str,
        version: Option<&str>,
        content: String,
    ) -> crate::error::Result<()> {
        let key = CacheKeyGenerator::crate_cache_key(crate_name, version);
        let ttl = self.ttl.crate_docs_duration();
        self.cache.set(key, content, Some(ttl)).await?;
        self.stats.record_set();
        Ok(())
    }

    /// Get cached crate HTML
    pub async fn get_crate_html(&self, crate_name: &str, version: Option<&str>) -> Option<String> {
        let key = CacheKeyGenerator::crate_html_cache_key(crate_name, version);
        let result = self.cache.get(&key).await;
        if result.is_some() {
            self.stats.record_hit();
        } else {
            self.stats.record_miss();
        }
        result.map(|arc| (*arc).clone())
    }

    /// Set crate HTML cache
    ///
    /// # Errors
    ///
    /// Returns error if cache operation fails
    pub async fn set_crate_html(
        &self,
        crate_name: &str,
        version: Option<&str>,
        content: String,
    ) -> crate::error::Result<()> {
        let key = CacheKeyGenerator::crate_html_cache_key(crate_name, version);
        let ttl = self.ttl.crate_docs_duration();
        self.cache.set(key, content, Some(ttl)).await?;
        self.stats.record_set();
        Ok(())
    }

    /// Get cached search results
    ///
    /// # Arguments
    ///
    /// * `query` - Search query
    /// * `limit` - Result count limit
    /// * `sort` - Optional search sort order
    ///
    /// # Returns
    ///
    /// Returns search results if cache hit;otherwise returns `None`
    pub async fn get_search_results(
        &self,
        query: &str,
        limit: u32,
        sort: Option<&str>,
    ) -> Option<String> {
        let key = CacheKeyGenerator::search_cache_key(query, limit, sort);
        let result = self.cache.get(&key).await;
        if result.is_some() {
            self.stats.record_hit();
        } else {
            self.stats.record_miss();
        }
        // Convert Arc<String> to String for API compatibility
        result.map(|arc| (*arc).clone())
    }

    /// Set search results cache
    ///
    /// # Arguments
    ///
    /// * `query` - Search query
    /// * `limit` - Result count limit
    /// * `sort` - Optional search sort order
    /// * `content` - search result content
    ///
    /// # Errors
    ///
    /// Returns error if cache operation fails
    pub async fn set_search_results(
        &self,
        query: &str,
        limit: u32,
        sort: Option<&str>,
        content: String,
    ) -> crate::error::Result<()> {
        let key = CacheKeyGenerator::search_cache_key(query, limit, sort);
        let ttl = self.ttl.search_results_duration();
        self.cache.set(key, content, Some(ttl)).await?;
        self.stats.record_set();
        Ok(())
    }

    /// Get cached item docs
    ///
    /// # Arguments
    ///
    /// * `crate_name` - crate name
    /// * `item_path` - Item path
    /// * `version` - Optional version
    ///
    /// # Returns
    ///
    /// Returns item docs if cache hit;otherwise returns `None`
    pub async fn get_item_docs(
        &self,
        crate_name: &str,
        item_path: &str,
        version: Option<&str>,
    ) -> Option<String> {
        let key = CacheKeyGenerator::item_cache_key(crate_name, item_path, version);
        let result = self.cache.get(&key).await;
        if result.is_some() {
            self.stats.record_hit();
        } else {
            self.stats.record_miss();
        }
        // Convert Arc<String> to String for API compatibility
        result.map(|arc| (*arc).clone())
    }

    /// Set item docs cache
    ///
    /// # Arguments
    ///
    /// * `crate_name` - crate name
    /// * `item_path` - Item path
    /// * `version` - Optional version
    /// * `content` - Document content
    ///
    /// # Errors
    ///
    /// Returns error if cache operation fails
    pub async fn set_item_docs(
        &self,
        crate_name: &str,
        item_path: &str,
        version: Option<&str>,
        content: String,
    ) -> crate::error::Result<()> {
        let key = CacheKeyGenerator::item_cache_key(crate_name, item_path, version);
        let ttl = self.ttl.item_docs_duration();
        self.cache.set(key, content, Some(ttl)).await?;
        self.stats.record_set();
        Ok(())
    }

    /// Get cached item HTML
    pub async fn get_item_html(
        &self,
        crate_name: &str,
        item_path: &str,
        version: Option<&str>,
    ) -> Option<String> {
        let key = CacheKeyGenerator::item_html_cache_key(crate_name, item_path, version);
        let result = self.cache.get(&key).await;
        if result.is_some() {
            self.stats.record_hit();
        } else {
            self.stats.record_miss();
        }
        result.map(|arc| (*arc).clone())
    }

    /// Set item HTML cache
    ///
    /// # Errors
    ///
    /// Returns error if cache operation fails
    pub async fn set_item_html(
        &self,
        crate_name: &str,
        item_path: &str,
        version: Option<&str>,
        content: String,
    ) -> crate::error::Result<()> {
        let key = CacheKeyGenerator::item_html_cache_key(crate_name, item_path, version);
        let ttl = self.ttl.item_docs_duration();
        self.cache.set(key, content, Some(ttl)).await?;
        self.stats.record_set();
        Ok(())
    }

    /// Clear cache
    ///
    /// # Errors
    ///
    /// Returns error if cache operation fails
    pub async fn clear(&self) -> crate::error::Result<()> {
        self.cache.clear().await
    }

    /// Get cache statistics
    #[must_use]
    pub fn stats(&self) -> &CacheStats {
        &self.stats
    }

    /// Get TTL configuration
    #[must_use]
    pub fn ttl(&self) -> &DocCacheTtl {
        &self.ttl
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

        // Test crate document cache
        doc_cache
            .set_crate_docs("serde", Some("1.0"), "Test docs".to_string())
            .await
            .expect("set_crate_docs should succeed");
        let cached = doc_cache.get_crate_docs("serde", Some("1.0")).await;
        assert_eq!(cached, Some("Test docs".to_string()));

        // Test search results cache
        doc_cache
            .set_search_results(
                "web framework",
                10,
                Some("relevance"),
                "Search results".to_string(),
            )
            .await
            .expect("set_search_results should succeed");
        let search_cached = doc_cache
            .get_search_results("web framework", 10, Some("relevance"))
            .await;
        assert_eq!(search_cached, Some("Search results".to_string()));

        // Test item docs cache
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

        // Test clear
        doc_cache.clear().await.expect("clear should succeed");
        let cleared = doc_cache.get_crate_docs("serde", Some("1.0")).await;
        assert_eq!(cleared, None);
    }

    #[tokio::test]
    async fn test_doc_cache_with_ttl() {
        let memory_cache = MemoryCache::new(100);
        let cache = Arc::new(memory_cache);

        let mut ttl = DocCacheTtl::default();
        ttl.crate_docs_secs = 7200;
        ttl.search_results_secs = 600;
        ttl.item_docs_secs = 3600;
        ttl.set_jitter_ratio(0.0); // Disable jitter for predictable tests

        let doc_cache = DocCache::with_ttl(cache, ttl);

        assert_eq!(doc_cache.ttl().crate_docs_secs, 7200);
        assert_eq!(doc_cache.ttl().search_results_secs, 600);
        assert_eq!(doc_cache.ttl().item_docs_secs, 3600);
    }

    #[tokio::test]
    async fn test_doc_cache_stats() {
        let memory_cache = MemoryCache::new(100);
        let cache = Arc::new(memory_cache);
        let doc_cache = DocCache::new(cache);

        // Record a hit
        doc_cache
            .set_crate_docs("serde", None, "docs".to_string())
            .await
            .ok();
        doc_cache.get_crate_docs("serde", None).await;

        // Record a miss
        doc_cache.get_crate_docs("nonexistent", None).await;

        assert_eq!(doc_cache.stats().hits(), 1);
        assert_eq!(doc_cache.stats().misses(), 1);
        assert_eq!(doc_cache.stats().sets(), 1);
    }

    #[test]
    fn test_doc_cache_default() {
        let doc_cache = DocCache::default();
        assert_eq!(doc_cache.ttl().crate_docs_secs, 3600);
        assert_eq!(doc_cache.ttl().search_results_secs, 300);
        assert_eq!(doc_cache.ttl().item_docs_secs, 1800);
    }
}
