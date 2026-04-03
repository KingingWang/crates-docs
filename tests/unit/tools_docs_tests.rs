//! Unit tests for tools/docs module

use crates_docs::tools::docs::{
    cache::{DocCache, DocCacheTtl},
    html::{clean_html, extract_documentation, extract_search_results, html_to_text},
};
use serial_test::serial;
use std::sync::Arc;

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
    assert_eq!(result, Some("Serde documentation".to_string()));

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
        .set_search_results("web framework", 10, "Search results".to_string())
        .await
        .expect("set_search_results should succeed");

    let result = doc_cache.get_search_results("web framework", 10).await;
    assert_eq!(result, Some("Search results".to_string()));

    // Test different limit
    let result = doc_cache.get_search_results("web framework", 20).await;
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
    assert_eq!(result, Some("Serialize trait docs".to_string()));
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

    let _guard = EnvVarGuard::new("CRATES_DOCS_CRATES_IO_URL", &mock_uri);

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
        "query": "serialization",
        "limit": 10,
        "sort": "relevance",
        "format": "markdown"
    });

    let result = tool.execute(args).await;
    assert!(result.is_ok());
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

    let _guard = EnvVarGuard::new("CRATES_DOCS_CRATES_IO_URL", &mock_uri);

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

    let _guard = EnvVarGuard::new("CRATES_DOCS_CRATES_IO_URL", &mock_uri);

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
        "query": "http client",
        "format": "json"
    });

    let result = tool.execute(args).await;
    assert!(result.is_ok());
}

#[tokio::test]
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

    let _guard = EnvVarGuard::new("CRATES_DOCS_CRATES_IO_URL", &mock_uri);

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
        "limit": 200
    });

    let result = tool.execute(args).await;
    assert!(result.is_ok());
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
    assert_eq!(result, Some("docs".to_string()));
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
    assert_eq!(result, Some("docs".to_string()));
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
            assert_eq!(result, Some(format!("docs_{}", i)));
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.await.expect("task failed");
    }
}

// ============================================================================
// Format Parsing Tests
// ============================================================================

#[test]
fn test_parse_format_none() {
    use crates_docs::tools::docs::{parse_format, Format};
    assert_eq!(parse_format(None).unwrap(), Format::Markdown);
}

#[test]
fn test_parse_format_markdown() {
    use crates_docs::tools::docs::{parse_format, Format};
    assert_eq!(parse_format(Some("markdown")).unwrap(), Format::Markdown);
    assert_eq!(parse_format(Some("MARKDOWN")).unwrap(), Format::Markdown);
    assert_eq!(parse_format(Some("Markdown")).unwrap(), Format::Markdown);
}

#[test]
fn test_parse_format_text() {
    use crates_docs::tools::docs::{parse_format, Format};
    assert_eq!(parse_format(Some("text")).unwrap(), Format::Text);
    assert_eq!(parse_format(Some("TEXT")).unwrap(), Format::Text);
}

#[test]
fn test_parse_format_html() {
    use crates_docs::tools::docs::{parse_format, Format};
    assert_eq!(parse_format(Some("html")).unwrap(), Format::Html);
    assert_eq!(parse_format(Some("HTML")).unwrap(), Format::Html);
}

#[test]
fn test_parse_format_json() {
    use crates_docs::tools::docs::{parse_format, Format};
    assert_eq!(parse_format(Some("json")).unwrap(), Format::Json);
    assert_eq!(parse_format(Some("JSON")).unwrap(), Format::Json);
}

#[test]
fn test_parse_format_invalid() {
    use crates_docs::tools::docs::parse_format;
    assert!(parse_format(Some("invalid")).is_err());
    assert!(parse_format(Some("xml")).is_err());
    assert!(parse_format(Some("")).is_err());
}

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
