//! 文档查询工具模块

pub mod lookup;
pub mod search;
pub mod cache;

use crate::cache::Cache;
use std::sync::Arc;

/// 文档服务
pub struct DocService {
    client: reqwest::Client,
    cache: Arc<dyn Cache>,
    doc_cache: cache::DocCache,
}

impl DocService {
    /// 创建新的文档服务
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
    
    /// 获取 HTTP 客户端
    #[must_use]
    pub fn client(&self) -> &reqwest::Client {
        &self.client
    }
    
    /// 获取缓存
    #[must_use]
    pub fn cache(&self) -> &Arc<dyn Cache> {
        &self.cache
    }
    
    /// 获取文档缓存
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
pub use search::SearchCratesTool;
pub use lookup::LookupItemTool;