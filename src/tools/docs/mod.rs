//! Document query tools module

pub mod cache;
pub mod lookup;
pub mod search;

use crate::cache::Cache;
use std::sync::Arc;

/// Document service
pub struct DocService {
    client: reqwest::Client,
    cache: Arc<dyn Cache>,
    doc_cache: cache::DocCache,
}

impl DocService {
    /// Create a new document service
    pub fn new(cache: Arc<dyn Cache>) -> Self {
        let doc_cache = cache::DocCache::new(cache.clone());
        Self {
            client: reqwest::Client::builder()
                .user_agent(format!("CratesDocsMCP/{}", crate::VERSION))
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("Failed to create HTTP client"),
            cache,
            doc_cache,
        }
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
        Self::new(cache)
    }
}

/// 重新导出工具
pub use lookup::LookupCrateTool;
pub use lookup::LookupItemTool;
pub use search::SearchCratesTool;
