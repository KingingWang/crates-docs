//! 文档查询工具模块
//!
//! 提供用于查询 Rust crate 文档的工具和服务。
//!
//! # 子模块
//!
//! - `cache`: 文档缓存
//! - `html`: HTML 处理
//! - `lookup_crate`: Crate 文档查找
//! - `lookup_item`: 项目文档查找
//! - `search`: Crate 搜索
//!
//! # 示例
//!
//! ```rust,no_run
//! use std::sync::Arc;
//! use crates_docs::tools::docs::DocService;
//! use crates_docs::cache::memory::MemoryCache;
//!
//! let cache = Arc::new(MemoryCache::new(1000));
//! let service = DocService::new(cache).expect("Failed to create DocService");
//! ```

pub mod cache;
pub mod html;
pub mod lookup_crate;
pub mod lookup_item;
pub mod search;

use crate::cache::{Cache, CacheConfig};
use crate::config::PerformanceConfig;
use std::sync::Arc;

/// 文档服务
///
/// 提供 HTTP 客户端（带自动重试）、缓存和文档缓存的集中管理。
///
/// # 字段
///
/// - `client`: 带重试中间件的 HTTP 客户端（共享引用以实现连接池复用）
/// - `cache`: 通用缓存实例
/// - `doc_cache`: 文档专用缓存
pub struct DocService {
    client: Arc<reqwest_middleware::ClientWithMiddleware>,
    cache: Arc<dyn Cache>,
    doc_cache: cache::DocCache,
}

impl DocService {
    /// 创建新的文档服务（使用默认 TTL）
    ///
    /// # 参数
    ///
    /// * `cache` - 缓存实例
    ///
    /// # 错误
    ///
    /// 如果 HTTP 客户端创建失败，返回错误
    ///
    /// # 示例
    ///
    /// ```rust,no_run
    /// use std::sync::Arc;
    /// use crates_docs::tools::docs::DocService;
    /// use crates_docs::cache::memory::MemoryCache;
    ///
    /// let cache = Arc::new(MemoryCache::new(1000));
    /// let service = DocService::new(cache).expect("Failed to create DocService");
    /// ```
    ///
    /// # Note
    ///
    /// This method uses the global HTTP client singleton for connection pool reuse.
    /// Make sure to call `init_global_http_client()` during server initialization
    /// for optimal performance.
    pub fn new(cache: Arc<dyn Cache>) -> crate::error::Result<Self> {
        Self::with_config(cache, &CacheConfig::default())
    }

    /// 创建新的文档服务（使用自定义缓存配置）
    ///
    /// # 参数
    ///
    /// * `cache` - 缓存实例
    /// * `cache_config` - 缓存配置
    ///
    /// # 错误
    ///
    /// 如果 HTTP 客户端创建失败，返回错误
    ///
    /// # Note
    ///
    /// This method uses the global HTTP client singleton for connection pool reuse.
    /// If the global client is not initialized, it will be initialized with default config.
    pub fn with_config(
        cache: Arc<dyn Cache>,
        cache_config: &CacheConfig,
    ) -> crate::error::Result<Self> {
        let ttl = cache::DocCacheTtl::from_cache_config(cache_config);
        let doc_cache = cache::DocCache::with_ttl(cache.clone(), ttl);
        // Use global HTTP client singleton for connection pool reuse
        let client = crate::utils::get_or_init_global_http_client()?;
        Ok(Self {
            client,
            cache,
            doc_cache,
        })
    }

    /// 创建新的文档服务（使用完整配置）
    ///
    /// # 参数
    ///
    /// * `cache` - 缓存实例
    /// * `cache_config` - 缓存配置
    /// * `perf_config` - 性能配置（仅用于初始化全局 HTTP 客户端，如果尚未初始化）
    ///
    /// # 错误
    ///
    /// 如果 HTTP 客户端创建失败，返回错误
    ///
    /// # Note
    ///
    /// This method uses the global HTTP client singleton for connection pool reuse.
    /// The `perf_config` is used only if the global client hasn't been initialized yet.
    /// For consistent configuration, call `init_global_http_client()` during server startup.
    pub fn with_full_config(
        cache: Arc<dyn Cache>,
        cache_config: &CacheConfig,
        _perf_config: &PerformanceConfig,
    ) -> crate::error::Result<Self> {
        let ttl = cache::DocCacheTtl::from_cache_config(cache_config);
        let doc_cache = cache::DocCache::with_ttl(cache.clone(), ttl);
        // Use global HTTP client singleton for connection pool reuse
        let client = crate::utils::get_or_init_global_http_client()?;
        Ok(Self {
            client,
            cache,
            doc_cache,
        })
    }

    /// 获取 HTTP 客户端（带重试中间件）
    #[must_use]
    pub fn client(&self) -> &reqwest_middleware::ClientWithMiddleware {
        &self.client
    }

    /// 获取缓存实例
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
        Self::new(cache).expect("Failed to create default DocService")
    }
}

/// 重新导出工具类型
pub use lookup_crate::LookupCrateTool;
pub use lookup_item::LookupItemTool;
pub use search::SearchCratesTool;

/// 重新导出缓存类型
pub use cache::DocCacheTtl;
