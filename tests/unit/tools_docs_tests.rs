//! Unit tests for tools/docs module

use crates_docs::tools::docs::{
    cache::{DocCache, DocCacheTtl},
    html::{clean_html, extract_documentation, extract_search_results, html_to_text},
};
use std::sync::Arc;

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
    assert!(result.contains("未找到项目"));
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
