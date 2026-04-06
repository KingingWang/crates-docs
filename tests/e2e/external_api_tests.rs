//! External API integration tests
//!
//! Uses wiremock to mock external services (crates.io and docs.rs).
//! Note: Since DocService's API URLs are hardcoded, these tests mainly verify tool behavior.

use crates_docs::{
    cache::{create_cache, CacheConfig},
    tools::docs::DocService,
};
use std::sync::Arc;

/// Test DocService creation
#[tokio::test]
async fn test_doc_service_creation() {
    let cache_config = CacheConfig::default();
    let cache = create_cache(&cache_config).expect("Failed to create cache");
    let cache_arc: Arc<dyn crates_docs::cache::Cache> = Arc::from(cache);

    let doc_service = DocService::new(cache_arc);
    assert!(doc_service.is_ok(), "Failed to create DocService");
}

/// Test DocService creation with configuration
#[tokio::test]
async fn test_doc_service_with_config() {
    let cache_config = CacheConfig {
        memory_size: Some(500),
        default_ttl: Some(1800),
        ..Default::default()
    };
    let cache = create_cache(&cache_config).expect("Failed to create cache");
    let cache_arc: Arc<dyn crates_docs::cache::Cache> = Arc::from(cache);

    let doc_service = DocService::with_config(cache_arc.clone(), &cache_config);
    assert!(
        doc_service.is_ok(),
        "Failed to create DocService with config"
    );

    let doc_service = doc_service.unwrap();

    // Verify cache configuration
    let cache = doc_service.cache();
    // Cache should be available
    cache
        .set("test_key".to_string(), "test_value".to_string(), None)
        .await
        .expect("Cache set failed");
    let value = cache.get("test_key").await;
    assert!(value.is_some());
    assert_eq!(value.unwrap().as_ref(), "test_value");
}

/// Test cache functionality
#[tokio::test]
async fn test_doc_service_cache_operations() {
    let cache_config = CacheConfig::default();
    let cache = create_cache(&cache_config).expect("Failed to create cache");
    let cache_arc: Arc<dyn crates_docs::cache::Cache> = Arc::from(cache);

    let doc_service = DocService::new(cache_arc).expect("Failed to create DocService");

    // Test basic cache operations
    let cache = doc_service.cache();

    // Set cache
    cache
        .set("crate:serde".to_string(), "serde docs".to_string(), None)
        .await
        .expect("Cache set failed");

    // Get cache
    let value = cache.get("crate:serde").await;
    assert!(value.is_some());
    assert_eq!(value.unwrap().as_ref(), "serde docs");

    // Delete cache
    cache
        .delete("crate:serde")
        .await
        .expect("Cache delete failed");
    let value = cache.get("crate:serde").await;
    assert_eq!(value, None);
}

/// Test cache TTL
#[tokio::test]
async fn test_doc_service_cache_ttl() {
    let cache_config = CacheConfig::default();
    let cache = create_cache(&cache_config).expect("Failed to create cache");
    let cache_arc: Arc<dyn crates_docs::cache::Cache> = Arc::from(cache);

    let doc_service = DocService::new(cache_arc).expect("Failed to create DocService");
    let cache = doc_service.cache();

    // Set cache with TTL
    cache
        .set(
            "expiring_key".to_string(),
            "expiring_value".to_string(),
            Some(std::time::Duration::from_secs(1)),
        )
        .await
        .expect("Cache set failed");

    // Immediate retrieval should succeed
    let value = cache.get("expiring_key").await;
    assert!(value.is_some());
    assert_eq!(value.unwrap().as_ref(), "expiring_value");

    // Wait for expiration
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    // Should return None after expiration
    let value = cache.get("expiring_key").await;
    assert_eq!(value, None);
}

/// Test HTTP client configuration
#[tokio::test]
async fn test_doc_service_http_client() {
    let cache_config = CacheConfig::default();
    let cache = create_cache(&cache_config).expect("Failed to create cache");
    let cache_arc: Arc<dyn crates_docs::cache::Cache> = Arc::from(cache);

    let doc_service = DocService::new(cache_arc).expect("Failed to create DocService");

    // Verify HTTP client is created
    let _client = doc_service.client();
    // Client should be available
}

/// Test tool registry with DocService integration
#[tokio::test]
async fn test_tool_registry_with_doc_service() {
    let cache_config = CacheConfig::default();
    let cache = create_cache(&cache_config).expect("Failed to create cache");
    let cache_arc: Arc<dyn crates_docs::cache::Cache> = Arc::from(cache);

    let doc_service = Arc::new(DocService::new(cache_arc).expect("Failed to create DocService"));

    // Create tool registry
    let registry = crates_docs::tools::create_default_registry(&doc_service);

    // Verify tools are registered
    let tools = registry.get_tools();
    assert_eq!(tools.len(), 4, "Should have 4 tools registered");

    let tool_names: std::collections::HashSet<String> =
        tools.iter().map(|t| t.name.clone()).collect();

    assert!(
        tool_names.contains("lookup_crate"),
        "Should have lookup_crate tool"
    );
    assert!(
        tool_names.contains("lookup_item"),
        "Should have lookup_item tool"
    );
    assert!(
        tool_names.contains("search_crates"),
        "Should have search_crates tool"
    );
    assert!(
        tool_names.contains("health_check"),
        "Should have health_check tool"
    );
}

/// Test DocService default implementation
#[tokio::test]
async fn test_doc_service_default() {
    let doc_service = DocService::default();

    // Verify default service is available
    let _client = doc_service.client();
    let _cache = doc_service.cache();
    let _doc_cache = doc_service.doc_cache();
}

/// Test cache key formats
#[tokio::test]
async fn test_cache_key_formats() {
    let cache_config = CacheConfig::default();
    let cache = create_cache(&cache_config).expect("Failed to create cache");
    let cache_arc: Arc<dyn crates_docs::cache::Cache> = Arc::from(cache);

    let doc_service = DocService::new(cache_arc).expect("Failed to create DocService");
    let cache = doc_service.cache();

    // Test different types of cache keys
    let keys = vec![
        "crate:serde",
        "crate:serde:1.0.0",
        "item:serde:Serialize",
        "search:web framework:relevance:10",
    ];

    for key in keys {
        let expected_value = format!("value_for_{}", key);
        cache
            .set(key.to_string(), expected_value.clone(), None)
            .await
            .expect("Cache set failed");

        let value = cache.get(key).await;
        assert!(value.is_some(), "Cache key {} failed", key);
        assert_eq!(
            value.unwrap().as_ref(),
            expected_value.as_str(),
            "Cache key {} value mismatch",
            key
        );
    }
}

/// Test cache clear
#[tokio::test]
async fn test_cache_clear() {
    let cache_config = CacheConfig::default();
    let cache = create_cache(&cache_config).expect("Failed to create cache");
    let cache_arc: Arc<dyn crates_docs::cache::Cache> = Arc::from(cache);

    let doc_service = DocService::new(cache_arc).expect("Failed to create DocService");
    let cache = doc_service.cache();

    // Add multiple cache entries
    for i in 0..10 {
        cache
            .set(format!("key_{}", i), format!("value_{}", i), None)
            .await
            .expect("Cache set failed");
    }

    // Verify cache exists
    for i in 0..10 {
        let value = cache.get(&format!("key_{}", i)).await;
        assert!(value.is_some(), "Cache key_{} should exist", i);
    }

    // Clear cache
    cache.clear().await.expect("Cache clear failed");

    // Verify cache is cleared
    for i in 0..10 {
        let value = cache.get(&format!("key_{}", i)).await;
        assert!(value.is_none(), "Cache key_{} should be cleared", i);
    }
}

/// Test concurrent cache access
#[tokio::test]
async fn test_concurrent_cache_access() {
    let cache_config = CacheConfig::default();
    let cache = create_cache(&cache_config).expect("Failed to create cache");
    let cache_arc: Arc<dyn crates_docs::cache::Cache> = Arc::from(cache);

    let doc_service = Arc::new(DocService::new(cache_arc).expect("Failed to create DocService"));

    // Create multiple concurrent tasks
    let mut handles = vec![];

    for i in 0..10 {
        let service = doc_service.clone();
        let handle = tokio::spawn(async move {
            let cache = service.cache();
            let key = format!("concurrent_key_{}", i);
            let value = format!("concurrent_value_{}", i);

            cache
                .set(key.clone(), value.clone(), None)
                .await
                .expect("Set failed");
            let retrieved = cache.get(&key).await;
            assert!(
                retrieved.is_some(),
                "Concurrent access failed for key {}",
                i
            );
            assert_eq!(
                retrieved.unwrap().as_ref(),
                value.as_str(),
                "Concurrent access value mismatch for key {}",
                i
            );
        });
        handles.push(handle);
    }

    // Wait for all tasks to complete
    for handle in handles {
        handle.await.expect("Task failed");
    }
}

/// Test DocService with_full_config
#[tokio::test]
async fn test_doc_service_with_full_config() {
    use crates_docs::config::PerformanceConfig;

    let cache_config = CacheConfig {
        memory_size: Some(1000),
        default_ttl: Some(3600),
        ..Default::default()
    };

    let perf_config = PerformanceConfig {
        http_client_pool_size: 20,
        http_client_timeout_secs: 60,
        ..Default::default()
    };

    let cache = create_cache(&cache_config).expect("Failed to create cache");
    let cache_arc: Arc<dyn crates_docs::cache::Cache> = Arc::from(cache);

    let doc_service = DocService::with_full_config(cache_arc, &cache_config, &perf_config);
    assert!(
        doc_service.is_ok(),
        "Failed to create DocService with full config"
    );

    let doc_service = doc_service.unwrap();

    // Verify service is available
    let _client = doc_service.client();
    let _cache = doc_service.cache();
    let _doc_cache = doc_service.doc_cache();
}

/// Test DocCache TTL configuration
#[tokio::test]
async fn test_doc_cache_ttl_configuration() {
    use crates_docs::tools::docs::DocCacheTtl;

    let cache_config = CacheConfig {
        crate_docs_ttl_secs: Some(7200),    // 2 hours
        item_docs_ttl_secs: Some(1800),     // 30 minutes
        search_results_ttl_secs: Some(300), // 5 minutes
        ..Default::default()
    };

    let ttl = DocCacheTtl::from_cache_config(&cache_config);

    // Verify TTL configuration
    assert_eq!(ttl.crate_docs_secs, 7200);
    assert_eq!(ttl.item_docs_secs, 1800);
    assert_eq!(ttl.search_results_secs, 300);
}

/// Test cache statistics (if supported)
#[tokio::test]
async fn test_cache_statistics() {
    let cache_config = CacheConfig::default();
    let cache = create_cache(&cache_config).expect("Failed to create cache");
    let cache_arc: Arc<dyn crates_docs::cache::Cache> = Arc::from(cache);

    let doc_service = DocService::new(cache_arc).expect("Failed to create DocService");
    let cache = doc_service.cache();

    // Perform some cache operations
    cache
        .set("stats_key1".to_string(), "value1".to_string(), None)
        .await
        .expect("Set failed");
    cache
        .set("stats_key2".to_string(), "value2".to_string(), None)
        .await
        .expect("Set failed");
    cache.get("stats_key1").await;
    cache.get("stats_key2").await;
    cache.get("nonexistent").await; // Cache miss

    // Cache should work normally
    assert!(cache.get("stats_key1").await.is_some());
    assert!(cache.get("stats_key2").await.is_some());
    assert!(cache.get("nonexistent").await.is_none());
}
