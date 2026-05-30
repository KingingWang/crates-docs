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

/// Formats supported by the documentation lookup tools (`lookup_crate`,
/// `lookup_item`). JSON is intentionally excluded: these tools render prose
/// documentation, not structured data.
pub const DOC_FORMATS: &[Format] = &[Format::Markdown, Format::Text, Format::Html];

/// Formats supported by the `search_crates` tool. HTML is intentionally
/// excluded: search results are structured records, not an HTML document.
pub const SEARCH_FORMATS: &[Format] = &[Format::Markdown, Format::Text, Format::Json];

/// Parse and validate a format string against the formats a tool supports.
///
/// `allowed` lists the formats the calling tool actually accepts. Both an
/// unrecognized string and a recognized format outside `allowed` produce an
/// error that lists only the supported formats, so a caller is never advised to
/// retry with a format the tool will then reject. `None` defaults to markdown,
/// which every tool supports.
pub fn parse_format(
    tool_name: &str,
    format_str: Option<&str>,
    allowed: &[Format],
) -> Result<Format, CallToolError> {
    let Some(s) = format_str else {
        return Ok(Format::Markdown);
    };
    let parsed = match s.trim().to_lowercase().as_str() {
        "markdown" => Some(Format::Markdown),
        "text" => Some(Format::Text),
        "html" => Some(Format::Html),
        "json" => Some(Format::Json),
        _ => None,
    };
    match parsed {
        Some(format) if allowed.contains(&format) => Ok(format),
        _ => {
            let supported = allowed
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ");
            Err(CallToolError::invalid_arguments(
                tool_name,
                Some(format!(
                    "Invalid format '{s}'. This tool supports: {supported}"
                )),
            ))
        }
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
pub fn validate_crate_name(tool_name: &str, crate_name: &str) -> Result<(), CallToolError> {
    let name = crate_name.trim();
    if name.is_empty() {
        return Err(CallToolError::invalid_arguments(
            tool_name,
            Some("crate_name must not be empty".to_string()),
        ));
    }
    if name.len() > 64 {
        return Err(CallToolError::invalid_arguments(
            tool_name,
            Some("crate_name is too long (max 64 characters)".to_string()),
        ));
    }
    if !name
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        return Err(CallToolError::invalid_arguments(
            tool_name,
            Some(format!(
                "Invalid crate_name '{crate_name}'. Only ASCII letters, digits, '_' and '-' are allowed"
            )),
        ));
    }
    Ok(())
}

/// Normalize a user-supplied version string for docs.rs URL construction.
///
/// Trims surrounding whitespace and strips a single leading `v`/`V` when it is
/// immediately followed by a digit (e.g. `v1.2.3` -> `1.2.3`). crates.io and
/// docs.rs versions are plain semver and never carry a `v` prefix, but users
/// routinely copy versions from git tags or changelogs where that prefix is
/// conventional; without this they hit a confusing 400/404. Non-version
/// identifiers such as `latest` (no leading-`v`-before-digit) are unchanged.
#[must_use]
pub fn normalize_version(version: &str) -> String {
    let trimmed = version.trim();
    let bytes = trimmed.as_bytes();
    if bytes.len() >= 2 && (bytes[0] == b'v' || bytes[0] == b'V') && bytes[1].is_ascii_digit() {
        trimmed[1..].to_string()
    } else {
        trimmed.to_string()
    }
}

/// Validate an optional version string supplied by a tool caller.
///
/// Accepts concrete versions and identifiers such as `latest` while rejecting
/// path-traversal sequences and characters that could escape the docs.rs path.
///
/// # Errors
///
/// Returns a `CallToolError` describing the first problem found.
pub fn validate_version(tool_name: &str, version: Option<&str>) -> Result<(), CallToolError> {
    let Some(raw) = version else {
        return Ok(());
    };
    let ver = raw.trim();
    if ver.is_empty() {
        return Err(CallToolError::invalid_arguments(
            tool_name,
            Some("version must not be empty when provided".to_string()),
        ));
    }
    if ver.len() > 64 {
        return Err(CallToolError::invalid_arguments(
            tool_name,
            Some("version is too long (max 64 characters)".to_string()),
        ));
    }
    if ver.contains("..")
        || !ver
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'+' | b'_' | b'~'))
    {
        return Err(CallToolError::invalid_arguments(
            tool_name,
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
pub fn validate_search_query(tool_name: &str, query: &str) -> Result<(), CallToolError> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return Err(CallToolError::invalid_arguments(
            tool_name,
            Some("query must not be empty".to_string()),
        ));
    }
    if trimmed.len() > 200 {
        return Err(CallToolError::invalid_arguments(
            tool_name,
            Some("query is too long (max 200 characters)".to_string()),
        ));
    }
    Ok(())
}

/// Validate an item path supplied by a tool caller.
///
/// Item paths are Rust paths made of identifier segments separated by `::`
/// (for example `serde::Serialize` or `std::vec::Vec::push`). This rejects
/// path-traversal sequences and characters such as `/`, `.` or whitespace that
/// could escape the docs.rs path or otherwise form an invalid request, giving
/// callers an actionable error instead of an opaque HTTP 400.
///
/// # Errors
///
/// Returns a `CallToolError` describing the first problem found.
pub fn validate_item_path(tool_name: &str, item_path: &str) -> Result<(), CallToolError> {
    let path = item_path.trim();
    if path.is_empty() {
        return Err(CallToolError::invalid_arguments(
            tool_name,
            Some("item_path must not be empty".to_string()),
        ));
    }
    if path.len() > 256 {
        return Err(CallToolError::invalid_arguments(
            tool_name,
            Some("item_path is too long (max 256 characters)".to_string()),
        ));
    }
    if path.contains("..")
        || !path
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b':')
    {
        return Err(CallToolError::invalid_arguments(
            tool_name,
            Some(format!(
                "Invalid item_path '{item_path}'. Only ASCII letters, digits, '_' and '::' separators are allowed"
            )),
        ));
    }
    // Rust paths use '::' as the separator; a lone ':' (e.g. `serde:Serialize`)
    // or an empty segment (e.g. `serde::`) would otherwise pass the byte check
    // above and then silently fall back to the crate overview after a 404.
    if path
        .split("::")
        .any(|segment| segment.is_empty() || segment.contains(':'))
    {
        return Err(CallToolError::invalid_arguments(
            tool_name,
            Some(format!(
                "Invalid item_path '{item_path}'. Path segments must be separated by '::'"
            )),
        ));
    }
    Ok(())
}

/// Summarize a non-success HTTP response from docs.rs into a concise,
/// actionable error string.
///
/// docs.rs returns a full HTML error page (often several KB) for failures such
/// as 404. Dumping that entire page into the tool error is noisy and unhelpful,
/// so this collapses it to the status plus a short hint. HTML bodies are never
/// echoed back; only short plain-text bodies are included as a snippet.
fn summarize_http_status(status: reqwest::StatusCode, body: &str) -> String {
    if status == reqwest::StatusCode::NOT_FOUND {
        return "HTTP 404 Not Found - the requested crate, version, or item does not exist on docs.rs. Verify the crate name, version, and item path.".to_string();
    }

    let trimmed = body.trim();
    let lower = trimmed.to_ascii_lowercase();
    let looks_like_html =
        trimmed.starts_with('<') || lower.contains("<!doctype") || lower.contains("<html");
    if trimmed.is_empty() || looks_like_html {
        format!("HTTP {status}")
    } else {
        let snippet: String = trimmed.chars().take(200).collect();
        format!("HTTP {status} - {snippet}")
    }
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
/// Standard distribution crates documented on doc.rust-lang.org.
///
/// The `std`, `core`, `alloc`, `proc_macro`, and `test` crates are not
/// published to docs.rs; their rustdoc lives on doc.rust-lang.org using the
/// same item-page layout but without a version path segment. Item and index
/// (`all.html`) URLs must target that host or every lookup 404s and silently
/// falls back to the crate overview.
#[must_use]
pub fn is_rust_std_crate(crate_name: &str) -> bool {
    matches!(
        crate_name,
        "std" | "core" | "alloc" | "proc_macro" | "proc-macro" | "test"
    )
}

/// Base URL for Rust std-family crate docs on doc.rust-lang.org, honoring an
/// explicit version.
///
/// `doc.rust-lang.org` serves versioned docs at `/{version}/{krate}/` (e.g.
/// `https://doc.rust-lang.org/1.75.0/std/`, and channels `stable`/`beta`/
/// `nightly`). `None` or `"latest"` use the unversioned current docs. The
/// returned base always ends in `/`.
fn rust_lang_docs_base(krate: &str, version: Option<&str>) -> String {
    match version {
        Some(ver) if !ver.trim().is_empty() && ver != "latest" => {
            format!("https://doc.rust-lang.org/{}/{krate}/", ver.trim())
        }
        _ => format!("https://doc.rust-lang.org/{krate}/"),
    }
}

/// Build docs.rs URL for crate documentation
#[must_use]
pub fn build_docs_url(crate_name: &str, version: Option<&str>) -> String {
    if is_rust_std_crate(crate_name) {
        let krate = crate_name.replace('-', "_");
        return rust_lang_docs_base(&krate, version);
    }
    let base_url = docs_rs_base_url();
    match version {
        Some(ver) => format!("{base_url}/{crate_name}/{ver}/"),
        None => format!("{base_url}/{crate_name}/"),
    }
}

/// Build docs.rs search URL for item lookup
#[must_use]
pub fn build_docs_item_url(crate_name: &str, version: Option<&str>, item_path: &str) -> String {
    let encoded_path = urlencoding::encode(item_path);
    if is_rust_std_crate(crate_name) {
        // std/core/alloc/etc. are not published to docs.rs; their docs live on
        // doc.rust-lang.org. Mirror the other URL builders so the last-resort
        // fallback degrades to the crate overview instead of a hard 404.
        let krate = crate_name.replace('-', "_");
        let base = rust_lang_docs_base(&krate, version);
        return format!("{base}?search={encoded_path}");
    }
    let base_url = docs_rs_base_url();
    match version {
        Some(ver) => format!("{base_url}/{crate_name}/{ver}/?search={encoded_path}"),
        None => format!("{base_url}/{crate_name}/?search={encoded_path}"),
    }
}

/// Build candidate docs.rs URLs for a specific item, in priority order.
///
/// rustdoc item pages use predictable `{kind}.{name}.html` file names, but the
/// item kind (struct/trait/fn/...) cannot be derived from the path alone. This
/// returns the plausible candidate URLs to probe; the caller fetches each in
/// order and uses the first that exists (HTTP 200). A trailing module candidate
/// (`{name}/index.html`) covers items that are themselves modules.
///
/// The crate's library path component uses the underscore form (docs.rs maps
/// `-` to `_` for module paths). A leading path segment equal to the crate name
/// is dropped so both `Serialize` and `serde::Serialize` resolve correctly.
#[must_use]
pub fn build_docs_item_url_candidates(
    crate_name: &str,
    version: Option<&str>,
    item_path: &str,
) -> Vec<String> {
    let krate = crate_name.replace('-', "_");

    let segments: Vec<&str> = item_path
        .split("::")
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    let Some((item, mods)) = segments.split_last() else {
        return Vec::new();
    };

    // Drop a redundant leading crate-name segment (e.g. `serde::Serialize`).
    let mods: &[&str] = if mods.first().map(|m| m.replace('-', "_")) == Some(krate.clone()) {
        &mods[1..]
    } else {
        mods
    };

    let mut prefix = if is_rust_std_crate(crate_name) {
        rust_lang_docs_base(&krate, version)
    } else {
        let base_url = docs_rs_base_url();
        let ver = version.unwrap_or("latest");
        format!("{base_url}/{crate_name}/{ver}/{krate}/")
    };
    for m in mods {
        prefix.push_str(m);
        prefix.push('/');
    }

    // Ordered roughly by how common each item kind is.
    let kinds = [
        "struct",
        "trait",
        "enum",
        "fn",
        "type",
        "macro",
        "attr",
        "constant",
        "derive",
        "union",
        "primitive",
    ];
    let mut candidates: Vec<String> = kinds
        .iter()
        .map(|k| format!("{prefix}{k}.{item}.html"))
        .collect();
    // The item itself may be a module.
    candidates.push(format!("{prefix}{item}/index.html"));
    candidates
}

/// Build the docs.rs `all.html` index URL for a crate.
///
/// rustdoc emits an `all.html` page listing every item in the crate (including
/// re-exports) with hrefs relative to the crate root module. It is used to
/// resolve items that have no stub page at the path implied by their name.
#[must_use]
pub fn build_docs_all_items_url(crate_name: &str, version: Option<&str>) -> String {
    let krate = crate_name.replace('-', "_");
    if is_rust_std_crate(crate_name) {
        let base = rust_lang_docs_base(&krate, version);
        return format!("{base}all.html");
    }
    let base_url = docs_rs_base_url();
    let ver = version.unwrap_or("latest");
    format!("{base_url}/{crate_name}/{ver}/{krate}/all.html")
}

/// Resolve an item page URL from a crate's `all.html` index by item name.
///
/// Returns the absolute docs.rs URL of the first item whose rustdoc file name is
/// `{kind}.{item_name}.html` (for any item kind). This resolves re-exported
/// items such as `tokio::spawn` (actually defined at `tokio::task::spawn`),
/// which have no stub page at the crate root. Returns `None` if no match is
/// found or the name is empty.
#[must_use]
pub fn find_item_url_in_all_html(
    crate_name: &str,
    version: Option<&str>,
    all_html: &str,
    item_name: &str,
) -> Option<String> {
    let item_name = item_name.trim();
    if item_name.is_empty() {
        return None;
    }
    let kinds = "struct|trait|enum|fn|type|macro|attr|constant|derive|union|primitive";
    let pattern = format!(
        "href=\"((?:[^\"]*/)?(?:{kinds})\\.{}\\.html)\"",
        regex::escape(item_name)
    );
    let re = regex::Regex::new(&pattern).ok()?;
    let href = re.captures(all_html)?.get(1)?.as_str();

    let krate = crate_name.replace('-', "_");
    if is_rust_std_crate(crate_name) {
        // std/core/alloc docs live on doc.rust-lang.org, not docs.rs; the
        // all.html index there links relative to the crate root.
        let base = rust_lang_docs_base(&krate, version);
        return Some(format!("{base}{href}"));
    }
    let base_url = docs_rs_base_url();
    let ver = version.unwrap_or("latest");
    Some(format!("{base_url}/{crate_name}/{ver}/{krate}/{href}"))
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
                "{prefix}Failed to get documentation: {}",
                summarize_http_status(status, &error_body)
            )));
        }

        response.text().await.map_err(|e| {
            let prefix = tool_name.map_or(String::new(), |n| format!("[{n}] "));
            CallToolError::from_message(format!("{prefix}Failed to read response: {e}"))
        })
    }

    /// Fetch HTML from `url`, returning `Ok(None)` when the resource does not
    /// exist (HTTP 404) instead of an error.
    ///
    /// This is used to probe candidate docs.rs item URLs where a 404 simply
    /// means "this item kind does not match" rather than a hard failure.
    ///
    /// # Errors
    ///
    /// Returns a `CallToolError` if the request fails, the response has a
    /// non-success status other than 404, or reading the body fails.
    pub async fn fetch_html_optional(
        &self,
        url: &str,
        tool_name: Option<&str>,
    ) -> Result<Option<String>, CallToolError> {
        let response = self.client.get(url).send().await.map_err(|e| {
            let prefix = tool_name.map_or(String::new(), |n| format!("[{n}] "));
            CallToolError::from_message(format!("{prefix}HTTP request failed: {e}"))
        })?;

        let status = response.status();
        if status == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !status.is_success() {
            // Surface a body-read failure instead of masking it with an empty
            // string (matches `fetch_html` and the documented contract).
            let error_body = response.text().await.map_err(|e| {
                let prefix = tool_name.map_or(String::new(), |n| format!("[{n}] "));
                CallToolError::from_message(format!("{prefix}Failed to read error response: {e}"))
            })?;
            let prefix = tool_name.map_or(String::new(), |n| format!("[{n}] "));
            return Err(CallToolError::from_message(format!(
                "{prefix}Failed to get documentation: {}",
                summarize_http_status(status, &error_body)
            )));
        }

        let body = response.text().await.map_err(|e| {
            let prefix = tool_name.map_or(String::new(), |n| format!("[{n}] "));
            CallToolError::from_message(format!("{prefix}Failed to read response: {e}"))
        })?;
        Ok(Some(body))
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
                // Fallback: create a minimal client without retry middleware.
                // Apply timeouts matching HttpClientBuilder's defaults so the
                // fallback cannot hang forever on a slow/stalled connection.
                // If the builder fails for any reason, fall back to the
                // infallible Client::new() (which never panics).
                let plain_client = reqwest::Client::builder()
                    .timeout(std::time::Duration::from_secs(30))
                    .connect_timeout(std::time::Duration::from_secs(10))
                    .build()
                    .unwrap_or_else(|_| reqwest::Client::new());
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

    /// All syntactically valid formats, used to exercise the string->Format
    /// mapping independently of any single tool's allowed set.
    const ALL: &[Format] = &[Format::Markdown, Format::Text, Format::Html, Format::Json];

    #[test]
    fn test_validate_crate_name_accepts_valid() {
        assert!(validate_crate_name("lookup_crate", "serde").is_ok());
        assert!(validate_crate_name("lookup_crate", "serde_json").is_ok());
        assert!(validate_crate_name("lookup_crate", "tracing-subscriber").is_ok());
        assert!(validate_crate_name("lookup_crate", "  tokio  ").is_ok());
    }

    #[test]
    fn test_validate_crate_name_rejects_invalid() {
        assert!(validate_crate_name("lookup_crate", "").is_err());
        assert!(validate_crate_name("lookup_crate", "   ").is_err());
        assert!(validate_crate_name("lookup_crate", "../etc/passwd").is_err());
        assert!(validate_crate_name("lookup_crate", "foo/bar").is_err());
        assert!(validate_crate_name("lookup_crate", "foo bar").is_err());
        assert!(validate_crate_name("lookup_crate", "foo;rm").is_err());
        assert!(validate_crate_name("lookup_crate", &"a".repeat(65)).is_err());
    }

    #[test]
    fn test_normalize_version_strips_leading_v() {
        assert_eq!(normalize_version("v1.2.3"), "1.2.3");
        assert_eq!(normalize_version("V2.0.0"), "2.0.0");
        assert_eq!(normalize_version("  v1.0  "), "1.0");
        // Already canonical / non-version identifiers are untouched.
        assert_eq!(normalize_version("1.0.0"), "1.0.0");
        assert_eq!(normalize_version("latest"), "latest");
        // A leading 'v' not followed by a digit is part of the identifier.
        assert_eq!(normalize_version("vendored"), "vendored");
        assert_eq!(normalize_version("v"), "v");
    }

    #[test]
    fn test_validate_version_accepts_valid() {
        assert!(validate_version("lookup_crate", None).is_ok());
        assert!(validate_version("lookup_crate", Some("1.0.0")).is_ok());
        assert!(validate_version("lookup_crate", Some("1.0.0-rc.1")).is_ok());
        assert!(validate_version("lookup_crate", Some("1.0.0+build.5")).is_ok());
        assert!(validate_version("lookup_crate", Some("latest")).is_ok());
        assert!(validate_version("lookup_crate", Some("  1.2.3  ")).is_ok());
    }

    #[test]
    fn test_validate_version_rejects_invalid() {
        assert!(validate_version("lookup_crate", Some("")).is_err());
        assert!(validate_version("lookup_crate", Some("../../1.0")).is_err());
        assert!(validate_version("lookup_crate", Some("1.0/2.0")).is_err());
        assert!(validate_version("lookup_crate", Some("1.0 0")).is_err());
        assert!(validate_version("lookup_crate", Some("..")).is_err());
        assert!(validate_version("lookup_crate", Some(&"1".repeat(65))).is_err());
    }

    #[test]
    fn test_validate_item_path_accepts_valid() {
        assert!(validate_item_path("lookup_item", "Serialize").is_ok());
        assert!(validate_item_path("lookup_item", "serde::Serialize").is_ok());
        assert!(validate_item_path("lookup_item", "std::vec::Vec::push").is_ok());
        assert!(validate_item_path("lookup_item", "collections::HashMap").is_ok());
        assert!(validate_item_path("lookup_item", "u32").is_ok());
        assert!(validate_item_path("lookup_item", "  tokio::main  ").is_ok());
    }

    #[test]
    fn test_validate_item_path_rejects_invalid() {
        assert!(validate_item_path("lookup_item", "").is_err());
        assert!(validate_item_path("lookup_item", "   ").is_err());
        assert!(validate_item_path("lookup_item", "../../etc/passwd").is_err());
        assert!(validate_item_path("lookup_item", "serde/Serialize").is_err());
        assert!(validate_item_path("lookup_item", "serde::Ser ialize").is_err());
        assert!(validate_item_path("lookup_item", "foo;rm").is_err());
        assert!(validate_item_path("lookup_item", "foo.bar").is_err());
        assert!(validate_item_path("lookup_item", &"a".repeat(257)).is_err());
        // Single-colon separators and empty path segments are malformed.
        assert!(validate_item_path("lookup_item", "serde:Serialize").is_err());
        assert!(validate_item_path("lookup_item", "serde::").is_err());
        assert!(validate_item_path("lookup_item", "::Serialize").is_err());
        assert!(validate_item_path("lookup_item", "std:::vec").is_err());
    }

    #[test]
    fn test_validate_search_query_accepts_valid() {
        assert!(validate_search_query("search_crates", "serde").is_ok());
        assert!(validate_search_query("search_crates", "web framework").is_ok());
        assert!(validate_search_query("search_crates", "  tokio  ").is_ok());
        assert!(validate_search_query("search_crates", &"a".repeat(200)).is_ok());
    }

    #[test]
    fn test_validate_search_query_rejects_invalid() {
        assert!(validate_search_query("search_crates", "").is_err());
        assert!(validate_search_query("search_crates", "   ").is_err());
        assert!(validate_search_query("search_crates", &"a".repeat(201)).is_err());
    }

    #[test]
    fn test_item_url_candidates_include_attr_macro() {
        // Attribute proc-macros (e.g. async-trait's #[async_trait]) live at
        // attr.<name>.html and must be among the probed candidates.
        let c = build_docs_item_url_candidates("async-trait", None, "async_trait");
        assert!(
            c.iter()
                .any(|u| u.ends_with("/async_trait/attr.async_trait.html")),
            "missing attr candidate: {c:?}"
        );
    }

    #[test]
    fn test_item_url_candidates_strip_redundant_crate_segment() {
        let c = build_docs_item_url_candidates("serde", None, "serde::Serialize");
        assert!(c
            .iter()
            .any(|u| u.ends_with("/serde/latest/serde/trait.Serialize.html")));
        assert!(c
            .iter()
            .any(|u| u.ends_with("/serde/latest/serde/struct.Serialize.html")));
        // module fallback candidate is last
        assert!(c
            .last()
            .unwrap()
            .ends_with("/serde/latest/serde/Serialize/index.html"));
    }

    #[test]
    fn test_item_url_candidates_nested_module_and_version() {
        let c = build_docs_item_url_candidates("serde", Some("1.0.0"), "de::Deserializer");
        assert!(c
            .iter()
            .any(|u| u.ends_with("/serde/1.0.0/serde/de/trait.Deserializer.html")));
    }

    #[test]
    fn test_item_url_candidates_hyphen_crate_uses_underscore_path() {
        let c = build_docs_item_url_candidates("serde-with", None, "As");
        // First path component keeps the crate name; the lib path uses underscores.
        assert!(c
            .iter()
            .any(|u| u.ends_with("/serde-with/latest/serde_with/struct.As.html")));
    }

    #[test]
    fn test_item_url_candidates_empty_path() {
        assert!(build_docs_item_url_candidates("serde", None, "   ").is_empty());
    }

    #[test]
    fn test_all_items_url() {
        assert_eq!(
            build_docs_all_items_url("tokio", None),
            "https://docs.rs/tokio/latest/tokio/all.html"
        );
        assert_eq!(
            build_docs_all_items_url("foo-bar", Some("1.2.3")),
            "https://docs.rs/foo-bar/1.2.3/foo_bar/all.html"
        );
    }

    #[test]
    fn test_is_rust_std_crate() {
        for c in ["std", "core", "alloc", "proc_macro", "proc-macro", "test"] {
            assert!(is_rust_std_crate(c), "{c} should be a std crate");
        }
        for c in ["serde", "tokio", "anyhow", "stdweb"] {
            assert!(!is_rust_std_crate(c), "{c} should not be a std crate");
        }
    }

    #[test]
    fn test_std_crate_honors_explicit_version() {
        // doc.rust-lang.org serves versioned docs; an explicit version must not
        // be silently dropped for std-family crates.
        assert_eq!(
            build_docs_url("std", Some("1.75.0")),
            "https://doc.rust-lang.org/1.75.0/std/"
        );
        assert_eq!(
            build_docs_all_items_url("core", Some("1.75.0")),
            "https://doc.rust-lang.org/1.75.0/core/all.html"
        );
        let c = build_docs_item_url_candidates("std", Some("1.75.0"), "collections::HashMap");
        assert!(
            c.contains(
                &"https://doc.rust-lang.org/1.75.0/std/collections/struct.HashMap.html".to_string()
            ),
            "versioned std candidate missing: {c:?}"
        );
        // "latest" and None fall back to the unversioned current docs.
        assert_eq!(
            build_docs_url("std", Some("latest")),
            "https://doc.rust-lang.org/std/"
        );
    }

    #[test]
    fn test_std_crate_uses_rust_lang_host() {
        // Crate page, item candidates, and all.html for std crates must target
        // doc.rust-lang.org (they are not published to docs.rs).
        assert_eq!(
            build_docs_url("std", None),
            "https://doc.rust-lang.org/std/"
        );
        assert_eq!(
            build_docs_all_items_url("core", None),
            "https://doc.rust-lang.org/core/all.html"
        );
        let c = build_docs_item_url_candidates("std", None, "collections::HashMap");
        assert!(
            c.iter()
                .all(|u| u.starts_with("https://doc.rust-lang.org/std/collections/")),
            "candidates not on rust-lang host: {c:?}"
        );
        assert!(
            c.contains(
                &"https://doc.rust-lang.org/std/collections/struct.HashMap.html".to_string()
            ),
            "missing HashMap struct candidate: {c:?}"
        );
    }

    #[test]
    fn test_find_item_url_in_all_html_reexport() {
        let html = r#"<a href="task/fn.spawn.html">task::spawn</a>"#;
        let url = find_item_url_in_all_html("tokio", None, html, "spawn");
        assert_eq!(
            url.as_deref(),
            Some("https://docs.rs/tokio/latest/tokio/task/fn.spawn.html")
        );
    }

    #[test]
    fn test_find_item_url_in_all_html_root_struct() {
        let html = r#"<a href="struct.Builder.html">Builder</a>"#;
        let url = find_item_url_in_all_html("foo", Some("0.1.0"), html, "Builder");
        assert_eq!(
            url.as_deref(),
            Some("https://docs.rs/foo/0.1.0/foo/struct.Builder.html")
        );
    }

    #[test]
    fn test_find_item_url_in_all_html_std_routes_to_rust_lang() {
        // std/core/alloc re-export fallbacks must target doc.rust-lang.org,
        // not docs.rs (which always 404s for the standard library).
        let html = r#"<a href="task/fn.spawn.html">task::spawn</a>"#;
        let url = find_item_url_in_all_html("std", None, html, "spawn");
        assert_eq!(
            url.as_deref(),
            Some("https://doc.rust-lang.org/std/task/fn.spawn.html")
        );
        // An explicit version is honored and embedded in the path
        // (doc.rust-lang.org/{version}/{krate}/...).
        let core_html = r#"<a href="future/trait.Future.html">Future</a>"#;
        let core_url = find_item_url_in_all_html("core", Some("1.0.0"), core_html, "Future");
        assert_eq!(
            core_url.as_deref(),
            Some("https://doc.rust-lang.org/1.0.0/core/future/trait.Future.html")
        );
    }

    #[test]
    fn test_find_item_url_in_all_html_no_match() {
        let html = r#"<a href="struct.Other.html">Other</a>"#;
        assert!(find_item_url_in_all_html("foo", None, html, "spawn").is_none());
        assert!(find_item_url_in_all_html("foo", None, html, "").is_none());
    }

    #[test]
    fn test_summarize_http_status_not_found() {
        let msg = summarize_http_status(
            reqwest::StatusCode::NOT_FOUND,
            "<!DOCTYPE html><html><body>The requested crate does not exist</body></html>",
        );
        assert!(msg.contains("HTTP 404 Not Found"));
        assert!(msg.contains("does not exist on docs.rs"));
        // The full HTML body must never be echoed back.
        assert!(!msg.contains("<html"));
        assert!(!msg.contains("<!DOCTYPE"));
    }

    #[test]
    fn test_summarize_http_status_hides_html_body() {
        let msg = summarize_http_status(
            reqwest::StatusCode::INTERNAL_SERVER_ERROR,
            "<html><body>boom</body></html>",
        );
        assert_eq!(msg, "HTTP 500 Internal Server Error");
    }

    #[test]
    fn test_summarize_http_status_includes_short_plain_body() {
        let msg = summarize_http_status(reqwest::StatusCode::BAD_GATEWAY, "upstream timeout");
        assert_eq!(msg, "HTTP 502 Bad Gateway - upstream timeout");
    }

    #[test]
    fn test_summarize_http_status_empty_body() {
        let msg = summarize_http_status(reqwest::StatusCode::SERVICE_UNAVAILABLE, "   ");
        assert_eq!(msg, "HTTP 503 Service Unavailable");
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
        assert_eq!(
            parse_format("lookup_crate", None, ALL).unwrap(),
            Format::Markdown
        );
    }

    #[test]
    fn test_parse_format_markdown() {
        assert_eq!(
            parse_format("lookup_crate", Some("markdown"), ALL).unwrap(),
            Format::Markdown
        );
        assert_eq!(
            parse_format("lookup_crate", Some("MARKDOWN"), ALL).unwrap(),
            Format::Markdown
        );
        assert_eq!(
            parse_format("lookup_crate", Some("Markdown"), ALL).unwrap(),
            Format::Markdown
        );
    }

    #[test]
    fn test_parse_format_text() {
        assert_eq!(
            parse_format("lookup_crate", Some("text"), ALL).unwrap(),
            Format::Text
        );
        assert_eq!(
            parse_format("lookup_crate", Some("TEXT"), ALL).unwrap(),
            Format::Text
        );
    }

    #[test]
    fn test_parse_format_html() {
        assert_eq!(
            parse_format("lookup_crate", Some("html"), ALL).unwrap(),
            Format::Html
        );
        assert_eq!(
            parse_format("lookup_crate", Some("HTML"), ALL).unwrap(),
            Format::Html
        );
    }

    #[test]
    fn test_parse_format_json() {
        assert_eq!(
            parse_format("lookup_crate", Some("json"), ALL).unwrap(),
            Format::Json
        );
        assert_eq!(
            parse_format("lookup_crate", Some("JSON"), ALL).unwrap(),
            Format::Json
        );
    }

    #[test]
    fn test_parse_format_trims_whitespace() {
        // Surrounding whitespace is tolerated (consistent with sort
        // normalization) so e.g. " markdown " parses like "markdown".
        assert_eq!(
            parse_format("lookup_crate", Some(" markdown "), ALL).unwrap(),
            Format::Markdown
        );
        assert_eq!(
            parse_format("lookup_crate", Some("\tjson\n"), ALL).unwrap(),
            Format::Json
        );
        // Whitespace-only input still trims to empty and is rejected.
        assert!(parse_format("lookup_crate", Some("   "), ALL).is_err());
    }

    #[test]
    fn test_parse_format_invalid() {
        assert!(parse_format("lookup_crate", Some("invalid"), ALL).is_err());
        assert!(parse_format("lookup_crate", Some("xml"), ALL).is_err());
        assert!(parse_format("lookup_crate", Some(""), ALL).is_err());
    }

    #[test]
    fn test_parse_format_rejects_unsupported_for_tool() {
        // `html` is a valid format string but not supported by search; the
        // error must advertise only the formats search actually accepts and
        // must not over-advertise html.
        let err = parse_format("search_crates", Some("html"), SEARCH_FORMATS).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("This tool supports: markdown, text, json"),
            "got: {msg}"
        );
        assert!(!msg.contains("text, html"), "over-advertises html: {msg}");

        // `json` is valid but unsupported by the doc lookup tools.
        let err = parse_format("lookup_crate", Some("json"), DOC_FORMATS).unwrap_err();
        assert!(
            err.to_string()
                .contains("This tool supports: markdown, text, html"),
            "got: {err}"
        );

        // Unknown formats are rejected against the same per-tool allowed list.
        let err = parse_format("search_crates", Some("xml"), SEARCH_FORMATS).unwrap_err();
        assert!(
            err.to_string().contains("markdown, text, json"),
            "got: {err}"
        );

        // Supported formats still parse.
        assert_eq!(
            parse_format("search_crates", Some("json"), SEARCH_FORMATS).unwrap(),
            Format::Json
        );
        assert_eq!(
            parse_format("lookup_crate", Some("html"), DOC_FORMATS).unwrap(),
            Format::Html
        );
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
    #[test]
    fn test_validation_errors_report_their_tool_name() {
        // Regression: argument-validation errors must name the *tool*
        // (e.g. "lookup_crate"), not the offending field (e.g. "crate_name"),
        // so callers see "Invalid arguments for tool 'lookup_crate'".
        let err = validate_crate_name("lookup_crate", "../etc/passwd").unwrap_err();
        assert!(
            err.to_string().contains("lookup_crate"),
            "expected tool name in error, got: {err}"
        );

        let err = validate_version("lookup_crate", Some("1.0/2.0")).unwrap_err();
        assert!(err.to_string().contains("lookup_crate"), "got: {err}");

        let err = validate_item_path("lookup_item", "foo/bar").unwrap_err();
        assert!(err.to_string().contains("lookup_item"), "got: {err}");

        let err = validate_search_query("search_crates", "").unwrap_err();
        assert!(err.to_string().contains("search_crates"), "got: {err}");

        let err = parse_format("lookup_crate", Some("xml"), ALL).unwrap_err();
        assert!(err.to_string().contains("lookup_crate"), "got: {err}");
    }
}
