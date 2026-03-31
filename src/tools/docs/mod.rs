//! Document lookup tool module
//!
//! Provides tools and services for querying Rust crate documentation.
//!
//! # Submodules
//!
//! - `cache`: Document cache
//! - `html`: HTML processing
//! - `lookup_crate`: Crate documentation lookup
//! - `lookup_item`: Item documentation lookup
//! - `search`: Crate search
//!
//! # Examples
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
use rust_mcp_sdk::schema::CallToolError;
use std::sync::Arc;

#[cfg(not(test))]
const DOCS_RS_BASE_URL: &str = "https://docs.rs";

#[cfg(not(test))]
const CRATES_IO_BASE_URL: &str = "https://crates.io";

#[must_use]
#[cfg(test)]
/// Get the docs.rs base URL (configurable via environment variable for testing)
pub fn docs_rs_base_url() -> String {
    std::env::var("CRATES_DOCS_DOCS_RS_URL").unwrap_or_else(|_| "https://docs.rs".to_string())
}

#[must_use]
#[cfg(not(test))]
/// Get the docs.rs base URL
pub fn docs_rs_base_url() -> String {
    DOCS_RS_BASE_URL.to_string()
}

#[must_use]
#[cfg(test)]
/// Get the crates.io base URL (configurable via environment variable for testing)
pub fn crates_io_base_url() -> String {
    std::env::var("CRATES_DOCS_CRATES_IO_URL").unwrap_or_else(|_| "https://crates.io".to_string())
}

#[must_use]
#[cfg(not(test))]
/// Get the crates.io base URL
pub fn crates_io_base_url() -> String {
    CRATES_IO_BASE_URL.to_string()
}

/// Document service
///
/// Provides centralized management of HTTP client (with auto-retry), cache, and document cache.
///
/// # Fields
///
/// - `client`: HTTP client with retry middleware (shared reference for connection pool reuse)
/// - `cache`: Generic cache instance
/// - `doc_cache`: Document-specific cache
pub struct DocService {
    client: Arc<reqwest_middleware::ClientWithMiddleware>,
    cache: Arc<dyn Cache>,
    doc_cache: cache::DocCache,
}

impl DocService {
    /// Create new document service (with default TTL)
    ///
    /// # Arguments
    ///
    /// * `cache` - cache instance
    ///
    /// # Errors
    ///
    /// Returns error if HTTP client creation fails
    ///
    /// # Examples
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

    /// Create new document service (with custom cache config)
    ///
    /// # Arguments
    ///
    /// * `cache` - cache instance
    /// * `cache_config` - cache configuration
    ///
    /// # Errors
    ///
    /// Returns error if HTTP client creation fails
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

    /// Create new document service (with full config)
    ///
    /// # Arguments
    ///
    /// * `cache` - cache instance
    /// * `cache_config` - cache configuration
    /// * `perf_config` - performance configuration(used only for initializing global HTTP client if not yet initialized)
    ///
    /// # Errors
    ///
    /// Returns error if HTTP client creation fails
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

    /// Get HTTP client (with retry middleware)
    #[must_use]
    pub fn client(&self) -> &reqwest_middleware::ClientWithMiddleware {
        &self.client
    }

    /// Get cache instance
    #[must_use]
    pub fn cache(&self) -> &Arc<dyn Cache> {
        &self.cache
    }

    /// Get document cache
    #[must_use]
    pub fn doc_cache(&self) -> &cache::DocCache {
        &self.doc_cache
    }

    /// Fetch HTML content from a URL
    ///
    /// This is a shared utility method used by multiple tools to fetch HTML
    /// from docs.rs and crates.io.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL to fetch
    /// * `tool_name` - Optional tool name for better error messages (e.g., "`lookup_crate`", "`lookup_item`")
    ///
    /// # Errors
    ///
    /// Returns a `CallToolError` if:
    /// - The HTTP request fails
    /// - The response status is not successful
    /// - Reading the response body fails
    pub async fn fetch_html(
        &self,
        url: &str,
        tool_name: Option<&str>,
    ) -> Result<String, CallToolError> {
        let response = self.client.get(url).send().await.map_err(|e| {
            let prefix = tool_name.map_or(String::new(), |n| format!("[{n}] "));
            CallToolError::from_message(format!("{prefix}HTTP request failed: {e}"))
        })?;

        let status = response.status();
        if !status.is_success() {
            let error_body = response.text().await.map_err(|e| {
                let prefix = tool_name.map_or(String::new(), |n| format!("[{n}] "));
                CallToolError::from_message(format!("{prefix}Failed to read error response: {e}"))
            })?;
            let prefix = tool_name.map_or(String::new(), |n| format!("[{n}] "));
            return Err(CallToolError::from_message(format!(
                "{prefix}Failed to get documentation: HTTP {} - {}",
                status,
                if error_body.is_empty() {
                    "No error details"
                } else {
                    &error_body
                }
            )));
        }

        response.text().await.map_err(|e| {
            let prefix = tool_name.map_or(String::new(), |n| format!("[{n}] "));
            CallToolError::from_message(format!("{prefix}Failed to read response: {e}"))
        })
    }
}

impl Default for DocService {
    fn default() -> Self {
        let cache = Arc::new(crate::cache::memory::MemoryCache::new(1000));
        Self::new(cache).expect("Failed to create default DocService")
    }
}

/// Re-export tool types
pub use lookup_crate::LookupCrateTool;
pub use lookup_item::LookupItemTool;
pub use search::SearchCratesTool;

/// Re-export cache types
pub use cache::DocCacheTtl;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_doc_service_default() {
        let service = DocService::default();
        let _ = service.client();
        // HTTP client is always available after service creation
    }

    #[test]
    fn test_doc_service_accessors() {
        let service = DocService::default();
        let _ = service.client();
        let _ = service.client();
        let _ = service.cache();
        let _ = service.doc_cache();
    }
}
