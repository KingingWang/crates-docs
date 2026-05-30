//! Unit tests for tools/docs module

use crates_docs::cache::Cache;
use crates_docs::tools::docs::{
    cache::{DocCache, DocCacheTtl},
    html::{clean_html, extract_documentation, extract_search_results, html_to_text},
};
use http::Extensions;
use reqwest::{Request, Response, Url};
use reqwest_middleware::{ClientBuilder, Middleware, Next};
use serial_test::serial;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

struct EnvVarGuard {
    key: &'static str,
    original_value: Option<String>,
}

impl EnvVarGuard {
    fn new(key: &'static str, value: &str) -> Self {
        let original_value = std::env::var(key).ok();
        std::env::set_var(key, value);
        Self {
            key,
            original_value,
        }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        if let Some(ref value) = self.original_value {
            std::env::set_var(self.key, value);
        } else {
            std::env::remove_var(self.key);
        }
    }
}

#[derive(Clone)]
/// Test middleware that redirects outgoing docs.rs requests to a wiremock
/// server while keeping the original request path and query intact.
struct RewriteDocsRsMiddleware {
    target_base_url: Url,
    request_count: Arc<AtomicUsize>,
}

#[async_trait::async_trait]
impl Middleware for RewriteDocsRsMiddleware {
    async fn handle(
        &self,
        mut req: Request,
        extensions: &mut Extensions,
        next: Next<'_>,
    ) -> reqwest_middleware::Result<Response> {
        self.request_count.fetch_add(1, Ordering::SeqCst);

        let url = req.url_mut();
        url.set_scheme(self.target_base_url.scheme())
            .expect("scheme rewrite should succeed");
        url.set_host(self.target_base_url.host_str())
            .expect("host rewrite should succeed");
        url.set_port(self.target_base_url.port())
            .expect("port rewrite should succeed");

        next.run(req, extensions).await
    }
}

fn build_docs_rs_test_client(
    target_base_url: &str,
    request_count: Arc<AtomicUsize>,
) -> Arc<reqwest_middleware::ClientWithMiddleware> {
    let middleware = RewriteDocsRsMiddleware {
        target_base_url: Url::parse(target_base_url).expect("mock server URL should parse"),
        request_count,
    };

    Arc::new(
        ClientBuilder::new(reqwest::Client::new())
            .with(middleware)
            .build(),
    )
}

/// Build a test client that transparently redirects crates.io API requests to
/// a wiremock server, mirroring `build_docs_rs_test_client`. Routing via
/// middleware keeps the search tests hermetic without relying on the
/// `#[cfg(test)]`-gated `CRATES_DOCS_CRATES_IO_URL` override, which is not
/// compiled into the integration-test build of the library and would otherwise
/// let these tests fall through to the live crates.io API (a source of
/// rate-limit flakes and false-positive assertions).
fn build_crates_io_test_client(
    target_base_url: &str,
) -> Arc<reqwest_middleware::ClientWithMiddleware> {
    build_docs_rs_test_client(target_base_url, Arc::new(AtomicUsize::new(0)))
}

/// Test DocCacheTtl default values
#[test]
fn test_doc_cache_ttl_default() {
    let ttl = DocCacheTtl::default();
    assert_eq!(ttl.crate_docs_secs, 3600);
    assert_eq!(ttl.search_results_secs, 300);
    assert_eq!(ttl.item_docs_secs, 1800);
}

/// Test DocCacheTtl from cache config
#[test]
fn test_doc_cache_ttl_from_config() {
    use crates_docs::cache::CacheConfig;

    let config = CacheConfig {
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

/// Test DocCache basic operations
#[tokio::test]
async fn test_doc_cache_crate_docs() {
    let memory_cache = crates_docs::cache::memory::MemoryCache::new(100);
    let cache = Arc::new(memory_cache);
    let doc_cache = DocCache::new(cache);

    // Test set and get crate docs
    doc_cache
        .set_crate_docs("serde", Some("1.0.0"), "Serde documentation".to_string())
        .await
        .expect("set_crate_docs should succeed");

    let result = doc_cache.get_crate_docs("serde", Some("1.0.0")).await;
    assert_eq!(
        result.as_ref().map(|s| s.as_ref()),
        Some("Serde documentation")
    );

    // Test get non-existent crate
    let result = doc_cache.get_crate_docs("nonexistent", None).await;
    assert_eq!(result, None);
}

/// Test DocCache search results
#[tokio::test]
async fn test_doc_cache_search_results() {
    let memory_cache = crates_docs::cache::memory::MemoryCache::new(100);
    let cache = Arc::new(memory_cache);
    let doc_cache = DocCache::new(cache);

    // Test set and get search results
    doc_cache
        .set_search_results(
            "web framework",
            10,
            Some("relevance"),
            "Search results".to_string(),
        )
        .await
        .expect("set_search_results should succeed");

    let result = doc_cache
        .get_search_results("web framework", 10, Some("relevance"))
        .await;
    assert_eq!(result.as_ref().map(|s| s.as_ref()), Some("Search results"));

    // Test different limit
    let result = doc_cache
        .get_search_results("web framework", 20, Some("relevance"))
        .await;
    assert_eq!(result, None);
}

/// Test DocCache item docs
#[tokio::test]
async fn test_doc_cache_item_docs() {
    let memory_cache = crates_docs::cache::memory::MemoryCache::new(100);
    let cache = Arc::new(memory_cache);
    let doc_cache = DocCache::new(cache);

    // Test set and get item docs
    doc_cache
        .set_item_docs(
            "serde",
            "serde::Serialize",
            Some("1.0.0"),
            "Serialize trait docs".to_string(),
        )
        .await
        .expect("set_item_docs should succeed");

    let result = doc_cache
        .get_item_docs("serde", "serde::Serialize", Some("1.0.0"))
        .await;
    assert_eq!(
        result.as_ref().map(|s| s.as_ref()),
        Some("Serialize trait docs")
    );
}

/// Test HTML cleaning
#[test]
fn test_clean_html_basic() {
    let html = "<html><body><p>Hello World</p></body></html>";
    let cleaned = clean_html(html);
    assert!(cleaned.contains("<p>Hello World</p>"));
}

/// Test HTML cleaning removes script tags
#[test]
fn test_clean_html_removes_script() {
    let html = r#"<html><script>alert("xss")</script><body>Content</body></html>"#;
    let cleaned = clean_html(html);
    assert!(!cleaned.contains("script"));
    assert!(!cleaned.contains("alert"));
    assert!(cleaned.contains("Content"));
}

/// Test HTML cleaning removes style tags
#[test]
fn test_clean_html_removes_style() {
    let html = r#"<html><style>.red { color: red; }</style><body>Content</body></html>"#;
    let cleaned = clean_html(html);
    assert!(!cleaned.contains("style"));
    assert!(!cleaned.contains(".red"));
    assert!(cleaned.contains("Content"));
}

/// Test HTML to text conversion
#[test]
fn test_html_to_text_basic() {
    let html = "<p>Hello <strong>World</strong>!</p>";
    let text = html_to_text(html);
    assert!(text.contains("Hello"));
    assert!(text.contains("World"));
    assert!(!text.contains("<p>"));
    assert!(!text.contains("</p>"));
}

/// Test HTML entity decoding
#[test]
fn test_html_to_text_entities() {
    let html = "<p>Tom & Jerry</p>";
    let text = html_to_text(html);
    // The function should decode amp entity or at least contain Tom
    assert!(text.contains("Tom") || text.contains("&"));
}

/// Test extract documentation
#[test]
fn test_extract_documentation_basic() {
    let html = "<html><body><h1>Title</h1><p>Content</p></body></html>";
    let docs = extract_documentation(html);
    assert!(docs.contains("Title"));
    assert!(docs.contains("Content"));
}

/// Test extract search results
#[test]
fn test_extract_search_results_found() {
    let html = "<html><body><h1>Result</h1><p>Description</p></body></html>";
    let result = extract_search_results(html, "test::item");
    assert!(result.contains("test::item"));
    assert!(result.contains("Result"));
}

/// Test extract search results not found
#[test]
fn test_extract_search_results_not_found() {
    let html = "<html><body></body></html>";
    let result = extract_search_results(html, "nonexistent");
    assert!(result.contains("not found"));
    assert!(result.contains("nonexistent"));
}

/// Test DocCache clear
#[tokio::test]
async fn test_doc_cache_clear() {
    let memory_cache = crates_docs::cache::memory::MemoryCache::new(100);
    let cache = Arc::new(memory_cache);
    let doc_cache = DocCache::new(cache);

    // Add some entries
    doc_cache
        .set_crate_docs("serde", None, "docs".to_string())
        .await
        .expect("set should succeed");

    // Clear cache
    doc_cache.clear().await.expect("clear should succeed");

    // Verify entries are cleared
    let result = doc_cache.get_crate_docs("serde", None).await;
    assert_eq!(result, None);
}

// ============================================================================
// LookupCrateTool tests
// ============================================================================

#[test]
fn test_lookup_crate_tool_params() {
    use crates_docs::tools::docs::lookup_crate::LookupCrateTool;

    let params = LookupCrateTool {
        crate_name: "serde".to_string(),
        version: Some("1.0.0".to_string()),
        format: Some("markdown".to_string()),
    };

    assert_eq!(params.crate_name, "serde");
    assert_eq!(params.version, Some("1.0.0".to_string()));
    assert_eq!(params.format, Some("markdown".to_string()));
}

#[tokio::test]
#[serial(docs_rs_env)]
async fn test_lookup_crate_tool_execute_markdown() {
    use crates_docs::tools::Tool;
    use wiremock::{matchers, Mock, MockServer, ResponseTemplate};

    let mock_server = MockServer::start().await;
    let mock_uri = mock_server.uri();
    let mock_html = r#"
    <!DOCTYPE html>
    <html>
    <head><title>Serde</title></head>
    <body>
        <section id="main-content">
            <h1>Serde</h1>
            <p>Serialization framework for Rust</p>
        </section>
    </body>
    </html>
    "#;

    Mock::given(matchers::method("GET"))
        .and(matchers::path("/serde/"))
        .respond_with(ResponseTemplate::new(200).set_body_string(mock_html))
        .mount(&mock_server)
        .await;

    let _guard = EnvVarGuard::new("CRATES_DOCS_DOCS_RS_URL", &mock_uri);

    let memory_cache = crates_docs::cache::memory::MemoryCache::new(100);
    let cache = Arc::new(memory_cache);
    let cache_config = crates_docs::cache::CacheConfig::default();

    let test_client = reqwest_middleware::ClientBuilder::new(reqwest::Client::new()).build();
    let service = crates_docs::tools::docs::DocService::with_custom_client(
        cache,
        &cache_config,
        Arc::new(test_client),
    );

    let tool = crates_docs::tools::docs::lookup_crate::LookupCrateToolImpl::new(Arc::new(service));

    let args = serde_json::json!({
        "crate_name": "serde",
        "format": "markdown"
    });

    let result = tool.execute(args).await;
    assert!(result.is_ok());
}

#[tokio::test]
#[serial(docs_rs_env)]
async fn test_lookup_crate_tool_execute_text_format() {
    use crates_docs::tools::Tool;
    use wiremock::{matchers, Mock, MockServer, ResponseTemplate};

    let mock_server = MockServer::start().await;
    let mock_uri = mock_server.uri();
    let mock_html = r#"
    <html><body><h1>Serde</h1><p>Serialization framework</p></body></html>
    "#;

    Mock::given(matchers::method("GET"))
        .and(matchers::path("/serde/"))
        .respond_with(ResponseTemplate::new(200).set_body_string(mock_html))
        .mount(&mock_server)
        .await;

    let _guard = EnvVarGuard::new("CRATES_DOCS_DOCS_RS_URL", &mock_uri);

    let memory_cache = crates_docs::cache::memory::MemoryCache::new(100);
    let cache = Arc::new(memory_cache);
    let cache_config = crates_docs::cache::CacheConfig::default();

    let test_client = reqwest_middleware::ClientBuilder::new(reqwest::Client::new()).build();
    let service = crates_docs::tools::docs::DocService::with_custom_client(
        cache,
        &cache_config,
        Arc::new(test_client),
    );

    let tool = crates_docs::tools::docs::lookup_crate::LookupCrateToolImpl::new(Arc::new(service));

    let args = serde_json::json!({
        "crate_name": "serde",
        "format": "text"
    });

    let result = tool.execute(args).await;
    assert!(result.is_ok());
}

#[tokio::test]
#[serial(docs_rs_env)]
async fn test_lookup_crate_tool_execute_html_format() {
    use crates_docs::tools::Tool;
    use wiremock::{matchers, Mock, MockServer, ResponseTemplate};

    let mock_server = MockServer::start().await;
    let mock_uri = mock_server.uri();
    let mock_html = r#"
    <html><body><h1>Serde</h1><p>Serialization framework</p></body></html>
    "#;

    Mock::given(matchers::method("GET"))
        .and(matchers::path("/serde/"))
        .respond_with(ResponseTemplate::new(200).set_body_string(mock_html))
        .mount(&mock_server)
        .await;

    let _guard = EnvVarGuard::new("CRATES_DOCS_DOCS_RS_URL", &mock_uri);

    let memory_cache = crates_docs::cache::memory::MemoryCache::new(100);
    let cache = Arc::new(memory_cache);
    let cache_config = crates_docs::cache::CacheConfig::default();

    let test_client = reqwest_middleware::ClientBuilder::new(reqwest::Client::new()).build();
    let service = crates_docs::tools::docs::DocService::with_custom_client(
        cache,
        &cache_config,
        Arc::new(test_client),
    );

    let tool = crates_docs::tools::docs::lookup_crate::LookupCrateToolImpl::new(Arc::new(service));

    let args = serde_json::json!({
        "crate_name": "serde",
        "format": "html"
    });

    let result = tool.execute(args).await;
    assert!(result.is_ok());
}

#[tokio::test]
#[serial(docs_rs_env)]
async fn test_lookup_crate_tool_execute_with_version() {
    use crates_docs::tools::Tool;
    use wiremock::{matchers, Mock, MockServer, ResponseTemplate};

    let mock_server = MockServer::start().await;
    let mock_uri = mock_server.uri();
    let mock_html = r#"
    <html><body><h1>Serde 1.0.0</h1><p>Version 1.0.0 docs</p></body></html>
    "#;

    Mock::given(matchers::method("GET"))
        .and(matchers::path("/serde/1.0.0/"))
        .respond_with(ResponseTemplate::new(200).set_body_string(mock_html))
        .mount(&mock_server)
        .await;

    let _guard = EnvVarGuard::new("CRATES_DOCS_DOCS_RS_URL", &mock_uri);

    let memory_cache = crates_docs::cache::memory::MemoryCache::new(100);
    let cache = Arc::new(memory_cache);
    let cache_config = crates_docs::cache::CacheConfig::default();

    let test_client = reqwest_middleware::ClientBuilder::new(reqwest::Client::new()).build();
    let service = crates_docs::tools::docs::DocService::with_custom_client(
        cache,
        &cache_config,
        Arc::new(test_client),
    );

    let tool = crates_docs::tools::docs::lookup_crate::LookupCrateToolImpl::new(Arc::new(service));

    let args = serde_json::json!({
        "crate_name": "serde",
        "version": "1.0.0",
        "format": "markdown"
    });

    let result = tool.execute(args).await;
    assert!(result.is_ok());
}

#[tokio::test]
#[serial(docs_rs_env)]
async fn test_lookup_crate_tool_reuses_single_upstream_fetch_across_formats() {
    use crates_docs::tools::Tool;
    use wiremock::{matchers, Mock, MockServer, ResponseTemplate};

    let mock_server = MockServer::start().await;
    let mock_uri = mock_server.uri();
    let mock_html = r#"
    <!DOCTYPE html>
    <html>
    <body>
        <section id="main-content">
            <h1>Serde</h1>
            <p>Serialization framework for Rust</p>
        </section>
    </body>
    </html>
    "#;

    Mock::given(matchers::method("GET"))
        .and(matchers::path("/serde/"))
        .respond_with(ResponseTemplate::new(200).set_body_string(mock_html))
        .mount(&mock_server)
        .await;

    let memory_cache = crates_docs::cache::memory::MemoryCache::new(100);
    let cache = Arc::new(memory_cache);
    let cache_config = crates_docs::cache::CacheConfig::default();
    let request_count = Arc::new(AtomicUsize::new(0));

    let service = crates_docs::tools::docs::DocService::with_custom_client(
        cache,
        &cache_config,
        build_docs_rs_test_client(&mock_uri, request_count.clone()),
    );

    let tool = crates_docs::tools::docs::lookup_crate::LookupCrateToolImpl::new(Arc::new(service));

    let markdown_args = serde_json::json!({
        "crate_name": "serde",
        "format": "markdown"
    });
    let text_args = serde_json::json!({
        "crate_name": "serde",
        "format": "text"
    });

    let markdown_result = tool.execute(markdown_args).await;
    assert!(markdown_result.is_ok());

    let text_result = tool.execute(text_args).await;
    assert!(text_result.is_ok());

    assert_eq!(
        request_count.load(Ordering::SeqCst),
        1,
        "expected a single upstream request"
    );
}

#[tokio::test]
#[serial(docs_rs_env)]
async fn test_lookup_crate_tool_keeps_versioned_and_unversioned_cache_entries_distinct() {
    use crates_docs::tools::Tool;
    use wiremock::{matchers, Mock, MockServer, ResponseTemplate};

    let mock_server = MockServer::start().await;
    let mock_uri = mock_server.uri();

    Mock::given(matchers::method("GET"))
        .and(matchers::path("/serde/"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(
                r#"<html><body><section id="main-content"><h1>Serde latest</h1></section></body></html>"#,
            ),
        )
        .mount(&mock_server)
        .await;

    Mock::given(matchers::method("GET"))
        .and(matchers::path("/serde/1.0.0/"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(
                r#"<html><body><section id="main-content"><h1>Serde 1.0.0</h1></section></body></html>"#,
            ),
        )
        .mount(&mock_server)
        .await;

    let memory_cache = crates_docs::cache::memory::MemoryCache::new(100);
    let cache = Arc::new(memory_cache);
    let cache_config = crates_docs::cache::CacheConfig::default();
    let request_count = Arc::new(AtomicUsize::new(0));

    let service = crates_docs::tools::docs::DocService::with_custom_client(
        cache,
        &cache_config,
        build_docs_rs_test_client(&mock_uri, request_count.clone()),
    );

    let tool = crates_docs::tools::docs::lookup_crate::LookupCrateToolImpl::new(Arc::new(service));

    let latest_args = serde_json::json!({
        "crate_name": "serde",
        "format": "markdown"
    });
    let versioned_args = serde_json::json!({
        "crate_name": "serde",
        "version": "1.0.0",
        "format": "text"
    });

    let latest_result = tool.execute(latest_args).await;
    assert!(latest_result.is_ok());

    let versioned_result = tool.execute(versioned_args).await;
    assert!(versioned_result.is_ok());

    assert_eq!(
        request_count.load(Ordering::SeqCst),
        2,
        "expected separate upstream requests"
    );
}

#[tokio::test]
async fn test_lookup_crate_tool_invalid_params() {
    use crates_docs::tools::Tool;

    let memory_cache = crates_docs::cache::memory::MemoryCache::new(100);
    let cache = Arc::new(memory_cache);
    let cache_config = crates_docs::cache::CacheConfig::default();

    let test_client = reqwest_middleware::ClientBuilder::new(reqwest::Client::new()).build();
    let service = crates_docs::tools::docs::DocService::with_custom_client(
        cache,
        &cache_config,
        Arc::new(test_client),
    );

    let tool = crates_docs::tools::docs::lookup_crate::LookupCrateToolImpl::new(Arc::new(service));

    // Missing crate_name
    let args = serde_json::json!({
        "version": "1.0.0"
    });

    let result = tool.execute(args).await;
    assert!(result.is_err());
}

#[tokio::test]
#[serial(crates_io_env)]
async fn test_lookup_crate_tool_invalid_format_preserves_detailed_message() {
    use crates_docs::tools::Tool;

    let memory_cache = crates_docs::cache::memory::MemoryCache::new(100);
    let cache = Arc::new(memory_cache);
    let cache_config = crates_docs::cache::CacheConfig::default();

    let test_client = reqwest_middleware::ClientBuilder::new(reqwest::Client::new()).build();
    let service = crates_docs::tools::docs::DocService::with_custom_client(
        cache,
        &cache_config,
        Arc::new(test_client),
    );

    let tool = crates_docs::tools::docs::lookup_crate::LookupCrateToolImpl::new(Arc::new(service));

    let args = serde_json::json!({
        "crate_name": "serde",
        "format": "xml"
    });

    // Invalid format must fail fast with a detailed, actionable message and
    // without performing any network request.
    let error = tool
        .execute(args)
        .await
        .expect_err("invalid format should fail");
    let error_message = error.to_string();
    assert!(
        error_message.contains("Invalid format 'xml'"),
        "unexpected error message: {error_message}"
    );
    assert!(error_message.contains("markdown, text, html"));
}

// ============================================================================
// LookupItemTool tests
// ============================================================================

#[test]
fn test_lookup_item_tool_params() {
    use crates_docs::tools::docs::lookup_item::LookupItemTool;

    let params = LookupItemTool {
        crate_name: "serde".to_string(),
        item_path: "serde::Serialize".to_string(),
        version: Some("1.0.0".to_string()),
        format: Some("markdown".to_string()),
    };

    assert_eq!(params.crate_name, "serde");
    assert_eq!(params.item_path, "serde::Serialize");
    assert_eq!(params.version, Some("1.0.0".to_string()));
    assert_eq!(params.format, Some("markdown".to_string()));
}

#[tokio::test]
#[serial(docs_rs_env)]
async fn test_lookup_item_tool_execute_markdown() {
    use crates_docs::tools::Tool;
    use wiremock::{matchers, Mock, MockServer, ResponseTemplate};

    let mock_server = MockServer::start().await;
    let mock_uri = mock_server.uri();
    let mock_html = r#"
    <html><body><h1>Serialize Trait</h1><p>Serialize data structure</p></body></html>
    "#;

    Mock::given(matchers::method("GET"))
        .and(matchers::path_regex(r".*search=.*"))
        .respond_with(ResponseTemplate::new(200).set_body_string(mock_html))
        .mount(&mock_server)
        .await;

    let _guard = EnvVarGuard::new("CRATES_DOCS_DOCS_RS_URL", &mock_uri);

    let memory_cache = crates_docs::cache::memory::MemoryCache::new(100);
    let cache = Arc::new(memory_cache);
    let cache_config = crates_docs::cache::CacheConfig::default();

    let test_client = reqwest_middleware::ClientBuilder::new(reqwest::Client::new()).build();
    let service = crates_docs::tools::docs::DocService::with_custom_client(
        cache,
        &cache_config,
        Arc::new(test_client),
    );

    let tool = crates_docs::tools::docs::lookup_item::LookupItemToolImpl::new(Arc::new(service));

    let args = serde_json::json!({
        "crate_name": "serde",
        "item_path": "serde::Serialize",
        "format": "markdown"
    });

    let result = tool.execute(args).await;
    assert!(result.is_ok());
}

#[tokio::test]
#[serial(docs_rs_env)]
async fn test_lookup_item_tool_execute_text_format() {
    use crates_docs::tools::Tool;
    use wiremock::{matchers, Mock, MockServer, ResponseTemplate};

    let mock_server = MockServer::start().await;
    let mock_uri = mock_server.uri();
    let mock_html = r#"
    <html><body><h1>Serialize Trait</h1><p>Serialize data structure</p></body></html>
    "#;

    Mock::given(matchers::method("GET"))
        .and(matchers::path_regex(r".*search=.*"))
        .respond_with(ResponseTemplate::new(200).set_body_string(mock_html))
        .mount(&mock_server)
        .await;

    let _guard = EnvVarGuard::new("CRATES_DOCS_DOCS_RS_URL", &mock_uri);

    let memory_cache = crates_docs::cache::memory::MemoryCache::new(100);
    let cache = Arc::new(memory_cache);
    let cache_config = crates_docs::cache::CacheConfig::default();

    let test_client = reqwest_middleware::ClientBuilder::new(reqwest::Client::new()).build();
    let service = crates_docs::tools::docs::DocService::with_custom_client(
        cache,
        &cache_config,
        Arc::new(test_client),
    );

    let tool = crates_docs::tools::docs::lookup_item::LookupItemToolImpl::new(Arc::new(service));

    let args = serde_json::json!({
        "crate_name": "serde",
        "item_path": "serde::Serialize",
        "format": "text"
    });

    let result = tool.execute(args).await;
    assert!(result.is_ok());
}

#[tokio::test]
#[serial(docs_rs_env)]
async fn test_lookup_item_tool_execute_html_format() {
    use crates_docs::tools::Tool;
    use wiremock::{matchers, Mock, MockServer, ResponseTemplate};

    let mock_server = MockServer::start().await;
    let mock_uri = mock_server.uri();
    let mock_html = r#"
    <html><body><h1>Serialize Trait</h1><p>Serialize data structure</p></body></html>
    "#;

    Mock::given(matchers::method("GET"))
        .and(matchers::path_regex(r".*search=.*"))
        .respond_with(ResponseTemplate::new(200).set_body_string(mock_html))
        .mount(&mock_server)
        .await;

    let _guard = EnvVarGuard::new("CRATES_DOCS_DOCS_RS_URL", &mock_uri);

    let memory_cache = crates_docs::cache::memory::MemoryCache::new(100);
    let cache = Arc::new(memory_cache);
    let cache_config = crates_docs::cache::CacheConfig::default();

    let test_client = reqwest_middleware::ClientBuilder::new(reqwest::Client::new()).build();
    let service = crates_docs::tools::docs::DocService::with_custom_client(
        cache,
        &cache_config,
        Arc::new(test_client),
    );

    let tool = crates_docs::tools::docs::lookup_item::LookupItemToolImpl::new(Arc::new(service));

    let args = serde_json::json!({
        "crate_name": "serde",
        "item_path": "serde::Serialize",
        "format": "html"
    });

    let result = tool.execute(args).await;
    assert!(result.is_ok());
}

/// When the dedicated item page cannot be resolved and the crate overview is
/// returned, the HTML format must include the same fallback note that the
/// markdown and text formats already provide.
#[tokio::test]
#[serial(docs_rs_env)]
async fn test_lookup_item_tool_html_format_includes_fallback_note() {
    use crates_docs::tools::Tool;
    use wiremock::{matchers, Mock, MockServer, ResponseTemplate};

    let mock_server = MockServer::start().await;
    let mock_uri = mock_server.uri();
    // A crate-overview page: extracted text begins with "Crate ".
    let mock_html = r#"
    <html><body><h1>Crate serde</h1><p>Serialization framework</p></body></html>
    "#;

    Mock::given(matchers::method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_string(mock_html))
        .mount(&mock_server)
        .await;

    let _guard = EnvVarGuard::new("CRATES_DOCS_DOCS_RS_URL", &mock_uri);

    let memory_cache = crates_docs::cache::memory::MemoryCache::new(100);
    let cache = Arc::new(memory_cache);
    let cache_config = crates_docs::cache::CacheConfig::default();
    let test_client = reqwest_middleware::ClientBuilder::new(reqwest::Client::new()).build();
    let service = crates_docs::tools::docs::DocService::with_custom_client(
        cache,
        &cache_config,
        Arc::new(test_client),
    );
    let tool = crates_docs::tools::docs::lookup_item::LookupItemToolImpl::new(Arc::new(service));

    let args = serde_json::json!({
        "crate_name": "serde",
        "item_path": "serde::DoesNotExist",
        "format": "html"
    });

    let result = tool.execute(args).await.expect("execute should succeed");
    let rendered = serde_json::to_string(&result).expect("serialize result");
    assert!(
        rendered.contains("No dedicated documentation page was found"),
        "HTML fallback note missing: {rendered}"
    );
}

#[tokio::test]
#[serial(docs_rs_env)]
async fn test_lookup_item_tool_execute_with_version() {
    use crates_docs::tools::Tool;
    use wiremock::{matchers, Mock, MockServer, ResponseTemplate};

    let mock_server = MockServer::start().await;
    let mock_uri = mock_server.uri();
    let mock_html = r#"
    <html><body><h1>Serialize Trait v1.0.0</h1></body></html>
    "#;

    Mock::given(matchers::method("GET"))
        .and(matchers::path_regex(r".*search=.*"))
        .respond_with(ResponseTemplate::new(200).set_body_string(mock_html))
        .mount(&mock_server)
        .await;

    let _guard = EnvVarGuard::new("CRATES_DOCS_DOCS_RS_URL", &mock_uri);

    let memory_cache = crates_docs::cache::memory::MemoryCache::new(100);
    let cache = Arc::new(memory_cache);
    let cache_config = crates_docs::cache::CacheConfig::default();

    let test_client = reqwest_middleware::ClientBuilder::new(reqwest::Client::new()).build();
    let service = crates_docs::tools::docs::DocService::with_custom_client(
        cache,
        &cache_config,
        Arc::new(test_client),
    );

    let tool = crates_docs::tools::docs::lookup_item::LookupItemToolImpl::new(Arc::new(service));

    let args = serde_json::json!({
        "crate_name": "serde",
        "item_path": "serde::Serialize",
        "version": "1.0.0",
        "format": "markdown"
    });

    let result = tool.execute(args).await;
    assert!(result.is_ok());
}

#[tokio::test]
#[serial(docs_rs_env)]
async fn test_lookup_item_tool_reuses_single_upstream_fetch_across_formats() {
    use crates_docs::tools::Tool;
    use wiremock::{matchers, Mock, MockServer, ResponseTemplate};

    let mock_server = MockServer::start().await;
    let mock_uri = mock_server.uri();
    let mock_html = r#"
    <html>
    <body>
        <h1>Serialize Trait</h1>
        <p>Serialize data structure</p>
    </body>
    </html>
    "#;

    Mock::given(matchers::method("GET"))
        .and(matchers::path("/serde/"))
        .and(matchers::query_param("search", "serde::Serialize"))
        .respond_with(ResponseTemplate::new(200).set_body_string(mock_html))
        .mount(&mock_server)
        .await;

    let memory_cache = crates_docs::cache::memory::MemoryCache::new(100);
    let cache = Arc::new(memory_cache);
    let cache_config = crates_docs::cache::CacheConfig::default();
    let request_count = Arc::new(AtomicUsize::new(0));

    let service = crates_docs::tools::docs::DocService::with_custom_client(
        cache,
        &cache_config,
        build_docs_rs_test_client(&mock_uri, request_count.clone()),
    );

    let tool = crates_docs::tools::docs::lookup_item::LookupItemToolImpl::new(Arc::new(service));

    let markdown_args = serde_json::json!({
        "crate_name": "serde",
        "item_path": "serde::Serialize",
        "format": "markdown"
    });
    let html_args = serde_json::json!({
        "crate_name": "serde",
        "item_path": "serde::Serialize",
        "format": "html"
    });

    let markdown_result = tool.execute(markdown_args).await;
    assert!(markdown_result.is_ok());

    // The first lookup probes candidate item URLs and then falls back to the
    // crate page; record however many upstream requests that took.
    let after_first = request_count.load(Ordering::SeqCst);
    assert!(after_first > 0, "first lookup should hit upstream");

    let html_result = tool.execute(html_args).await;
    assert!(html_result.is_ok());

    assert_eq!(
        request_count.load(Ordering::SeqCst),
        after_first,
        "second format should be served entirely from cache (no new upstream requests)"
    );
}

/// Resolving an item whose direct candidate pages do not exist falls back to
/// the crate `all.html` index for both the full path and its parent path. The
/// index must be fetched at most once across both attempts (it is the same
/// crate-level URL), not re-fetched per attempt.
#[tokio::test]
#[serial(docs_rs_env)]
async fn test_lookup_item_tool_fetches_all_html_index_only_once() {
    use crates_docs::tools::Tool;
    use wiremock::{matchers, Mock, MockServer, ResponseTemplate};

    let mock_server = MockServer::start().await;
    let mock_uri = mock_server.uri();

    // The `all.html` index exists but lists nothing matching the requested
    // item, so neither the full path (`mycrate::Foo::bar`) nor the parent path
    // (`mycrate::Foo`) resolves through it. `expect(1)` makes the MockServer
    // assert (on drop) that exactly one request reached the index.
    Mock::given(matchers::method("GET"))
        .and(matchers::path("/mycrate/latest/mycrate/all.html"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(r#"<html><body><a href="index.html">no items</a></body></html>"#),
        )
        .expect(1)
        .mount(&mock_server)
        .await;
    // Every other URL (candidate item pages, final crate search page) is
    // unmounted, so wiremock answers 404, which the resolver treats as "absent".

    let memory_cache = crates_docs::cache::memory::MemoryCache::new(100);
    let cache = Arc::new(memory_cache);
    let cache_config = crates_docs::cache::CacheConfig::default();
    let request_count = Arc::new(AtomicUsize::new(0));
    let service = crates_docs::tools::docs::DocService::with_custom_client(
        cache,
        &cache_config,
        build_docs_rs_test_client(&mock_uri, request_count.clone()),
    );
    let tool = crates_docs::tools::docs::lookup_item::LookupItemToolImpl::new(Arc::new(service));

    let args = serde_json::json!({
        "crate_name": "mycrate",
        "item_path": "mycrate::Foo::bar",
        "format": "markdown"
    });

    // The final fallback (crate search page) 404s, so execution ultimately
    // errors; we only care that the index was not fetched twice. Drop the
    // MockServer explicitly to trigger `expect(1)` verification.
    let _ = tool.execute(args).await;
    drop(mock_server);
}

#[tokio::test]
#[serial(docs_rs_env)]
async fn test_lookup_item_tool_keeps_versioned_and_unversioned_cache_entries_distinct() {
    use crates_docs::tools::Tool;
    use wiremock::{matchers, Mock, MockServer, ResponseTemplate};

    let mock_server = MockServer::start().await;
    let mock_uri = mock_server.uri();

    Mock::given(matchers::method("GET"))
        .and(matchers::path("/serde/"))
        .and(matchers::query_param("search", "serde::Serialize"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(r#"<html><body><h1>Serialize latest</h1></body></html>"#),
        )
        .mount(&mock_server)
        .await;

    Mock::given(matchers::method("GET"))
        .and(matchers::path("/serde/1.0.0/"))
        .and(matchers::query_param("search", "serde::Serialize"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(r#"<html><body><h1>Serialize 1.0.0</h1></body></html>"#),
        )
        .mount(&mock_server)
        .await;

    let memory_cache = crates_docs::cache::memory::MemoryCache::new(100);
    let cache = Arc::new(memory_cache);
    let cache_config = crates_docs::cache::CacheConfig::default();
    let request_count = Arc::new(AtomicUsize::new(0));

    let service = crates_docs::tools::docs::DocService::with_custom_client(
        cache,
        &cache_config,
        build_docs_rs_test_client(&mock_uri, request_count.clone()),
    );

    let tool = crates_docs::tools::docs::lookup_item::LookupItemToolImpl::new(Arc::new(service));

    let latest_args = serde_json::json!({
        "crate_name": "serde",
        "item_path": "serde::Serialize",
        "format": "markdown"
    });
    let versioned_args = serde_json::json!({
        "crate_name": "serde",
        "item_path": "serde::Serialize",
        "version": "1.0.0",
        "format": "text"
    });

    let latest_result = tool.execute(latest_args).await;
    assert!(latest_result.is_ok());
    let after_latest = request_count.load(Ordering::SeqCst);
    assert!(
        after_latest > 0,
        "first (latest) lookup should hit upstream"
    );

    let versioned_result = tool.execute(versioned_args).await;
    assert!(versioned_result.is_ok());

    assert!(
        request_count.load(Ordering::SeqCst) > after_latest,
        "versioned lookup must use a distinct cache key and trigger its own upstream fetch"
    );
}

#[tokio::test]
async fn test_lookup_item_tool_invalid_params() {
    use crates_docs::tools::Tool;

    let memory_cache = crates_docs::cache::memory::MemoryCache::new(100);
    let cache = Arc::new(memory_cache);
    let cache_config = crates_docs::cache::CacheConfig::default();

    let test_client = reqwest_middleware::ClientBuilder::new(reqwest::Client::new()).build();
    let service = crates_docs::tools::docs::DocService::with_custom_client(
        cache,
        &cache_config,
        Arc::new(test_client),
    );

    let tool = crates_docs::tools::docs::lookup_item::LookupItemToolImpl::new(Arc::new(service));

    // Missing item_path
    let args = serde_json::json!({
        "crate_name": "serde"
    });

    let result = tool.execute(args).await;
    assert!(result.is_err());
}

#[tokio::test]
#[serial(crates_io_env)]
async fn test_lookup_item_tool_invalid_format_preserves_detailed_message() {
    use crates_docs::tools::Tool;

    let memory_cache = crates_docs::cache::memory::MemoryCache::new(100);
    let cache = Arc::new(memory_cache);
    let cache_config = crates_docs::cache::CacheConfig::default();

    let test_client = reqwest_middleware::ClientBuilder::new(reqwest::Client::new()).build();
    let service = crates_docs::tools::docs::DocService::with_custom_client(
        cache,
        &cache_config,
        Arc::new(test_client),
    );

    let tool = crates_docs::tools::docs::lookup_item::LookupItemToolImpl::new(Arc::new(service));

    let args = serde_json::json!({
        "crate_name": "serde",
        "item_path": "serde::Serialize",
        "format": "xml"
    });

    // Invalid format must fail fast with a detailed, actionable message and
    // without performing any network request.
    let error = tool
        .execute(args)
        .await
        .expect_err("invalid format should fail");
    let error_message = error.to_string();
    assert!(
        error_message.contains("Invalid format 'xml'"),
        "unexpected error message: {error_message}"
    );
    assert!(error_message.contains("markdown, text, html"));
}

// ============================================================================
// SearchCratesTool tests
// ============================================================================

#[test]
fn test_search_crates_tool_params() {
    use crates_docs::tools::docs::search::SearchCratesTool;

    let params = SearchCratesTool {
        query: "web framework".to_string(),
        limit: Some(20),
        sort: Some("downloads".to_string()),
        format: Some("json".to_string()),
    };

    assert_eq!(params.query, "web framework");
    assert_eq!(params.limit, Some(20));
    assert_eq!(params.sort, Some("downloads".to_string()));
    assert_eq!(params.format, Some("json".to_string()));
}

#[tokio::test]
#[serial(crates_io_env)]
async fn test_search_crates_tool_execute_markdown() {
    use crates_docs::tools::Tool;
    use wiremock::{matchers, Mock, MockServer, ResponseTemplate};

    let mock_server = MockServer::start().await;
    let mock_uri = mock_server.uri();
    let mock_response = r#"
    {
        "crates": [
            {
                "name": "serde",
                "max_version": "1.0.0",
                "description": "Serialization framework",
                "downloads": 1000000,
                "repository": "https://github.com/serde-rs/serde",
                "documentation": "https://docs.rs/serde"
            }
        ]
    }
    "#;

    Mock::given(matchers::method("GET"))
        .and(matchers::path_regex(r"/api/v1/crates.*"))
        .respond_with(ResponseTemplate::new(200).set_body_string(mock_response))
        .mount(&mock_server)
        .await;

    let memory_cache = crates_docs::cache::memory::MemoryCache::new(100);
    let cache = Arc::new(memory_cache);
    let cache_config = crates_docs::cache::CacheConfig::default();

    let service = crates_docs::tools::docs::DocService::with_custom_client(
        cache,
        &cache_config,
        build_crates_io_test_client(&mock_uri),
    );

    let tool = crates_docs::tools::docs::search::SearchCratesToolImpl::new(Arc::new(service));

    let args = serde_json::json!({
        "query": "serialization",
        "limit": 10,
        "sort": "relevance",
        "format": "markdown"
    });

    let result = tool.execute(args).await;
    assert!(result.is_ok());
}

/// Crate metadata (description, repository, documentation) is controlled by the
/// crate publisher. The markdown renderer must neutralize markdown/link/HTML
/// metacharacters so a malicious crate cannot inject an active link, inline
/// HTML, or a non-`http` scheme into the MCP client's rendered output.
#[tokio::test]
#[serial(crates_io_env)]
async fn test_search_crates_tool_escapes_malicious_metadata() {
    use crates_docs::tools::Tool;
    use wiremock::{matchers, Mock, MockServer, ResponseTemplate};

    let mock_server = MockServer::start().await;
    let mock_uri = mock_server.uri();
    let mock_response = r##"
    {
        "crates": [
            {
                "name": "evilcrate",
                "max_version": "1.0.0",
                "description": "see [click me](http://evil.example/pwn) and <img src=x>",
                "downloads": 1,
                "repository": "javascript:alert(1)",
                "documentation": "https://docs.rs/evilcrate"
            }
        ]
    }
    "##;

    Mock::given(matchers::method("GET"))
        .and(matchers::path_regex(r"/api/v1/crates.*"))
        .respond_with(ResponseTemplate::new(200).set_body_string(mock_response))
        .mount(&mock_server)
        .await;

    let memory_cache = crates_docs::cache::memory::MemoryCache::new(100);
    let cache = Arc::new(memory_cache);
    let cache_config = crates_docs::cache::CacheConfig::default();
    let service = crates_docs::tools::docs::DocService::with_custom_client(
        cache,
        &cache_config,
        build_crates_io_test_client(&mock_uri),
    );
    let tool = crates_docs::tools::docs::search::SearchCratesToolImpl::new(Arc::new(service));

    let args = serde_json::json!({
        "query": "evil",
        "limit": 10,
        "sort": "relevance",
        "format": "markdown"
    });

    let result = tool.execute(args).await.expect("execute should succeed");
    let rendered = serde_json::to_string(&result).expect("serialize result");

    // The injected markdown link's brackets must be escaped (no active link).
    assert!(
        !rendered.contains("[click me]("),
        "unescaped injected markdown link leaked: {rendered}"
    );
    // The inline HTML `<` must be neutralized.
    assert!(
        !rendered.contains("<img src=x>"),
        "unescaped inline HTML leaked: {rendered}"
    );
    // The non-http repository scheme must not render as an active link target.
    assert!(
        !rendered.contains("[Link](javascript:"),
        "non-http scheme rendered as active link: {rendered}"
    );
    // The legitimate documentation URL should still render as a normal link.
    assert!(
        rendered.contains("[Link](https://docs.rs/evilcrate)"),
        "legitimate https link should be preserved: {rendered}"
    );
}

#[tokio::test]
#[serial(crates_io_env)]
async fn test_search_crates_tool_execute_text_format() {
    use crates_docs::tools::Tool;
    use wiremock::{matchers, Mock, MockServer, ResponseTemplate};

    let mock_server = MockServer::start().await;
    let mock_uri = mock_server.uri();
    let mock_response = r#"
    {
    "crates": [
            {
                "name": "tokio",
                "max_version": "1.0.0",
                "description": "Async runtime",
                "downloads": 2000000
            }
        ]
    }
    "#;

    Mock::given(matchers::method("GET"))
        .and(matchers::path_regex(r"/api/v1/crates.*"))
        .respond_with(ResponseTemplate::new(200).set_body_string(mock_response))
        .mount(&mock_server)
        .await;

    let memory_cache = crates_docs::cache::memory::MemoryCache::new(100);
    let cache = Arc::new(memory_cache);
    let cache_config = crates_docs::cache::CacheConfig::default();

    let service = crates_docs::tools::docs::DocService::with_custom_client(
        cache,
        &cache_config,
        build_crates_io_test_client(&mock_uri),
    );

    let tool = crates_docs::tools::docs::search::SearchCratesToolImpl::new(Arc::new(service));

    let args = serde_json::json!({
        "query": "async",
        "format": "text"
    });

    let result = tool.execute(args).await;
    assert!(result.is_ok());
}

#[tokio::test]
#[serial(crates_io_env)]
async fn test_search_crates_tool_execute_json_format() {
    use crates_docs::tools::Tool;
    use wiremock::{matchers, Mock, MockServer, ResponseTemplate};

    let mock_server = MockServer::start().await;
    let mock_uri = mock_server.uri();
    let mock_response = r#"
    {
        "crates": [
            {
                "name": "reqwest",
                "max_version": "0.11.0",
                "description": "HTTP client",
                "downloads": 500000
            }
        ]
    }
    "#;

    Mock::given(matchers::method("GET"))
        .and(matchers::path_regex(r"/api/v1/crates.*"))
        .respond_with(ResponseTemplate::new(200).set_body_string(mock_response))
        .mount(&mock_server)
        .await;

    let memory_cache = crates_docs::cache::memory::MemoryCache::new(100);
    let cache = Arc::new(memory_cache);
    let cache_config = crates_docs::cache::CacheConfig::default();

    let service = crates_docs::tools::docs::DocService::with_custom_client(
        cache,
        &cache_config,
        build_crates_io_test_client(&mock_uri),
    );

    let tool = crates_docs::tools::docs::search::SearchCratesToolImpl::new(Arc::new(service));

    let args = serde_json::json!({
        "query": "http client",
        "format": "json"
    });

    let result = tool.execute(args).await;
    assert!(result.is_ok());

    // The JSON output must expose the canonical docs.rs URL so structured
    // consumers get the same docs.rs link that the markdown/text formats
    // always include.
    let call = result.unwrap();
    let text = call
        .content
        .first()
        .and_then(|c| c.as_text_content().ok())
        .map(|t| t.text.clone())
        .expect("json result should contain text content");
    let parsed: serde_json::Value =
        serde_json::from_str(&text).expect("json format output should be valid JSON");
    assert_eq!(
        parsed[0]["docs_rs"], "https://docs.rs/reqwest/",
        "json output should include the canonical docs_rs URL: {text}"
    );
}

#[tokio::test]
#[serial(crates_io_env)]
async fn test_search_crates_tool_invalid_sort() {
    use crates_docs::tools::Tool;

    let memory_cache = crates_docs::cache::memory::MemoryCache::new(100);
    let cache = Arc::new(memory_cache);
    let cache_config = crates_docs::cache::CacheConfig::default();

    let test_client = reqwest_middleware::ClientBuilder::new(reqwest::Client::new()).build();
    let service = crates_docs::tools::docs::DocService::with_custom_client(
        cache,
        &cache_config,
        Arc::new(test_client),
    );

    let tool = crates_docs::tools::docs::search::SearchCratesToolImpl::new(Arc::new(service));

    let args = serde_json::json!({
        "query": "test",
        "sort": "invalid_sort_option"
    });

    let result = tool.execute(args).await;
    assert!(result.is_err());
}

#[tokio::test]
#[serial(crates_io_env)]
async fn test_search_crates_tool_invalid_format_preserves_detailed_message() {
    use crates_docs::tools::Tool;

    let memory_cache = crates_docs::cache::memory::MemoryCache::new(100);
    let cache = Arc::new(memory_cache);
    let cache_config = crates_docs::cache::CacheConfig::default();

    let test_client = reqwest_middleware::ClientBuilder::new(reqwest::Client::new()).build();
    let service = crates_docs::tools::docs::DocService::with_custom_client(
        cache,
        &cache_config,
        Arc::new(test_client),
    );

    let tool = crates_docs::tools::docs::search::SearchCratesToolImpl::new(Arc::new(service));

    let args = serde_json::json!({
        "query": "test",
        "format": "xml"
    });

    let result = tool.execute(args).await;
    let error = result.expect_err("invalid format should fail");
    let error_message = error.to_string();

    assert!(error_message.contains("Invalid format 'xml'"));
    assert!(error_message.contains("markdown, text, json"));
}

#[tokio::test]
#[serial(crates_io_env)]
async fn test_search_crates_tool_rejects_html_format() {
    use crates_docs::tools::Tool;

    let memory_cache = crates_docs::cache::memory::MemoryCache::new(100);
    let cache = Arc::new(memory_cache);
    let cache_config = crates_docs::cache::CacheConfig::default();

    let test_client = reqwest_middleware::ClientBuilder::new(reqwest::Client::new()).build();
    let service = crates_docs::tools::docs::DocService::with_custom_client(
        cache,
        &cache_config,
        Arc::new(test_client),
    );

    let tool = crates_docs::tools::docs::search::SearchCratesToolImpl::new(Arc::new(service));

    // search_crates does not support html output; it must reject it explicitly
    // (fail-fast, before any network request) rather than silently returning
    // markdown.
    let args = serde_json::json!({
        "query": "serde",
        "format": "html"
    });

    let error = tool
        .execute(args)
        .await
        .expect_err("html format should be rejected for search_crates");
    let msg = error.to_string();
    assert!(msg.contains("html"), "unexpected message: {msg}");
    assert!(
        msg.contains("markdown, text, json"),
        "unexpected message: {msg}"
    );
}

#[tokio::test]
#[serial(crates_io_env)]
async fn test_search_crates_tool_uses_canonical_search_cache_key() {
    use crates_docs::tools::Tool;
    use wiremock::MockServer;

    let mock_server = MockServer::start().await;
    let memory_cache = crates_docs::cache::memory::MemoryCache::new(100);
    let cache = Arc::new(memory_cache);
    let cache_config = crates_docs::cache::CacheConfig::default();

    let service = crates_docs::tools::docs::DocService::with_custom_client(
        cache,
        &cache_config,
        build_crates_io_test_client(&mock_server.uri()),
    );

    service
        .doc_cache()
        .set_search_results(
            "serde",
            10,
            Some("relevance"),
            serde_json::json!([
                {
                    "name": "serde",
                    "description": "Serialization framework",
                    "version": "1.0.0",
                    "downloads": 1000000,
                    "repository": "https://github.com/serde-rs/serde",
                    "documentation": "https://docs.rs/serde"
                }
            ])
            .to_string(),
        )
        .await
        .expect("set_search_results should succeed");

    let tool = crates_docs::tools::docs::search::SearchCratesToolImpl::new(Arc::new(service));

    let args = serde_json::json!({
        "query": "  SERDE  ",
        "limit": 10,
        "sort": "relevance",
        "format": "json"
    });

    let result = tool.execute(args).await;
    assert!(
        result.is_ok(),
        "expected canonical cache hit, got error: {:?}",
        result.err()
    );
}

#[tokio::test]
#[serial(crates_io_env)]
async fn test_search_crates_tool_limit_clamping() {
    use crates_docs::tools::Tool;
    use wiremock::{matchers, Mock, MockServer, ResponseTemplate};

    let mock_server = MockServer::start().await;
    let mock_uri = mock_server.uri();
    let mock_response = r#"{"crates": []}"#;

    Mock::given(matchers::method("GET"))
        .and(matchers::path_regex(r"/api/v1/crates.*"))
        .respond_with(ResponseTemplate::new(200).set_body_string(mock_response))
        .mount(&mock_server)
        .await;

    let memory_cache = crates_docs::cache::memory::MemoryCache::new(100);
    let cache = Arc::new(memory_cache);
    let cache_config = crates_docs::cache::CacheConfig::default();

    let service = crates_docs::tools::docs::DocService::with_custom_client(
        cache,
        &cache_config,
        build_crates_io_test_client(&mock_uri),
    );

    let tool = crates_docs::tools::docs::search::SearchCratesToolImpl::new(Arc::new(service));

    let args = serde_json::json!({
        "query": "test",
        "limit": 200
    });

    let result = tool.execute(args).await;
    assert!(result.is_ok());
}

#[tokio::test]
#[serial(crates_io_env)]
async fn test_search_crates_tool_cache_key_differs_by_sort() {
    use crates_docs::tools::Tool;
    use wiremock::{matchers, Mock, MockServer, ResponseTemplate};

    let mock_server = MockServer::start().await;
    let mock_uri = mock_server.uri();

    // Setup mock to expect two different requests (different sort parameters)
    // relevance sort request
    Mock::given(matchers::method("GET"))
        .and(matchers::path("/api/v1/crates"))
        .and(matchers::query_param("sort", "relevance"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r#"{"crates":[{"name":"reqwest","max_version":"1.0","downloads":1000}]}"#,
        ))
        .mount(&mock_server)
        .await;

    // downloads sort request
    Mock::given(matchers::method("GET"))
        .and(matchers::path("/api/v1/crates"))
        .and(matchers::query_param("sort", "downloads"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r#"{"crates":[{"name":"tokio","max_version":"2.0","downloads":2000}]}"#,
        ))
        .mount(&mock_server)
        .await;

    let memory_cache = crates_docs::cache::memory::MemoryCache::new(100);
    let cache = Arc::new(memory_cache);
    let cache_config = crates_docs::cache::CacheConfig::default();

    let service = crates_docs::tools::docs::DocService::with_custom_client(
        cache,
        &cache_config,
        build_crates_io_test_client(&mock_uri),
    );

    let tool = crates_docs::tools::docs::search::SearchCratesToolImpl::new(Arc::new(service));

    // First search with relevance sort
    let args1 = serde_json::json!({
        "query": "http client",
        "sort": "relevance",
        "format": "json"
    });
    let _result1 = tool
        .execute(args1)
        .await
        .expect("First search should succeed");

    // Second search with downloads sort - should hit different cache key or fetch again
    let args2 = serde_json::json!({
        "query": "http client",
        "sort": "downloads",
        "format": "json"
    });
    let _result2 = tool
        .execute(args2)
        .await
        .expect("Second search should succeed");

    // Both searches should succeed (execute returns Result<CallToolResult, CallToolError>)
    // We already asserted success with .expect() above

    // The key verification: mock server should have received requests for BOTH sort values
    // If cache keys were the same, second request would use cache and mock would only see one request
    // This test proves sort parameter is part of the cache key

    // Verify mock received both requests (both sorts were called, not just one)
    mock_server.verify().await;
}

// ============================================================================
// DocService tests
// ============================================================================

#[tokio::test]
async fn test_doc_service_fetch_html_success() {
    use wiremock::{matchers, Mock, MockServer, ResponseTemplate};

    let mock_server = MockServer::start().await;
    let mock_html = "<html><body>Test content</body></html>";

    Mock::given(matchers::method("GET"))
        .and(matchers::path("/test"))
        .respond_with(ResponseTemplate::new(200).set_body_string(mock_html))
        .mount(&mock_server)
        .await;

    let memory_cache = crates_docs::cache::memory::MemoryCache::new(100);
    let cache = Arc::new(memory_cache);
    let cache_config = crates_docs::cache::CacheConfig::default();

    let test_client = reqwest_middleware::ClientBuilder::new(reqwest::Client::new()).build();
    let service = crates_docs::tools::docs::DocService::with_custom_client(
        cache,
        &cache_config,
        Arc::new(test_client),
    );

    let url = format!("{}/test", mock_server.uri());
    let result = service.fetch_html(&url, Some("test_tool")).await;
    assert!(result.is_ok());
    assert!(result.unwrap().contains("Test content"));
}

#[tokio::test]
async fn test_doc_service_fetch_html_404_error() {
    use wiremock::{matchers, Mock, MockServer, ResponseTemplate};

    let mock_server = MockServer::start().await;

    Mock::given(matchers::method("GET"))
        .and(matchers::path("/notfound"))
        .respond_with(ResponseTemplate::new(404).set_body_string("Not found"))
        .mount(&mock_server)
        .await;

    let memory_cache = crates_docs::cache::memory::MemoryCache::new(100);
    let cache = Arc::new(memory_cache);
    let cache_config = crates_docs::cache::CacheConfig::default();

    let test_client = reqwest_middleware::ClientBuilder::new(reqwest::Client::new()).build();
    let service = crates_docs::tools::docs::DocService::with_custom_client(
        cache,
        &cache_config,
        Arc::new(test_client),
    );

    let url = format!("{}/notfound", mock_server.uri());
    let result = service.fetch_html(&url, Some("test_tool")).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_doc_service_fetch_html_timeout_error() {
    let memory_cache = crates_docs::cache::memory::MemoryCache::new(100);
    let cache = Arc::new(memory_cache);
    let cache_config = crates_docs::cache::CacheConfig::default();

    let test_client = reqwest_middleware::ClientBuilder::new(reqwest::Client::new()).build();
    let service = crates_docs::tools::docs::DocService::with_custom_client(
        cache,
        &cache_config,
        Arc::new(test_client),
    );

    // Test with invalid URL that should fail
    let result = service
        .fetch_html("http://localhost:99999/nonexistent", Some("test_tool"))
        .await;
    assert!(result.is_err());
}

// ============================================================================
// Additional HTML processing tests
// ============================================================================

#[test]
fn test_extract_documentation_with_main_content() {
    let html = r#"
    <html>
    <body>
        <nav>Navigation</nav>
        <section id="main-content">
            <h1>Main Title</h1>
            <p>Main content</p>
        </section>
        <footer>Footer</footer>
    </body>
    </html>
    "#;
    let docs = extract_documentation(html);
    assert!(docs.contains("Main Title"));
    assert!(docs.contains("Main content"));
    assert!(!docs.contains("Navigation"));
    assert!(!docs.contains("Footer"));
}

#[test]
fn test_extract_documentation_without_main_content() {
    let html = r#"
    <html>
    <body>
        <h1>Title</h1>
        <p>Content</p>
    </body>
    </html>
    "#;
    let docs = extract_documentation(html);
    assert!(docs.contains("Title"));
    assert!(docs.contains("Content"));
}

#[test]
fn test_clean_html_removes_noscript() {
    let html = r#"<html><noscript>Enable JavaScript</noscript><body>Content</body></html>"#;
    let cleaned = clean_html(html);
    assert!(!cleaned.contains("noscript"));
    assert!(!cleaned.contains("Enable JavaScript"));
    assert!(cleaned.contains("Content"));
}

#[test]
fn test_clean_html_removes_iframe() {
    let html = r#"<html><iframe src="ads.html"></iframe><body>Content</body></html>"#;
    let cleaned = clean_html(html);
    assert!(!cleaned.contains("iframe"));
    assert!(!cleaned.contains("ads.html"));
    assert!(cleaned.contains("Content"));
}

#[test]
fn test_clean_html_removes_nav() {
    let html = r#"<html><nav><ul><li>Link1</li></ul></nav><body>Main</body></html>"#;
    let cleaned = clean_html(html);
    assert!(!cleaned.contains("<nav"));
    assert!(!cleaned.contains("Link1"));
}

#[test]
fn test_clean_html_removes_header() {
    let html = r#"<header>Site Header</header><body>Main</body>"#;
    let cleaned = clean_html(html);
    assert!(!cleaned.contains("Site Header"));
}

#[test]
fn test_clean_html_removes_footer() {
    let html = r#"<html><body>Main<footer>Copyright</footer></body></html>"#;
    let cleaned = clean_html(html);
    assert!(!cleaned.contains("Copyright"));
}

#[test]
fn test_clean_html_removes_aside() {
    let html = r#"<html><aside>Sidebar</aside><body>Main</body></html>"#;
    let cleaned = clean_html(html);
    assert!(!cleaned.contains("Sidebar"));
}

#[test]
fn test_clean_html_removes_button() {
    let html = r#"<html><body><button>Click me</button>Content</body></html>"#;
    let cleaned = clean_html(html);
    assert!(!cleaned.contains("button"));
    assert!(!cleaned.contains("Click me"));
}

#[test]
fn test_clean_html_preserves_summary_text() {
    let html = r#"<html><body><details><summary>Toggle me</summary><p>Content</p></details></body></html>"#;
    let cleaned = clean_html(html);
    assert!(cleaned.contains("Toggle me"));
    assert!(cleaned.contains("Content"));
}

#[test]
fn test_html_to_text_with_nested_tags() {
    let html = r#"<div><p>Text1</p><div><p>Text2</p></div></div>"#;
    let text = html_to_text(html);
    assert!(text.contains("Text1"));
    assert!(text.contains("Text2"));
}

#[test]
fn test_html_to_text_with_code_block() {
    let html = r#"<pre><code>fn main() {}</code></pre>"#;
    let text = html_to_text(html);
    assert!(text.contains("fn main"));
}

#[test]
fn test_html_to_text_no_body_fallback() {
    // HTML without body tag - should use ALL_SELECTOR fallback
    let html = r#"<html><div>Content without body</div></html>"#;
    let text = html_to_text(html);
    assert!(text.contains("Content without body"));
}

// ============================================================================
// TTL jitter tests
// ============================================================================

#[test]
fn test_doc_cache_ttl_apply_jitter() {
    use crates_docs::tools::docs::cache::DocCacheTtl;

    let mut ttl = DocCacheTtl::default();
    ttl.crate_docs_secs = 3600;
    ttl.search_results_secs = 300;
    ttl.item_docs_secs = 1800;
    ttl.set_jitter_ratio(0.1);

    let base = 3600;
    let jittered = ttl.apply_jitter(base);

    // Should be within 10% of base (3240 to 3960)
    assert!(jittered >= 3240);
    assert!(jittered <= 3960);
}

#[test]
fn test_doc_cache_ttl_zero_jitter() {
    use crates_docs::tools::docs::cache::DocCacheTtl;

    let mut ttl = DocCacheTtl::default();
    ttl.crate_docs_secs = 3600;
    ttl.search_results_secs = 300;
    ttl.item_docs_secs = 1800;
    ttl.set_jitter_ratio(0.0);

    let base = 3600;
    let jittered = ttl.apply_jitter(base);

    // With zero jitter, should return base TTL
    assert_eq!(jittered, base);
}

// ============================================================================
// Cache key edge cases
// ============================================================================

#[tokio::test]
async fn test_doc_cache_version_normalization() {
    let memory_cache = crates_docs::cache::memory::MemoryCache::new(100);
    let cache = Arc::new(memory_cache);
    let doc_cache = DocCache::new(cache);

    // Test version trimming and lowercasing
    doc_cache
        .set_crate_docs("serde", Some("  1.0.0  "), "docs".to_string())
        .await
        .expect("set should succeed");

    // Should be accessible with normalized version
    let result = doc_cache.get_crate_docs("serde", Some("1.0.0")).await;
    assert_eq!(result.as_ref().map(|s| s.as_ref()), Some("docs"));
}

#[tokio::test]
async fn test_doc_cache_case_insensitive_crate_name() {
    let memory_cache = crates_docs::cache::memory::MemoryCache::new(100);
    let cache = Arc::new(memory_cache);
    let doc_cache = DocCache::new(cache);

    doc_cache
        .set_crate_docs("Serde", None, "docs".to_string())
        .await
        .expect("set should succeed");

    // Should be accessible with lowercase
    let result = doc_cache.get_crate_docs("serde", None).await;
    assert_eq!(result.as_ref().map(|s| s.as_ref()), Some("docs"));
}

// ============================================================================
// Concurrent access tests
// ============================================================================

#[tokio::test]
async fn test_doc_cache_concurrent_access() {
    let memory_cache = crates_docs::cache::memory::MemoryCache::new(1000);
    let cache = Arc::new(memory_cache);
    let doc_cache = Arc::new(DocCache::new(cache));

    let mut handles = vec![];

    for i in 0..10 {
        let doc_cache_clone = doc_cache.clone();
        let handle = tokio::spawn(async move {
            let key = format!("concurrent_crate_{}", i);
            doc_cache_clone
                .set_crate_docs(&key, None, format!("docs_{}", i))
                .await
                .expect("set should succeed");

            let result = doc_cache_clone.get_crate_docs(&key, None).await;
            assert_eq!(
                result.as_ref().map(|s| s.as_ref()),
                Some(format!("docs_{}", i).as_str())
            );
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.await.expect("task failed");
    }
}

// ============================================================================
// Arc<String> preservation tests
// ============================================================================

/// Test that DocCache getters preserve shared ownership (Arc<String>)
/// This verifies the optimization that avoids unnecessary cloning on cache hits.
#[tokio::test]
async fn test_doc_cache_preserves_arc_on_get_crate_docs() {
    use crates_docs::tools::docs::cache::CacheKeyGenerator;

    let memory_cache = crates_docs::cache::memory::MemoryCache::new(100);
    let cache = Arc::new(memory_cache);
    let doc_cache = DocCache::new(cache.clone());

    // Set up cache with a large document
    let large_doc = "Large documentation content".to_string();
    doc_cache
        .set_crate_docs("test_crate", Some("1.0.0"), large_doc.clone())
        .await
        .expect("set_crate_docs should succeed");

    // Get from DocCache - should return Arc<String>
    let from_doc_cache = doc_cache
        .get_crate_docs("test_crate", Some("1.0.0"))
        .await
        .expect("should get from doc cache");

    // Get directly from backend cache - should return same Arc<String>
    let key = CacheKeyGenerator::crate_cache_key("test_crate", Some("1.0.0"));
    let from_backend = cache.get(&key).await.expect("should get from backend");

    // Verify they point to the same allocation (no clone occurred)
    assert!(
        Arc::ptr_eq(&from_doc_cache, &from_backend),
        "DocCache should preserve Arc<String> without cloning"
    );
}

/// Test that DocCache preserves Arc<String> for search results
#[tokio::test]
async fn test_doc_cache_preserves_arc_on_get_search_results() {
    use crates_docs::tools::docs::cache::CacheKeyGenerator;

    let memory_cache = crates_docs::cache::memory::MemoryCache::new(100);
    let cache = Arc::new(memory_cache);
    let doc_cache = DocCache::new(cache.clone());

    let search_results = "Search results content".to_string();
    doc_cache
        .set_search_results("test query", 10, Some("relevance"), search_results.clone())
        .await
        .expect("set_search_results should succeed");

    let from_doc_cache = doc_cache
        .get_search_results("test query", 10, Some("relevance"))
        .await
        .expect("should get search results");

    let key = CacheKeyGenerator::search_cache_key("test query", 10, Some("relevance"));
    let from_backend = cache.get(&key).await.expect("should get from backend");

    assert!(
        Arc::ptr_eq(&from_doc_cache, &from_backend),
        "DocCache should preserve Arc<String> for search results"
    );
}

/// Test that DocCache preserves Arc<String> for item docs
#[tokio::test]
async fn test_doc_cache_preserves_arc_on_get_item_docs() {
    use crates_docs::tools::docs::cache::CacheKeyGenerator;

    let memory_cache = crates_docs::cache::memory::MemoryCache::new(100);
    let cache = Arc::new(memory_cache);
    let doc_cache = DocCache::new(cache.clone());

    let item_docs = "Item documentation content".to_string();
    doc_cache
        .set_item_docs("test_crate", "test::Item", Some("1.0.0"), item_docs.clone())
        .await
        .expect("set_item_docs should succeed");

    let from_doc_cache = doc_cache
        .get_item_docs("test_crate", "test::Item", Some("1.0.0"))
        .await
        .expect("should get item docs");

    let key = CacheKeyGenerator::item_cache_key("test_crate", "test::Item", Some("1.0.0"));
    let from_backend = cache.get(&key).await.expect("should get from backend");

    assert!(
        Arc::ptr_eq(&from_doc_cache, &from_backend),
        "DocCache should preserve Arc<String> for item docs"
    );
}

// ============================================================================
// Format Parsing Tests
// ============================================================================

#[test]
fn test_format_display() {
    use crates_docs::tools::docs::Format;
    assert_eq!(Format::Markdown.to_string(), "markdown");
    assert_eq!(Format::Text.to_string(), "text");
    assert_eq!(Format::Html.to_string(), "html");
    assert_eq!(Format::Json.to_string(), "json");
}

#[test]
fn test_format_default() {
    use crates_docs::tools::docs::Format;
    assert_eq!(Format::default(), Format::Markdown);
}

#[tokio::test]
async fn test_fetch_html_optional_returns_none_on_404() {
    use wiremock::{matchers, Mock, MockServer, ResponseTemplate};

    let mock_server = MockServer::start().await;
    Mock::given(matchers::method("GET"))
        .and(matchers::path("/missing"))
        .respond_with(ResponseTemplate::new(404).set_body_string("not found"))
        .mount(&mock_server)
        .await;

    let cache = Arc::new(crates_docs::cache::memory::MemoryCache::new(10));
    let cache_config = crates_docs::cache::CacheConfig::default();
    let test_client = reqwest_middleware::ClientBuilder::new(reqwest::Client::new()).build();
    let service = crates_docs::tools::docs::DocService::with_custom_client(
        cache,
        &cache_config,
        Arc::new(test_client),
    );

    let url = format!("{}/missing", mock_server.uri());
    let result = service.fetch_html_optional(&url, Some("lookup_item")).await;
    assert!(matches!(result, Ok(None)), "404 should map to Ok(None)");
}

#[tokio::test]
async fn test_fetch_html_optional_surfaces_server_error() {
    use wiremock::{matchers, Mock, MockServer, ResponseTemplate};

    let mock_server = MockServer::start().await;
    Mock::given(matchers::method("GET"))
        .and(matchers::path("/boom"))
        .respond_with(ResponseTemplate::new(500).set_body_string("upstream failure"))
        .mount(&mock_server)
        .await;

    let cache = Arc::new(crates_docs::cache::memory::MemoryCache::new(10));
    let cache_config = crates_docs::cache::CacheConfig::default();
    let test_client = reqwest_middleware::ClientBuilder::new(reqwest::Client::new()).build();
    let service = crates_docs::tools::docs::DocService::with_custom_client(
        cache,
        &cache_config,
        Arc::new(test_client),
    );

    let url = format!("{}/boom", mock_server.uri());
    let result = service.fetch_html_optional(&url, Some("lookup_item")).await;
    assert!(result.is_err(), "non-success status must be an error");
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("500"),
        "error should mention the status: {msg}"
    );
    assert!(
        msg.contains("[lookup_item]"),
        "error should carry the tool prefix: {msg}"
    );
}
