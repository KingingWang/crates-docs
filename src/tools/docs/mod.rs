//! Document query tools module

pub mod cache;
pub mod html;
pub mod lookup_crate;
pub mod lookup_item;
pub mod search;

use crate::cache::{Cache, CacheConfig};
use std::sync::Arc;

/// Document service
pub struct DocService {
    client: reqwest::Client,
    cache: Arc<dyn Cache>,
    doc_cache: cache::DocCache,
}

impl DocService {
    /// Create a new document service with default TTL
    ///
    /// # Errors
    ///
    /// Returns an error if the HTTP client cannot be created
    pub fn new(cache: Arc<dyn Cache>) -> crate::error::Result<Self> {
        Self::with_config(cache, &CacheConfig::default())
    }

    /// Create a new document service with custom cache configuration
    ///
    /// # Errors
    ///
    /// Returns an error if the HTTP client cannot be created
    pub fn with_config(
        cache: Arc<dyn Cache>,
        cache_config: &CacheConfig,
    ) -> crate::error::Result<Self> {
        let ttl = cache::DocCacheTtl::from_cache_config(cache_config);
        let doc_cache = cache::DocCache::with_ttl(cache.clone(), ttl);
        let client = reqwest::Client::builder()
            .user_agent(format!("CratesDocsMCP/{}", crate::VERSION))
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| {
                crate::error::Error::Initialization(format!("Failed to create HTTP client: {e}"))
            })?;
        Ok(Self {
            client,
            cache,
            doc_cache,
        })
    }

    /// Get HTTP client
    #[must_use]
    pub fn client(&self) -> &reqwest::Client {
        &self.client
    }

    /// Get cache
    #[must_use]
    pub fn cache(&self) -> &Arc<dyn Cache> {
        &self.cache
    }

    /// Get document cache
    #[must_use]
    pub fn doc_cache(&self) -> &cache::DocCache {
        &self.doc_cache
    }
}

impl Default for DocService {
    fn default() -> Self {
        let cache = Arc::new(crate::cache::memory::MemoryCache::new(1000));
        Self::new(cache).expect("Failed to create default DocService")
    }
}

/// Re-export tools
pub use lookup_crate::LookupCrateTool;
pub use lookup_item::LookupItemTool;
pub use search::SearchCratesTool;

/// Re-export cache types
pub use cache::DocCacheTtl;
