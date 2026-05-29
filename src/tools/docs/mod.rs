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

/// Output format for documentation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Format {
    /// Markdown format
    #[default]
    Markdown,
    /// Plain text format
    Text,
    /// HTML format
    Html,
    /// JSON format (used by search tool)
    Json,
}

impl std::fmt::Display for Format {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Markdown => write!(f, "markdown"),
            Self::Text => write!(f, "text"),
            Self::Html => write!(f, "html"),
            Self::Json => write!(f, "json"),
        }
    }
}

/// Parse format string into Format enum
pub fn parse_format(format_str: Option<&str>) -> Result<Format, CallToolError> {
    match format_str {
        None => Ok(Format::Markdown),
        Some(s) => match s.to_lowercase().as_str() {
            "markdown" => Ok(Format::Markdown),
            "text" => Ok(Format::Text),
            "html" => Ok(Format::Html),
            "json" => Ok(Format::Json),
            _ => Err(CallToolError::invalid_arguments(
                "format",
                Some(format!(
                    "Invalid format '{s}'. Expected one of: markdown, text, html, json"
                )),
            )),
        },
    }
}

/// Validate a crate name supplied by a tool caller.
///
/// Crate names on crates.io are restricted to ASCII alphanumerics plus `_` and
/// `-`. Rejecting anything else early provides a clear error and prevents
/// malformed values (path separators, `..`, whitespace, control characters)
/// from being interpolated into docs.rs URLs.
///
/// # Errors
///
/// Returns a `CallToolError` describing the first problem found.
pub fn validate_crate_name(crate_name: &str) -> Result<(), CallToolError> {
    let name = crate_name.trim();
    if name.is_empty() {
        return Err(CallToolError::invalid_arguments(
            "crate_name",
            Some("crate_name must not be empty".to_string()),
        ));
    }
    if name.len() > 64 {
        return Err(CallToolError::invalid_arguments(
            "crate_name",
            Some("crate_name is too long (max 64 characters)".to_string()),
        ));
    }
    if !name
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        return Err(CallToolError::invalid_arguments(
            "crate_name",
            Some(format!(
                "Invalid crate_name '{crate_name}'. Only ASCII letters, digits, '_' and '-' are allowed"
            )),
        ));
    }
    Ok(())
}

/// Validate an optional version string supplied by a tool caller.
///
/// Accepts concrete versions and identifiers such as `latest` while rejecting
/// path-traversal sequences and characters that could escape the docs.rs path.
///
/// # Errors
///
/// Returns a `CallToolError` describing the first problem found.
pub fn validate_version(version: Option<&str>) -> Result<(), CallToolError> {
    let Some(raw) = version else {
        return Ok(());
    };
    let ver = raw.trim();
    if ver.is_empty() {
        return Err(CallToolError::invalid_arguments(
            "version",
            Some("version must not be empty when provided".to_string()),
        ));
    }
    if ver.len() > 64 {
        return Err(CallToolError::invalid_arguments(
            "version",
            Some("version is too long (max 64 characters)".to_string()),
        ));
    }
    if ver.contains("..")
        || !ver
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'+' | b'_' | b'~'))
    {
        return Err(CallToolError::invalid_arguments(
            "version",
            Some(format!(
                "Invalid version '{raw}'. Only ASCII letters, digits and '.', '-', '+', '_', '~' are allowed"
            )),
        ));
    }
    Ok(())
}

/// Validate a search query supplied by a tool caller.
///
/// Rejects empty/whitespace-only queries (which would otherwise trigger an
/// unfiltered crates.io request returning arbitrary crates) and overly long
/// queries that cannot represent a meaningful search.
///
/// # Errors
///
/// Returns a `CallToolError` describing the first problem found.
pub fn validate_search_query(query: &str) -> Result<(), CallToolError> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return Err(CallToolError::invalid_arguments(
            "query",
            Some("query must not be empty".to_string()),
        ));
    }
    if trimmed.len() > 200 {
        return Err(CallToolError::invalid_arguments(
            "query",
            Some("query is too long (max 200 characters)".to_string()),
        ));
    }
    Ok(())
}

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
/// Build docs.rs URL for crate documentation
#[must_use]
pub fn build_docs_url(crate_name: &str, version: Option<&str>) -> String {
    let base_url = docs_rs_base_url();
    match version {
        Some(ver) => format!("{base_url}/{crate_name}/{ver}/"),
        None => format!("{base_url}/{crate_name}/"),
    }
}

/// Build docs.rs search URL for item lookup
#[must_use]
pub fn build_docs_item_url(crate_name: &str, version: Option<&str>, item_path: &str) -> String {
    let base_url = docs_rs_base_url();
    let encoded_path = urlencoding::encode(item_path);
    match version {
        Some(ver) => format!("{base_url}/{crate_name}/{ver}/?search={encoded_path}"),
        None => format!("{base_url}/{crate_name}/?search={encoded_path}"),
    }
}

/// Build crates.io API search URL
#[must_use]
pub fn build_crates_io_search_url(query: &str, sort: Option<&str>, limit: Option<usize>) -> String {
    let base_url = crates_io_base_url();
    let sort = sort.unwrap_or("relevance");
    let limit = limit.unwrap_or(10);
    format!(
        "{}/api/v1/crates?q={}&per_page={}&sort={}",
        base_url,
        urlencoding::encode(query),
        limit,
        urlencoding::encode(sort)
    )
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

    /// Create new document service with custom HTTP client (for testing)
    #[must_use]
    pub fn with_custom_client(
        cache: Arc<dyn Cache>,
        cache_config: &CacheConfig,
        client: Arc<reqwest_middleware::ClientWithMiddleware>,
    ) -> Self {
        let ttl = cache::DocCacheTtl::from_cache_config(cache_config);
        let doc_cache = cache::DocCache::with_ttl(cache.clone(), ttl);
        Self {
            client,
            cache,
            doc_cache,
        }
    }
}

impl Default for DocService {
    fn default() -> Self {
        // Try to create with fallible initialization
        Self::try_default_with_fallback()
    }
}

impl DocService {
    /// Create `DocService` with default settings using fallible initialization
    ///
    /// This method attempts to create a fully configured HTTP client.
    /// If that fails, it falls back to a basic client without retry middleware.
    /// The fallback uses `Client::new()` which is infallible.
    fn try_default_with_fallback() -> Self {
        let cache = Arc::new(crate::cache::memory::MemoryCache::new(1000));
        let cache_config = CacheConfig::default();

        // Try to create client with full configuration (may fail in extreme cases)
        let client: Arc<reqwest_middleware::ClientWithMiddleware> =
            if let Ok(c) = crate::utils::HttpClientBuilder::new().build() {
                Arc::new(c)
            } else {
                // Fallback: create a minimal client without retry middleware
                // Using Client::new() which is infallible - never panics
                let plain_client = reqwest::Client::new();
                Arc::new(reqwest_middleware::ClientBuilder::new(plain_client).build())
            };

        let ttl = cache::DocCacheTtl::from_cache_config(&cache_config);
        let doc_cache = cache::DocCache::with_ttl(cache.clone(), ttl);

        Self {
            client,
            cache,
            doc_cache,
        }
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
    fn test_validate_crate_name_accepts_valid() {
        assert!(validate_crate_name("serde").is_ok());
        assert!(validate_crate_name("serde_json").is_ok());
        assert!(validate_crate_name("tracing-subscriber").is_ok());
        assert!(validate_crate_name("  tokio  ").is_ok());
    }

    #[test]
    fn test_validate_crate_name_rejects_invalid() {
        assert!(validate_crate_name("").is_err());
        assert!(validate_crate_name("   ").is_err());
        assert!(validate_crate_name("../etc/passwd").is_err());
        assert!(validate_crate_name("foo/bar").is_err());
        assert!(validate_crate_name("foo bar").is_err());
        assert!(validate_crate_name("foo;rm").is_err());
        assert!(validate_crate_name(&"a".repeat(65)).is_err());
    }

    #[test]
    fn test_validate_version_accepts_valid() {
        assert!(validate_version(None).is_ok());
        assert!(validate_version(Some("1.0.0")).is_ok());
        assert!(validate_version(Some("1.0.0-rc.1")).is_ok());
        assert!(validate_version(Some("1.0.0+build.5")).is_ok());
        assert!(validate_version(Some("latest")).is_ok());
        assert!(validate_version(Some("  1.2.3  ")).is_ok());
    }

    #[test]
    fn test_validate_version_rejects_invalid() {
        assert!(validate_version(Some("")).is_err());
        assert!(validate_version(Some("../../1.0")).is_err());
        assert!(validate_version(Some("1.0/2.0")).is_err());
        assert!(validate_version(Some("1.0 0")).is_err());
        assert!(validate_version(Some("..")).is_err());
        assert!(validate_version(Some(&"1".repeat(65))).is_err());
    }

    #[test]
    fn test_validate_search_query_accepts_valid() {
        assert!(validate_search_query("serde").is_ok());
        assert!(validate_search_query("web framework").is_ok());
        assert!(validate_search_query("  tokio  ").is_ok());
        assert!(validate_search_query(&"a".repeat(200)).is_ok());
    }

    #[test]
    fn test_validate_search_query_rejects_invalid() {
        assert!(validate_search_query("").is_err());
        assert!(validate_search_query("   ").is_err());
        assert!(validate_search_query(&"a".repeat(201)).is_err());
    }

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

    #[test]
    fn test_parse_format_none() {
        assert_eq!(parse_format(None).unwrap(), Format::Markdown);
    }

    #[test]
    fn test_parse_format_markdown() {
        assert_eq!(parse_format(Some("markdown")).unwrap(), Format::Markdown);
        assert_eq!(parse_format(Some("MARKDOWN")).unwrap(), Format::Markdown);
        assert_eq!(parse_format(Some("Markdown")).unwrap(), Format::Markdown);
    }

    #[test]
    fn test_parse_format_text() {
        assert_eq!(parse_format(Some("text")).unwrap(), Format::Text);
        assert_eq!(parse_format(Some("TEXT")).unwrap(), Format::Text);
    }

    #[test]
    fn test_parse_format_html() {
        assert_eq!(parse_format(Some("html")).unwrap(), Format::Html);
        assert_eq!(parse_format(Some("HTML")).unwrap(), Format::Html);
    }

    #[test]
    fn test_parse_format_json() {
        assert_eq!(parse_format(Some("json")).unwrap(), Format::Json);
        assert_eq!(parse_format(Some("JSON")).unwrap(), Format::Json);
    }

    #[test]
    fn test_parse_format_invalid() {
        assert!(parse_format(Some("invalid")).is_err());
        assert!(parse_format(Some("xml")).is_err());
        assert!(parse_format(Some("")).is_err());
    }

    #[test]
    fn test_format_display() {
        assert_eq!(Format::Markdown.to_string(), "markdown");
        assert_eq!(Format::Text.to_string(), "text");
        assert_eq!(Format::Html.to_string(), "html");
        assert_eq!(Format::Json.to_string(), "json");
    }

    #[test]
    fn test_format_default() {
        assert_eq!(Format::default(), Format::Markdown);
    }

    // URL building tests
    #[test]
    fn test_build_docs_url_without_version() {
        std::env::set_var("CRATES_DOCS_DOCS_RS_URL", "https://docs.rs");
        let url = build_docs_url("serde", None);
        assert_eq!(url, "https://docs.rs/serde/");
        std::env::remove_var("CRATES_DOCS_DOCS_RS_URL");
    }

    #[test]
    fn test_build_docs_url_with_version() {
        std::env::set_var("CRATES_DOCS_DOCS_RS_URL", "https://docs.rs");
        let url = build_docs_url("serde", Some("1.0.0"));
        assert_eq!(url, "https://docs.rs/serde/1.0.0/");
        std::env::remove_var("CRATES_DOCS_DOCS_RS_URL");
    }

    #[test]
    fn test_build_docs_item_url_without_version() {
        std::env::set_var("CRATES_DOCS_DOCS_RS_URL", "https://docs.rs");
        let url = build_docs_item_url("serde", None, "Serialize");
        assert_eq!(url, "https://docs.rs/serde/?search=Serialize");
        std::env::remove_var("CRATES_DOCS_DOCS_RS_URL");
    }

    #[test]
    fn test_build_docs_item_url_with_version() {
        std::env::set_var("CRATES_DOCS_DOCS_RS_URL", "https://docs.rs");
        let url = build_docs_item_url("serde", Some("1.0.0"), "Serialize");
        assert_eq!(url, "https://docs.rs/serde/1.0.0/?search=Serialize");
        std::env::remove_var("CRATES_DOCS_DOCS_RS_URL");
    }

    #[test]
    fn test_build_docs_item_url_encodes_special_chars() {
        std::env::set_var("CRATES_DOCS_DOCS_RS_URL", "https://docs.rs");
        let url = build_docs_item_url("std", None, "collections::HashMap");
        assert!(url.contains("collections%3A%3AHashMap"));
        std::env::remove_var("CRATES_DOCS_DOCS_RS_URL");
    }

    #[test]
    fn test_build_crates_io_search_url_defaults() {
        std::env::set_var("CRATES_DOCS_CRATES_IO_URL", "https://crates.io");
        let url = build_crates_io_search_url("web framework", None, None);
        assert!(url.contains("crates.io/api/v1/crates"));
        assert!(url.contains("q=web+framework") || url.contains("q=web%20framework"));
        assert!(url.contains("per_page=10"));
        assert!(url.contains("sort=relevance"));
        std::env::remove_var("CRATES_DOCS_CRATES_IO_URL");
    }

    #[test]
    fn test_build_crates_io_search_url_with_params() {
        std::env::set_var("CRATES_DOCS_CRATES_IO_URL", "https://crates.io");
        let url = build_crates_io_search_url("async", Some("downloads"), Some(20));
        assert!(url.contains("crates.io/api/v1/crates"));
        assert!(url.contains("q=async"));
        assert!(url.contains("per_page=20"));
        assert!(url.contains("sort=downloads"));
        std::env::remove_var("CRATES_DOCS_CRATES_IO_URL");
    }

    #[test]
    fn test_build_crates_io_search_url_encodes_query() {
        std::env::set_var("CRATES_DOCS_CRATES_IO_URL", "https://crates.io");
        let url = build_crates_io_search_url("web framework", None, None);
        assert!(url.contains("web+framework") || url.contains("web%20framework"));
        std::env::remove_var("CRATES_DOCS_CRATES_IO_URL");
    }
}
