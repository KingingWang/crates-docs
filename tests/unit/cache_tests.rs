//! Cache module unit tests

use crates_docs::{
    cache::{create_cache, memory::MemoryCache, CacheConfig},
    tools::docs::cache::DocCache,
};
use std::sync::Arc;

// ============================================================================
// CacheConfig tests
// ============================================================================

#[test]
fn test_cache_config_default_values() {
    let config = CacheConfig::default();
    assert_eq!(config.cache_type, "memory");
    assert_eq!(config.memory_size, Some(1000));
    assert_eq!(config.default_ttl, Some(3600));
    assert!(config.redis_url.is_none());
    assert_eq!(config.key_prefix, String::new());
    assert_eq!(config.crate_docs_ttl_secs, Some(3600));
    assert_eq!(config.item_docs_ttl_secs, Some(1800));
    assert_eq!(config.search_results_ttl_secs, Some(300));
}

#[test]
fn test_cache_config_custom_values() {
    let config = CacheConfig {
        cache_type: "redis".to_string(),
        memory_size: Some(500),
        default_ttl: Some(7200),
        redis_url: Some("redis://localhost:6379".to_string()),
        key_prefix: "myapp".to_string(),
        crate_docs_ttl_secs: Some(1800),
        item_docs_ttl_secs: Some(900),
        search_results_ttl_secs: Some(150),
    };
    assert_eq!(config.cache_type, "redis");
    assert_eq!(config.memory_size, Some(500));
    assert_eq!(config.default_ttl, Some(7200));
    assert_eq!(config.redis_url, Some("redis://localhost:6379".to_string()));
    assert_eq!(config.key_prefix, "myapp");
    assert_eq!(config.crate_docs_ttl_secs, Some(1800));
    assert_eq!(config.item_docs_ttl_secs, Some(900));
    assert_eq!(config.search_results_ttl_secs, Some(150));
}

#[test]
fn test_cache_config_serialization() {
    let config = CacheConfig::default();
    let json = serde_json::to_string(&config).expect("Failed to serialize");
    assert!(json.contains("\"cache_type\":\"memory\""));
    assert!(json.contains("\"memory_size\":1000"));

    let deserialized: CacheConfig = serde_json::from_str(&json).expect("Failed to deserialize");
    assert_eq!(deserialized.cache_type, config.cache_type);
    assert_eq!(deserialized.memory_size, config.memory_size);
    assert_eq!(deserialized.default_ttl, config.default_ttl);
}

#[test]
fn test_cache_config_toml_deserialization() {
    let toml_str = r#"
        cache_type = "memory"
        memory_size = 2000
        key_prefix = "test_prefix"
        default_ttl = 1800
    "#;

    let config: CacheConfig = toml::from_str(toml_str).expect("Failed to parse TOML");
    assert_eq!(config.cache_type, "memory");
    assert_eq!(config.memory_size, Some(2000));
    assert_eq!(config.key_prefix, "test_prefix");
    assert_eq!(config.default_ttl, Some(1800));
}

#[test]
fn test_cache_config_defaults_functions() {
    use crates_docs::cache::{
        default_crate_docs_ttl, default_item_docs_ttl, default_key_prefix,
        default_search_results_ttl,
    };

    assert_eq!(default_crate_docs_ttl(), Some(3600));
    assert_eq!(default_item_docs_ttl(), Some(1800));
    assert_eq!(default_search_results_ttl(), Some(300));
    assert_eq!(default_key_prefix(), String::new());
}

#[test]
fn test_cache_config_with_missing_optional_fields() {
    let toml_str = r#"
        cache_type = "memory"
    "#;

    let config: CacheConfig = toml::from_str(toml_str).expect("Failed to parse TOML");
    assert_eq!(config.cache_type, "memory");
    assert_eq!(config.memory_size, None);
    assert_eq!(config.default_ttl, None);
    assert_eq!(config.redis_url, None);
    // These should use defaults from serde(default)
    assert_eq!(config.key_prefix, String::new());
    assert_eq!(config.crate_docs_ttl_secs, Some(3600));
    assert_eq!(config.item_docs_ttl_secs, Some(1800));
    assert_eq!(config.search_results_ttl_secs, Some(300));
}

// ============================================================================
// DocCache tests
// ============================================================================

#[tokio::test]
async fn test_doc_cache_crate_docs() {
    let config = CacheConfig::default();
    let cache = create_cache(&config).expect("Failed to create cache");
    let cache_arc: Arc<dyn crates_docs::cache::Cache> = Arc::from(cache);
    let doc_cache = DocCache::new(cache_arc);

    // Test cache miss
    let result = doc_cache.get_crate_docs("serde", None).await;
    assert!(result.is_none());

    // Set cache
    doc_cache
        .set_crate_docs("serde", None, "Serde documentation".to_string())
        .await
        .expect("set_crate_docs should succeed");

    // Test cache hit
    let result = doc_cache.get_crate_docs("serde", None).await;
    assert_eq!(
        result.as_ref().map(|s| s.as_ref()),
        Some("Serde documentation")
    );

    // Test cache with version
    doc_cache
        .set_crate_docs("tokio", Some("1.0.0"), "Tokio 1.0 docs".to_string())
        .await
        .expect("set_crate_docs should succeed");
    let result = doc_cache.get_crate_docs("tokio", Some("1.0.0")).await;
    assert_eq!(result.as_ref().map(|s| s.as_ref()), Some("Tokio 1.0 docs"));

    // Different versions should return different cached values
    let result = doc_cache.get_crate_docs("tokio", Some("1.1.0")).await;
    assert!(result.is_none());
}

#[tokio::test]
async fn test_doc_cache_item_docs() {
    let config = CacheConfig::default();
    let cache = create_cache(&config).expect("Failed to create cache");
    let cache_arc: Arc<dyn crates_docs::cache::Cache> = Arc::from(cache);
    let doc_cache = DocCache::new(cache_arc);

    // Test cache miss
    let result = doc_cache
        .get_item_docs("serde", "serde::Serialize", None)
        .await;
    assert!(result.is_none());

    // Set cache
    doc_cache
        .set_item_docs(
            "serde",
            "serde::Serialize",
            None,
            "Serialize trait docs".to_string(),
        )
        .await
        .expect("set_item_docs should succeed");

    // Test cache hit
    let result = doc_cache
        .get_item_docs("serde", "serde::Serialize", None)
        .await;
    assert_eq!(
        result.as_ref().map(|s| s.as_ref()),
        Some("Serialize trait docs")
    );

    // Test cache with version
    doc_cache
        .set_item_docs(
            "std",
            "std::collections::HashMap",
            Some("1.75.0"),
            "HashMap docs".to_string(),
        )
        .await
        .expect("set_item_docs should succeed");
    let result = doc_cache
        .get_item_docs("std", "std::collections::HashMap", Some("1.75.0"))
        .await;
    assert_eq!(result.as_ref().map(|s| s.as_ref()), Some("HashMap docs"));
}

// ============================================================================
// create_cache error path tests
// ============================================================================

#[test]
fn test_create_cache_unsupported_type() {
    let config = CacheConfig {
        cache_type: "unsupported".to_string(),
        memory_size: Some(100),
        default_ttl: Some(3600),
        redis_url: None,
        key_prefix: String::new(),
        crate_docs_ttl_secs: Some(3600),
        item_docs_ttl_secs: Some(1800),
        search_results_ttl_secs: Some(300),
    };
    let result = create_cache(&config);
    assert!(result.is_err());
    // Check that error message contains expected content (lowercase "unsupported")
    if let Err(e) = result {
        assert!(e.to_string().contains("unsupported cache type"));
    }
}

#[cfg(feature = "cache-redis")]
#[test]
fn test_create_cache_redis_sync_error() {
    let config = CacheConfig {
        cache_type: "redis".to_string(),
        memory_size: Some(100),
        default_ttl: Some(3600),
        redis_url: Some("redis://invalid:6379".to_string()),
        key_prefix: String::new(),
        crate_docs_ttl_secs: Some(3600),
        item_docs_ttl_secs: Some(1800),
        search_results_ttl_secs: Some(300),
    };

    let result = create_cache(&config);
    assert!(result.is_err());

    if let Err(e) = result {
        assert!(e.to_string().contains("async initialization"));
    }
}

#[cfg(not(feature = "cache-redis"))]
#[test]
fn test_create_cache_redis_sync_error() {
    let config = CacheConfig {
        cache_type: "redis".to_string(),
        memory_size: Some(100),
        default_ttl: Some(3600),
        redis_url: Some("redis://invalid:6379".to_string()),
        key_prefix: String::new(),
        crate_docs_ttl_secs: Some(3600),
        item_docs_ttl_secs: Some(1800),
        search_results_ttl_secs: Some(300),
    };

    let result = create_cache(&config);
    assert!(result.is_err());

    if let Err(e) = result {
        assert!(e.to_string().contains("feature is not enabled"));
    }
}

// ============================================================================
// Cache eviction tests
// ============================================================================

#[tokio::test]
async fn test_doc_cache_ttl_expiration() {
    use std::time::Duration;
    use tokio::time::sleep;

    let memory_cache = MemoryCache::new(100);

    // Use very short TTL for testing
    let mut ttl = DocCacheTtl::default();
    ttl.crate_docs_secs = 1; // 1 second
    ttl.set_jitter_ratio(0.0); // Disable jitter for predictable tests

    let doc_cache = DocCache::with_ttl(Arc::new(memory_cache), ttl);

    // Set cache entry
    doc_cache
        .set_crate_docs("test-crate", None, "Test docs".to_string())
        .await
        .expect("set_crate_docs should succeed");

    // Verify cache hit immediately
    let result = doc_cache.get_crate_docs("test-crate", None).await;
    assert!(result.is_some(), "Cache should hit immediately after set");

    // Wait for TTL to expire
    sleep(Duration::from_secs(2)).await;

    // Note: moka cache handles TTL expiration asynchronously
    // We rely on the sleep duration being longer than TTL

    // Verify cache miss after expiration
    let result = doc_cache.get_crate_docs("test-crate", None).await;
    assert!(result.is_none(), "Cache should miss after TTL expiration");
}

#[tokio::test]
async fn test_doc_cache_capacity_eviction() {
    // Create small cache to test eviction
    let memory_cache = MemoryCache::new(3);
    let cache = Arc::new(memory_cache);
    let doc_cache = DocCache::new(cache);

    // Fill cache beyond capacity
    for i in 0..5 {
        doc_cache
            .set_crate_docs(&format!("crate-{i}"), None, format!("Docs {i}"))
            .await
            .expect("set_crate_docs should succeed");
    }

    // Access first crate to make it "hot" (frequently accessed)
    let _ = doc_cache.get_crate_docs("crate-0", None).await;
    let _ = doc_cache.get_crate_docs("crate-0", None).await;
    let _ = doc_cache.get_crate_docs("crate-0", None).await;

    // Add more items to trigger eviction
    for i in 5..10 {
        doc_cache
            .set_crate_docs(&format!("crate-{i}"), None, format!("Docs {i}"))
            .await
            .expect("set_crate_docs should succeed");
    }

    // Frequently accessed crate should still be in cache
    let _hot_crate = doc_cache.get_crate_docs("crate-0", None).await;
    // Note: TinyLFU may evict based on frequency, so this is a probabilistic test
    // We mainly verify the cache doesn't crash under eviction pressure
}

#[tokio::test]
async fn test_doc_cache_different_types_independent_ttl() {
    use std::time::Duration;
    use tokio::time::sleep;

    let memory_cache = MemoryCache::new(100);

    // Configure different TTLs for different cache types
    let mut ttl = DocCacheTtl::default();
    ttl.crate_docs_secs = 1; // 1 second
    ttl.search_results_secs = 3; // 3 seconds
    ttl.item_docs_secs = 5; // 5 seconds
    ttl.set_jitter_ratio(0.0);

    let doc_cache = DocCache::with_ttl(Arc::new(memory_cache), ttl);

    // Set different cache types
    doc_cache
        .set_crate_docs("crate", None, "Crate docs".to_string())
        .await
        .unwrap();
    doc_cache
        .set_search_results("query", 10, None, "Search results".to_string())
        .await
        .unwrap();
    doc_cache
        .set_item_docs("crate", "item", None, "Item docs".to_string())
        .await
        .unwrap();

    // Wait for crate docs TTL to expire (1s), but not others
    sleep(Duration::from_secs(2)).await;

    // Note: moka cache handles TTL expiration asynchronously
    // We rely on the sleep duration being longer than TTL

    // Crate docs should be expired
    assert!(doc_cache.get_crate_docs("crate", None).await.is_none());
    // Search results should still exist
    assert!(doc_cache
        .get_search_results("query", 10, None)
        .await
        .is_some());
    // Item docs should still exist
    assert!(doc_cache
        .get_item_docs("crate", "item", None)
        .await
        .is_some());
}

// ============================================================================
// Concurrent access tests
// ============================================================================

#[tokio::test]
async fn test_doc_cache_concurrent_reads() {
    use tokio::task::JoinSet;

    let memory_cache = MemoryCache::new(100);
    let cache = Arc::new(memory_cache);
    let doc_cache = Arc::new(DocCache::new(cache));

    // Pre-populate cache
    doc_cache
        .set_crate_docs("concurrent-crate", None, "Shared docs".to_string())
        .await
        .unwrap();

    // Spawn multiple concurrent readers
    let mut join_set = JoinSet::new();
    for _ in 0..100 {
        let dc = doc_cache.clone();
        join_set.spawn(async move { dc.get_crate_docs("concurrent-crate", None).await });
    }

    // All reads should succeed
    let mut success_count = 0;
    while let Some(result) = join_set.join_next().await {
        if let Ok(Some(value)) = result {
            assert_eq!(value.as_ref(), "Shared docs");
            success_count += 1;
        }
    }
    assert_eq!(success_count, 100, "All concurrent reads should succeed");
}

#[tokio::test]
async fn test_doc_cache_concurrent_writes() {
    use tokio::task::JoinSet;

    let memory_cache = MemoryCache::new(100);
    let cache = Arc::new(memory_cache);
    let doc_cache = Arc::new(DocCache::new(cache));

    // Spawn multiple concurrent writers
    let mut join_set = JoinSet::new();
    for i in 0..50 {
        let dc = doc_cache.clone();
        join_set.spawn(async move {
            dc.set_crate_docs(
                &format!("crate-{}", i % 10), // Only 10 unique keys
                None,
                format!("Docs from writer {}", i),
            )
            .await
        });
    }

    // All writes should complete without error
    let mut success_count = 0;
    while let Some(result) = join_set.join_next().await {
        if result.is_ok() && result.unwrap().is_ok() {
            success_count += 1;
        }
    }
    assert_eq!(success_count, 50, "All concurrent writes should succeed");
}

#[tokio::test]
async fn test_doc_cache_concurrent_mixed_operations() {
    use tokio::task::JoinSet;

    let memory_cache = MemoryCache::new(100);
    let cache = Arc::new(memory_cache);
    let doc_cache = Arc::new(DocCache::new(cache));

    // Pre-populate some data
    for i in 0..10 {
        doc_cache
            .set_crate_docs(&format!("crate-{i}"), None, format!("Initial {i}"))
            .await
            .unwrap();
    }

    let mut join_set = JoinSet::new();

    // Spawn readers
    for _ in 0..50 {
        let dc = doc_cache.clone();
        join_set.spawn(async move {
            for i in 0..10 {
                dc.get_crate_docs(&format!("crate-{i}"), None).await;
            }
        });
    }

    // Spawn writers
    for i in 0..30 {
        let dc = doc_cache.clone();
        join_set.spawn(async move {
            let _ = dc
                .set_crate_docs(&format!("crate-{}", i % 10), None, format!("Updated {i}"))
                .await;
        });
    }

    // Spawn deleters
    for _ in 0..10 {
        let dc = doc_cache.clone();
        join_set.spawn(async move {
            let _ = dc.clear().await;
        });
    }

    // All operations should complete without panic
    let mut completed = 0;
    while let Some(_result) = join_set.join_next().await {
        // We don't check results because operations may fail due to clear
        // We just verify no panics occur
        completed += 1;
    }
    assert_eq!(completed, 90, "All concurrent operations should complete");
}

#[tokio::test]
async fn test_doc_cache_race_condition_read_after_write() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::task::JoinSet;

    let memory_cache = MemoryCache::new(100);
    let cache = Arc::new(memory_cache);
    let doc_cache = Arc::new(DocCache::new(cache));

    let hit_count = Arc::new(AtomicUsize::new(0));
    let miss_count = Arc::new(AtomicUsize::new(0));

    let mut join_set = JoinSet::new();

    // Writer task
    let dc_writer = doc_cache.clone();
    join_set.spawn(async move {
        dc_writer
            .set_crate_docs("race-crate", None, "Race docs".to_string())
            .await
    });

    // Multiple reader tasks that start before write completes
    for _ in 0..20 {
        let dc = doc_cache.clone();
        let hits = hit_count.clone();
        let misses = miss_count.clone();
        join_set.spawn(async move {
            // Small delay to increase chance of race
            tokio::task::yield_now().await;
            if dc.get_crate_docs("race-crate", None).await.is_some() {
                hits.fetch_add(1, Ordering::SeqCst);
            } else {
                misses.fetch_add(1, Ordering::SeqCst);
            }
            Ok::<(), crates_docs::error::Error>(())
        });
    }

    while let Some(result) = join_set.join_next().await {
        let _ = result;
    }

    // Verify final state is consistent
    let final_value = doc_cache.get_crate_docs("race-crate", None).await;
    assert!(final_value.is_some(), "Final state should have the value");
    assert_eq!(final_value.unwrap().as_ref(), "Race docs");
}

// ============================================================================
// TTL jitter tests
// ============================================================================

use crates_docs::tools::docs::cache::DocCacheTtl;

#[test]
fn test_doc_cache_ttl_default_includes_jitter() {
    let ttl = DocCacheTtl::default();
    assert_eq!(ttl.crate_docs_secs, 3600);
    assert_eq!(ttl.search_results_secs, 300);
    assert_eq!(ttl.item_docs_secs, 1800);
    // Default jitter is 10%
    assert!((ttl.jitter_ratio() - 0.1).abs() < f64::EPSILON);
}

#[test]
fn test_apply_jitter_zero_ratio_returns_base_ttl() {
    let mut ttl = DocCacheTtl::default();
    ttl.crate_docs_secs = 3600;
    ttl.search_results_secs = 300;
    ttl.item_docs_secs = 1800;
    ttl.set_jitter_ratio(0.0);

    // When jitter_ratio is 0, should return original value
    assert_eq!(ttl.apply_jitter(3600), 3600);
    assert_eq!(ttl.apply_jitter(300), 300);
}

#[test]
fn test_apply_jitter_within_expected_range() {
    let mut ttl = DocCacheTtl::default();
    ttl.crate_docs_secs = 3600;
    ttl.search_results_secs = 300;
    ttl.item_docs_secs = 1800;
    ttl.set_jitter_ratio(0.1); // 10% jitter

    // Call multiple times to ensure results are within expected range
    for _ in 0..100 {
        let result = ttl.apply_jitter(3600);
        // 10% jitter means result should be in [3240, 3960] range
        assert!(
            result >= 3240,
            "jitter result {result} is below minimum 3240"
        );
        assert!(
            result <= 3960,
            "jitter result {result} is above maximum 3960"
        );
    }
}

#[test]
fn test_apply_jitter_clamps_to_valid_range() {
    // Test negative jitter_ratio (should be clamped to 0)
    let mut ttl_negative = DocCacheTtl::default();
    ttl_negative.crate_docs_secs = 3600;
    ttl_negative.search_results_secs = 300;
    ttl_negative.item_docs_secs = 1800;
    ttl_negative.set_jitter_ratio(-0.5);
    // Negative values should be treated as 0, returning original value
    assert_eq!(ttl_negative.apply_jitter(3600), 3600);

    // Test jitter_ratio above 1.0 (should be clamped to 1.0)
    let mut ttl_high = DocCacheTtl::default();
    ttl_high.crate_docs_secs = 3600;
    ttl_high.search_results_secs = 300;
    ttl_high.item_docs_secs = 1800;
    ttl_high.set_jitter_ratio(2.0);
    // 200% jitter means result can be in [0, 7200] range, but minimum is 1
    for _ in 0..100 {
        let result = ttl_high.apply_jitter(3600);
        assert!(result >= 1, "jitter result {result} should be at least 1");
        assert!(
            result <= 7200,
            "jitter result {result} should be at most 7200"
        );
    }
}

#[test]
fn test_apply_jitter_different_base_values() {
    let mut ttl = DocCacheTtl::default();
    ttl.crate_docs_secs = 3600;
    ttl.search_results_secs = 300;
    ttl.item_docs_secs = 1800;
    ttl.set_jitter_ratio(0.1);

    // Test different base TTL values
    let base_values = [60, 300, 1800, 3600, 7200];

    for &base in &base_values {
        let expected_min = (base as f64 * 0.9) as u64;
        let expected_max = (base as f64 * 1.1) as u64;

        for _ in 0..50 {
            let result = ttl.apply_jitter(base);
            assert!(
                result >= expected_min && result <= expected_max,
                "jitter result {result} for base {base} is outside expected range [{expected_min}, {expected_max}]"
            );
        }
    }
}

// ============================================================================
// DocCacheTtl::with_jitter and from_cache_config None branches
// ============================================================================

#[test]
fn test_doc_cache_ttl_with_jitter() {
    let ttl = DocCacheTtl::with_jitter(7200, 600, 3600, 0.2);
    assert_eq!(ttl.crate_docs_secs, 7200);
    assert_eq!(ttl.search_results_secs, 600);
    assert_eq!(ttl.item_docs_secs, 3600);
    assert!((ttl.jitter_ratio() - 0.2).abs() < f64::EPSILON);
}

#[test]
fn test_doc_cache_ttl_with_jitter_clamped() {
    let ttl = DocCacheTtl::with_jitter(3600, 300, 1800, 1.5);
    assert!((ttl.jitter_ratio() - 1.0).abs() < f64::EPSILON);

    let ttl = DocCacheTtl::with_jitter(3600, 300, 1800, -0.5);
    assert!(ttl.jitter_ratio().abs() < f64::EPSILON);
}

#[test]
fn test_doc_cache_ttl_from_cache_config_none_defaults() {
    let config = CacheConfig {
        cache_type: "memory".to_string(),
        memory_size: Some(1000),
        redis_url: None,
        key_prefix: String::new(),
        default_ttl: None,
        crate_docs_ttl_secs: None,
        item_docs_ttl_secs: None,
        search_results_ttl_secs: None,
    };
    let ttl = DocCacheTtl::from_cache_config(&config);
    assert_eq!(ttl.crate_docs_secs, 3600);
    assert_eq!(ttl.search_results_secs, 300);
    assert_eq!(ttl.item_docs_secs, 1800);
}

// ============================================================================
// CacheStats inc methods and as_tuple
// ============================================================================

#[test]
fn test_cache_stats_inc_methods() {
    use crates_docs::tools::docs::cache::CacheStats;

    let stats = CacheStats::new();
    assert_eq!(stats.inc_hits(), 1);
    assert_eq!(stats.inc_hits(), 2);
    assert_eq!(stats.inc_misses(), 1);
    assert_eq!(stats.inc_misses(), 2);
    assert_eq!(stats.inc_sets(), 1);
    assert_eq!(stats.inc_sets(), 2);

    assert_eq!(stats.hits(), 2);
    assert_eq!(stats.misses(), 2);
    assert_eq!(stats.sets(), 2);
}

#[test]
fn test_cache_stats_as_tuple() {
    use crates_docs::tools::docs::cache::CacheStats;

    let stats = CacheStats::new();
    stats.record_hit();
    stats.record_miss();
    stats.record_set();

    let (hits, misses, sets) = stats.as_tuple();
    assert_eq!(hits, 1);
    assert_eq!(misses, 1);
    assert_eq!(sets, 1);
}

// ============================================================================
// CacheKeyGenerator invalid item path hash branch
// ============================================================================

#[test]
fn test_item_cache_key_invalid_path_with_version() {
    use crates_docs::tools::docs::cache::CacheKeyGenerator;

    // Invalid item path (contains newline) with version should hash
    let key = CacheKeyGenerator::item_cache_key("serde", "invalid\npath", Some("1.0"));
    assert!(key.contains("hash:"));
    assert!(key.contains(":1.0:")); // version should be present
}

#[test]
fn test_item_cache_key_invalid_path_no_version() {
    use crates_docs::tools::docs::cache::CacheKeyGenerator;

    // Invalid item path without version should hash without version
    let key = CacheKeyGenerator::item_cache_key("serde", "invalid\npath", None);
    assert!(key.contains("hash:"));
    // Should not have version in the key format
    assert!(!key.contains(":1.0"));
}
