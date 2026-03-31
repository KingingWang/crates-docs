//! Document cache module
//!
//! Provides document-specific cache service,Supports independent TTL configuration for crate docs, search results, and item docs.
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

use crate::cache::Cache;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::time::Duration;

/// Document cache TTL configuration
///
/// Configure independent TTL for different document types.
///
/// # Fields
///
/// - `crate_docs_secs`: Crate document cache duration (seconds)
/// - `search_results_secs`: search results cache duration (seconds)
/// - `item_docs_secs`: item docs cache duration (seconds)
/// - `jitter_ratio`: TTL jitter ratio(0.0-1.0),used to prevent cache stampede
#[derive(Debug, Clone, Copy)]
pub struct DocCacheTtl {
    /// Crate document TTL (seconds)
    pub crate_docs_secs: u64,
    /// Search results TTL (seconds)
    pub search_results_secs: u64,
    /// Item documentation TTL (seconds)
    pub item_docs_secs: u64,
    /// TTL jitter ratio (0.0-1.0), default 0.1 (10%)
    ///
    /// Actual TTL = `base_ttl * (1 + random(-jitter_ratio, jitter_ratio))`
    /// for example:`base_ttl=3600`, `jitter_ratio=0.1` => Actual TTL range `[3240, 3960]`
    pub jitter_ratio: f64,
}

/// Default TTL jitter ratio (10%)
const DEFAULT_JITTER_RATIO: f64 = 0.1;

impl Default for DocCacheTtl {
    fn default() -> Self {
        Self {
            crate_docs_secs: 3600,    // 1 hour
            search_results_secs: 300, // 5 minutes
            item_docs_secs: 1800,     // 30 minutes
            jitter_ratio: DEFAULT_JITTER_RATIO,
        }
    }
}

impl DocCacheTtl {
    /// Create TTL configuration from `CacheConfig`
    ///
    /// # Arguments
    ///
    /// * `config` - cache configuration
    ///
    /// # Returns
    ///
    /// Returns TTL configuration based on config
    #[must_use]
    pub fn from_cache_config(config: &crate::cache::CacheConfig) -> Self {
        Self {
            crate_docs_secs: config.crate_docs_ttl_secs.unwrap_or(3600),
            search_results_secs: config.search_results_ttl_secs.unwrap_or(300),
            item_docs_secs: config.item_docs_ttl_secs.unwrap_or(1800),
            jitter_ratio: DEFAULT_JITTER_RATIO,
        }
    }

    /// Calculate actual TTL with jitter
    ///
    /// # Arguments
    ///
    /// * `base_ttl` - Base TTL (seconds)
    ///
    /// # Returns
    ///
    /// Returns jittered TTL (seconds)
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    #[allow(clippy::cast_sign_loss)]
    #[allow(clippy::cast_precision_loss)]
    pub fn apply_jitter(&self, base_ttl: u64) -> u64 {
        if self.jitter_ratio <= 0.0 {
            return base_ttl;
        }

        // Limit jitter_ratio to [0.0, 1.0] range
        let ratio = self.jitter_ratio.clamp(0.0, 1.0);

        // Generate random offset in [-ratio, +ratio] range
        let rng = fastrand::f64();
        let offset = (rng * 2.0 - 1.0) * ratio;

        // Calculate jittered TTL, ensuring at least 1 second
        (base_ttl as f64 * (1.0 + offset)).max(1.0) as u64
    }
}

/// Document cache service
///
/// provides document-specific cache operations,supports crate docs, search results, and item docs.
///
/// # Fields
///
/// - `cache`: Underlying cache instance
/// - `ttl`: TTL configuration
#[derive(Clone)]
pub struct DocCache {
    cache: Arc<dyn Cache>,
    ttl: DocCacheTtl,
}

fn normalize_version(version: Option<&str>) -> Option<String> {
    version.map(|v| v.trim().to_lowercase())
}

fn is_valid_crate_name_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'-'
}

fn is_valid_item_path_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'-' || b == b':'
}

fn is_valid_crate_name(name: &str) -> bool {
    !name.is_empty() && name.bytes().all(is_valid_crate_name_char)
}

fn is_valid_item_path(path: &str) -> bool {
    !path.is_empty() && path.bytes().all(is_valid_item_path_char)
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
    /// let ttl = DocCacheTtl {
    ///     crate_docs_secs: 7200,
    ///     search_results_secs: 600,
    ///     item_docs_secs: 3600,
    ///     jitter_ratio: 0.1,
    /// };
    /// let doc_cache = DocCache::with_ttl(cache, ttl);
    /// ```
    #[must_use]
    pub fn with_ttl(cache: Arc<dyn Cache>, ttl: DocCacheTtl) -> Self {
        Self { cache, ttl }
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
        let key = Self::crate_cache_key(crate_name, version);
        self.cache.get(&key).await
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
        let key = Self::crate_cache_key(crate_name, version);
        let ttl = Duration::from_secs(self.ttl.apply_jitter(self.ttl.crate_docs_secs));
        self.cache.set(key, content, Some(ttl)).await
    }

    /// Get cached search results
    ///
    /// # Arguments
    ///
    /// * `query` - Search query
    /// * `limit` - Result count limit
    ///
    /// # Returns
    ///
    /// Returns search results if cache hit;otherwise returns `None`
    pub async fn get_search_results(&self, query: &str, limit: u32) -> Option<String> {
        let key = Self::search_cache_key(query, limit);
        self.cache.get(&key).await
    }

    /// Set search results cache
    ///
    /// # Arguments
    ///
    /// * `query` - Search query
    /// * `limit` - Result count limit
    /// * `content` - search result content
    ///
    /// # Errors
    ///
    /// Returns error if cache operation fails
    pub async fn set_search_results(
        &self,
        query: &str,
        limit: u32,
        content: String,
    ) -> crate::error::Result<()> {
        let key = Self::search_cache_key(query, limit);
        let ttl = Duration::from_secs(self.ttl.apply_jitter(self.ttl.search_results_secs));
        self.cache.set(key, content, Some(ttl)).await
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
        let key = Self::item_cache_key(crate_name, item_path, version);
        self.cache.get(&key).await
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
        let key = Self::item_cache_key(crate_name, item_path, version);
        let ttl = Duration::from_secs(self.ttl.apply_jitter(self.ttl.item_docs_secs));
        self.cache.set(key, content, Some(ttl)).await
    }

    /// Clear cache
    ///
    /// # Errors
    ///
    /// Returns error if cache operation fails
    pub async fn clear(&self) -> crate::error::Result<()> {
        self.cache.clear().await
    }

    /// Build crate cache key with normalization
    ///
    /// # Normalization rules
    ///
    /// - `crate_name`: lowercase
    /// - `version`: lowercase, trimmed
    /// - Invalid characters in `crate_name` (non-alphanumeric, non-underscore, non-hyphen)
    ///   will result in a hashed key to prevent injection
    fn crate_cache_key(crate_name: &str, version: Option<&str>) -> String {
        let normalized_name = crate_name.to_lowercase();

        if !is_valid_crate_name(&normalized_name) {
            let mut hasher = DefaultHasher::new();
            normalized_name.hash(&mut hasher);
            let hash = hasher.finish();
            return match normalize_version(version) {
                Some(normalized_ver) => format!("crate:hash:{hash}:{normalized_ver}"),
                None => format!("crate:hash:{hash}"),
            };
        }

        match normalize_version(version) {
            Some(normalized_ver) => format!("crate:{normalized_name}:{normalized_ver}"),
            None => format!("crate:{normalized_name}"),
        }
    }

    /// Build search cache key with normalization
    ///
    /// # Normalization rules
    ///
    /// - query: lowercase, trimmed (search is case-insensitive)
    fn search_cache_key(query: &str, limit: u32) -> String {
        let normalized_query = query.trim().to_lowercase();
        format!("search:{normalized_query}:{limit}")
    }

    /// Build item cache key with normalization
    ///
    /// # Normalization rules
    ///
    /// - `crate_name`: lowercase
    /// - `item_path`: trimmed but case-sensitive (Rust paths are case-sensitive)
    /// - `version`: lowercase, trimmed
    fn item_cache_key(crate_name: &str, item_path: &str, version: Option<&str>) -> String {
        let normalized_name = crate_name.to_lowercase();
        let normalized_path = item_path.trim();

        if !is_valid_crate_name(&normalized_name) || !is_valid_item_path(normalized_path) {
            let mut hasher = DefaultHasher::new();
            normalized_name.hash(&mut hasher);
            normalized_path.hash(&mut hasher);
            let hash = hasher.finish();
            return match normalize_version(version) {
                Some(normalized_ver) => {
                    format!("item:{normalized_name}:{normalized_ver}:hash:{hash}")
                }
                None => format!("item:{normalized_name}:hash:{hash}"),
            };
        }

        match normalize_version(version) {
            Some(normalized_ver) => {
                format!("item:{normalized_name}:{normalized_ver}:{normalized_path}")
            }
            None => format!("item:{normalized_name}:{normalized_path}"),
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

        // Test crate document cache
        doc_cache
            .set_crate_docs("serde", Some("1.0"), "Test docs".to_string())
            .await
            .expect("set_crate_docs should succeed");
        let cached = doc_cache.get_crate_docs("serde", Some("1.0")).await;
        assert_eq!(cached, Some("Test docs".to_string()));

        // Test search results cache
        doc_cache
            .set_search_results("web framework", 10, "Search results".to_string())
            .await
            .expect("set_search_results should succeed");
        let search_cached = doc_cache.get_search_results("web framework", 10).await;
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
    fn test_cache_key_normalization_case_insensitivity() {
        assert_eq!(
            DocCache::crate_cache_key("Serde", None),
            DocCache::crate_cache_key("serde", None)
        );
        assert_eq!(
            DocCache::crate_cache_key("SERDE", None),
            DocCache::crate_cache_key("serde", None)
        );

        assert_eq!(
            DocCache::crate_cache_key("Tokio", Some("1.0")),
            DocCache::crate_cache_key("tokio", Some("1.0"))
        );
        assert_eq!(
            DocCache::crate_cache_key("tokio", Some("1.0")),
            DocCache::crate_cache_key("tokio", Some("1.0"))
        );

        assert_eq!(
            DocCache::search_cache_key("Web Framework", 10),
            DocCache::search_cache_key("web framework", 10)
        );
        assert_eq!(
            DocCache::search_cache_key("ASYNC", 20),
            DocCache::search_cache_key("async", 20)
        );

        assert_eq!(
            DocCache::item_cache_key("Serde", "Serialize", None),
            DocCache::item_cache_key("serde", "Serialize", None)
        );
        assert_eq!(
            DocCache::item_cache_key("TOKIO", "runtime::Runtime", Some("1.0")),
            DocCache::item_cache_key("tokio", "runtime::Runtime", Some("1.0"))
        );
    }

    #[test]
    fn test_cache_key_normalization_whitespace() {
        assert_eq!(
            DocCache::crate_cache_key("serde", Some(" 1.0 ")),
            "crate:serde:1.0"
        );

        assert_eq!(
            DocCache::search_cache_key("  web framework  ", 10),
            "search:web framework:10"
        );

        assert_eq!(
            DocCache::item_cache_key("serde", "  Serialize  ", None),
            "item:serde:Serialize"
        );
    }

    #[test]
    fn test_cache_key_normalization_version_case() {
        assert_eq!(
            DocCache::crate_cache_key("serde", Some("1.0-RC1")),
            "crate:serde:1.0-rc1"
        );
        assert_eq!(
            DocCache::item_cache_key("serde", "Serialize", Some("V1.0")),
            "item:serde:v1.0:Serialize"
        );
    }

    #[test]
    fn test_cache_key_injection_prevention() {
        let malicious_key = DocCache::crate_cache_key("serde:malicious", None);
        assert!(malicious_key.starts_with("crate:hash:"));
        assert!(!malicious_key.contains("serde:malicious"));

        let malicious_key_with_version = DocCache::crate_cache_key("crate:evil", Some("1.0"));
        assert!(malicious_key_with_version.starts_with("crate:hash:"));
        assert!(!malicious_key_with_version.contains("crate:evil"));

        let valid_key = DocCache::crate_cache_key("serde-json", None);
        assert_eq!(valid_key, "crate:serde-json");

        let valid_key_underscore = DocCache::crate_cache_key("my_crate", None);
        assert_eq!(valid_key_underscore, "crate:my_crate");

        let valid_key_alphanumeric = DocCache::crate_cache_key("crate123", None);
        assert_eq!(valid_key_alphanumeric, "crate:crate123");
    }

    #[test]
    fn test_item_path_case_sensitivity() {
        assert_ne!(
            DocCache::item_cache_key("serde", "Serialize", None),
            DocCache::item_cache_key("serde", "serialize", None)
        );

        assert_eq!(
            DocCache::item_cache_key("serde", "Serialize", None),
            "item:serde:Serialize"
        );
        assert_eq!(
            DocCache::item_cache_key("serde", "serialize", None),
            "item:serde:serialize"
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

    #[test]
    fn test_cache_key_edge_cases() {
        let empty_key = DocCache::crate_cache_key("", None);
        assert!(empty_key.starts_with("crate:hash:"));

        let empty_with_version = DocCache::crate_cache_key("", Some("1.0"));
        assert!(empty_with_version.starts_with("crate:hash:"));
        assert!(empty_with_version.contains("1.0"));

        let whitespace_key = DocCache::crate_cache_key("   ", None);
        assert!(whitespace_key.starts_with("crate:hash:"));
        assert!(!whitespace_key.contains("   "));

        assert_eq!(DocCache::crate_cache_key("serde", Some("")), "crate:serde:");
        assert_eq!(
            DocCache::crate_cache_key("serde", Some("   ")),
            "crate:serde:"
        );

        let unicode_key = DocCache::crate_cache_key("serde测试", None);
        assert!(unicode_key.starts_with("crate:hash:"));
        assert!(!unicode_key.contains("测试"));

        let unicode_version = DocCache::crate_cache_key("serde", Some("测试版本"));
        assert!(unicode_version.contains("serde"));
        assert!(unicode_version.contains(":测试版本"));

        let malicious_item_path = DocCache::item_cache_key("serde", "Serialize\nmalicious", None);
        assert!(malicious_item_path.contains("hash:"));
        assert!(!malicious_item_path.contains('\n'));

        let malicious_item_colon =
            DocCache::item_cache_key("serde", "Serialize:extra:colons", None);
        assert_eq!(malicious_item_colon, "item:serde:Serialize:extra:colons");

        let valid_item_path = DocCache::item_cache_key("serde", "serde::Serialize", None);
        assert_eq!(valid_item_path, "item:serde:serde::Serialize");

        let item_with_unicode = DocCache::item_cache_key("serde", "Serialize测试", None);
        assert!(item_with_unicode.contains("hash:"));
        assert!(!item_with_unicode.contains("测试"));

        let empty_item_key = DocCache::item_cache_key("serde", "", None);
        assert!(empty_item_key.contains("hash:"));

        let empty_item_crate = DocCache::item_cache_key("", "Serialize", None);
        assert!(empty_item_crate.contains("hash:"));

        let long_name = "x".repeat(1000);
        let long_key = DocCache::crate_cache_key(&long_name, None);
        assert!(long_key.starts_with("crate:"));
        assert_eq!(long_key.len(), 1000 + 6);

        let long_item_path = "module::".repeat(100);
        let long_item_key = DocCache::item_cache_key("serde", &long_item_path, None);
        assert!(long_item_key.starts_with("item:serde:"));
    }
}
