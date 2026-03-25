//! 文档缓存模块
//!
//! 提供文档专用的缓存服务，支持 crate 文档、搜索结果和项目文档的独立 TTL 配置。
//!
//! # 缓存键格式
//!
//! - Crate 文档: `crate:{name}` 或 `crate:{name}:{version}`
//! - 搜索结果: `search:{query}:{limit}`
//! - 项目文档: `item:{crate}:{path}` 或 `item:{crate}:{version}:{path}`
//!
//! # 示例
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
use std::sync::Arc;
use std::time::Duration;

/// 文档缓存 TTL 配置
///
/// 为不同类型的文档配置独立的 TTL。
///
/// # 字段
///
/// - `crate_docs_secs`: crate 文档缓存时间（秒）
/// - `search_results_secs`: 搜索结果缓存时间（秒）
/// - `item_docs_secs`: 项目文档缓存时间（秒）
/// - `jitter_ratio`: TTL 抖动比例（0.0-1.0），用于防止缓存雪崩
#[derive(Debug, Clone, Copy)]
pub struct DocCacheTtl {
    /// crate 文档 TTL（秒）
    pub crate_docs_secs: u64,
    /// 搜索结果 TTL（秒）
    pub search_results_secs: u64,
    /// 项目文档 TTL（秒）
    pub item_docs_secs: u64,
    /// TTL 抖动比例（0.0-1.0），默认 0.1（10%）
    ///
    /// 实际 TTL = `base_ttl * (1 + random(-jitter_ratio, jitter_ratio))`
    /// 例如：`base_ttl=3600`, `jitter_ratio=0.1` => 实际 TTL 范围 `[3240, 3960]`
    pub jitter_ratio: f64,
}

/// 默认 TTL 抖动比例（10%）
const DEFAULT_JITTER_RATIO: f64 = 0.1;

impl Default for DocCacheTtl {
    fn default() -> Self {
        Self {
            crate_docs_secs: 3600,    // 1 小时
            search_results_secs: 300, // 5 分钟
            item_docs_secs: 1800,     // 30 分钟
            jitter_ratio: DEFAULT_JITTER_RATIO,
        }
    }
}

impl DocCacheTtl {
    /// 从 `CacheConfig` 创建 TTL 配置
    ///
    /// # 参数
    ///
    /// * `config` - 缓存配置
    ///
    /// # 返回值
    ///
    /// 返回根据配置创建的 TTL 配置
    #[must_use]
    pub fn from_cache_config(config: &crate::cache::CacheConfig) -> Self {
        Self {
            crate_docs_secs: config.crate_docs_ttl_secs.unwrap_or(3600),
            search_results_secs: config.search_results_ttl_secs.unwrap_or(300),
            item_docs_secs: config.item_docs_ttl_secs.unwrap_or(1800),
            jitter_ratio: DEFAULT_JITTER_RATIO,
        }
    }

    /// 计算带抖动的实际 TTL
    ///
    /// # 参数
    ///
    /// * `base_ttl` - 基础 TTL（秒）
    ///
    /// # 返回值
    ///
    /// 返回带抖动的实际 TTL（秒）
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    #[allow(clippy::cast_sign_loss)]
    #[allow(clippy::cast_precision_loss)]
    pub fn apply_jitter(&self, base_ttl: u64) -> u64 {
        if self.jitter_ratio <= 0.0 {
            return base_ttl;
        }

        // 限制 jitter_ratio 在 [0.0, 1.0] 范围内
        let ratio = self.jitter_ratio.clamp(0.0, 1.0);

        // 生成 [-ratio, +ratio] 范围内的随机偏移
        let rng = fastrand::f64();
        let offset = (rng * 2.0 - 1.0) * ratio;

        // 计算抖动后的 TTL，确保至少为 1 秒
        (base_ttl as f64 * (1.0 + offset)).max(1.0) as u64
    }
}

/// 文档缓存服务
///
/// 提供文档专用的缓存操作，支持 crate 文档、搜索结果和项目文档。
///
/// # 字段
///
/// - `cache`: 底层缓存实例
/// - `ttl`: TTL 配置
#[derive(Clone)]
pub struct DocCache {
    cache: Arc<dyn Cache>,
    ttl: DocCacheTtl,
}

impl DocCache {
    /// 创建新的文档缓存（使用默认 TTL）
    ///
    /// # 参数
    ///
    /// * `cache` - 缓存实例
    ///
    /// # 示例
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

    /// 创建新的文档缓存（使用自定义 TTL）
    ///
    /// # 参数
    ///
    /// * `cache` - 缓存实例
    /// * `ttl` - TTL 配置
    ///
    /// # 示例
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

    /// 获取缓存的 crate 文档
    ///
    /// # 参数
    ///
    /// * `crate_name` - crate 名称
    /// * `version` - 可选的版本号
    ///
    /// # 返回值
    ///
    /// 如果缓存命中，返回文档内容；否则返回 `None`
    pub async fn get_crate_docs(&self, crate_name: &str, version: Option<&str>) -> Option<String> {
        let key = Self::crate_cache_key(crate_name, version);
        self.cache.get(&key).await
    }

    /// 设置 crate 文档缓存
    ///
    /// # 参数
    ///
    /// * `crate_name` - crate 名称
    /// * `version` - 可选的版本号
    /// * `content` - 文档内容
    ///
    /// # 错误
    ///
    /// 如果缓存操作失败，返回错误
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

    /// 获取缓存的搜索结果
    ///
    /// # 参数
    ///
    /// * `query` - 搜索查询
    /// * `limit` - 结果数量限制
    ///
    /// # 返回值
    ///
    /// 如果缓存命中，返回搜索结果；否则返回 `None`
    pub async fn get_search_results(&self, query: &str, limit: u32) -> Option<String> {
        let key = Self::search_cache_key(query, limit);
        self.cache.get(&key).await
    }

    /// 设置搜索结果缓存
    ///
    /// # 参数
    ///
    /// * `query` - 搜索查询
    /// * `limit` - 结果数量限制
    /// * `content` - 搜索结果内容
    ///
    /// # 错误
    ///
    /// 如果缓存操作失败，返回错误
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

    /// 获取缓存的项目文档
    ///
    /// # 参数
    ///
    /// * `crate_name` - crate 名称
    /// * `item_path` - 项目路径
    /// * `version` - 可选的版本号
    ///
    /// # 返回值
    ///
    /// 如果缓存命中，返回项目文档；否则返回 `None`
    pub async fn get_item_docs(
        &self,
        crate_name: &str,
        item_path: &str,
        version: Option<&str>,
    ) -> Option<String> {
        let key = Self::item_cache_key(crate_name, item_path, version);
        self.cache.get(&key).await
    }

    /// 设置项目文档缓存
    ///
    /// # 参数
    ///
    /// * `crate_name` - crate 名称
    /// * `item_path` - 项目路径
    /// * `version` - 可选的版本号
    /// * `content` - 文档内容
    ///
    /// # 错误
    ///
    /// 如果缓存操作失败，返回错误
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

    /// 清除缓存
    ///
    /// # 错误
    ///
    /// 如果缓存操作失败，返回错误
    pub async fn clear(&self) -> crate::error::Result<()> {
        self.cache.clear().await
    }

    /// 构建 crate 缓存键
    fn crate_cache_key(crate_name: &str, version: Option<&str>) -> String {
        if let Some(ver) = version {
            format!("crate:{crate_name}:{ver}")
        } else {
            format!("crate:{crate_name}")
        }
    }

    /// 构建搜索缓存键
    fn search_cache_key(query: &str, limit: u32) -> String {
        format!("search:{query}:{limit}")
    }

    /// 构建项目缓存键
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
