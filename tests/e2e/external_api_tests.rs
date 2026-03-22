//! 外部 API 集成测试
//!
//! 使用 wiremock 模拟外部服务（crates.io 和 docs.rs）。
//! 注意：由于 DocService 的 API URL 是硬编码的，这些测试主要用于验证工具的行为。

use crates_docs::{
    cache::{create_cache, CacheConfig},
    tools::docs::DocService,
};
use std::sync::Arc;

/// 测试 DocService 创建
#[tokio::test]
async fn test_doc_service_creation() {
    let cache_config = CacheConfig::default();
    let cache = create_cache(&cache_config).expect("Failed to create cache");
    let cache_arc: Arc<dyn crates_docs::cache::Cache> = Arc::from(cache);

    let doc_service = DocService::new(cache_arc);
    assert!(doc_service.is_ok(), "Failed to create DocService");
}

/// 测试 DocService 带配置创建
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

    // 验证缓存配置
    let cache = doc_service.cache();
    // 缓存应该可用
    cache
        .set("test_key".to_string(), "test_value".to_string(), None)
        .await
        .expect("Cache set failed");
    let value = cache.get("test_key").await;
    assert_eq!(value, Some("test_value".to_string()));
}

/// 测试缓存功能
#[tokio::test]
async fn test_doc_service_cache_operations() {
    let cache_config = CacheConfig::default();
    let cache = create_cache(&cache_config).expect("Failed to create cache");
    let cache_arc: Arc<dyn crates_docs::cache::Cache> = Arc::from(cache);

    let doc_service = DocService::new(cache_arc).expect("Failed to create DocService");

    // 测试缓存基本操作
    let cache = doc_service.cache();

    // 设置缓存
    cache
        .set("crate:serde".to_string(), "serde docs".to_string(), None)
        .await
        .expect("Cache set failed");

    // 获取缓存
    let value = cache.get("crate:serde").await;
    assert_eq!(value, Some("serde docs".to_string()));

    // 删除缓存
    cache
        .delete("crate:serde")
        .await
        .expect("Cache delete failed");
    let value = cache.get("crate:serde").await;
    assert_eq!(value, None);
}

/// 测试缓存 TTL
#[tokio::test]
async fn test_doc_service_cache_ttl() {
    let cache_config = CacheConfig::default();
    let cache = create_cache(&cache_config).expect("Failed to create cache");
    let cache_arc: Arc<dyn crates_docs::cache::Cache> = Arc::from(cache);

    let doc_service = DocService::new(cache_arc).expect("Failed to create DocService");
    let cache = doc_service.cache();

    // 设置带 TTL 的缓存
    cache
        .set(
            "expiring_key".to_string(),
            "expiring_value".to_string(),
            Some(std::time::Duration::from_secs(1)),
        )
        .await
        .expect("Cache set failed");

    // 立即获取应该成功
    let value = cache.get("expiring_key").await;
    assert_eq!(value, Some("expiring_value".to_string()));

    // 等待过期
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    // 过期后应该返回 None
    let value = cache.get("expiring_key").await;
    assert_eq!(value, None);
}

/// 测试 HTTP 客户端配置
#[tokio::test]
async fn test_doc_service_http_client() {
    let cache_config = CacheConfig::default();
    let cache = create_cache(&cache_config).expect("Failed to create cache");
    let cache_arc: Arc<dyn crates_docs::cache::Cache> = Arc::from(cache);

    let doc_service = DocService::new(cache_arc).expect("Failed to create DocService");

    // 验证 HTTP 客户端已创建
    let _client = doc_service.client();
    // 客户端应该可用
}

/// 测试工具注册表与 DocService 集成
#[tokio::test]
async fn test_tool_registry_with_doc_service() {
    let cache_config = CacheConfig::default();
    let cache = create_cache(&cache_config).expect("Failed to create cache");
    let cache_arc: Arc<dyn crates_docs::cache::Cache> = Arc::from(cache);

    let doc_service = Arc::new(DocService::new(cache_arc).expect("Failed to create DocService"));

    // 创建工具注册表
    let registry = crates_docs::tools::create_default_registry(&doc_service);

    // 验证工具已注册
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

/// 测试 DocService 默认实现
#[tokio::test]
async fn test_doc_service_default() {
    let doc_service = DocService::default();

    // 验证默认服务可用
    let _client = doc_service.client();
    let _cache = doc_service.cache();
    let _doc_cache = doc_service.doc_cache();
}

/// 测试缓存键格式
#[tokio::test]
async fn test_cache_key_formats() {
    let cache_config = CacheConfig::default();
    let cache = create_cache(&cache_config).expect("Failed to create cache");
    let cache_arc: Arc<dyn crates_docs::cache::Cache> = Arc::from(cache);

    let doc_service = DocService::new(cache_arc).expect("Failed to create DocService");
    let cache = doc_service.cache();

    // 测试不同类型的缓存键
    let keys = vec![
        "crate:serde",
        "crate:serde:1.0.0",
        "item:serde:Serialize",
        "search:web framework:relevance:10",
    ];

    for key in keys {
        cache
            .set(key.to_string(), format!("value_for_{}", key), None)
            .await
            .expect("Cache set failed");

        let value = cache.get(key).await;
        assert_eq!(
            value,
            Some(format!("value_for_{}", key)),
            "Cache key {} failed",
            key
        );
    }
}

/// 测试缓存清空
#[tokio::test]
async fn test_cache_clear() {
    let cache_config = CacheConfig::default();
    let cache = create_cache(&cache_config).expect("Failed to create cache");
    let cache_arc: Arc<dyn crates_docs::cache::Cache> = Arc::from(cache);

    let doc_service = DocService::new(cache_arc).expect("Failed to create DocService");
    let cache = doc_service.cache();

    // 添加多个缓存项
    for i in 0..10 {
        cache
            .set(format!("key_{}", i), format!("value_{}", i), None)
            .await
            .expect("Cache set failed");
    }

    // 验证缓存存在
    for i in 0..10 {
        let value = cache.get(&format!("key_{}", i)).await;
        assert!(value.is_some(), "Cache key_{} should exist", i);
    }

    // 清空缓存
    cache.clear().await.expect("Cache clear failed");

    // 验证缓存已清空
    for i in 0..10 {
        let value = cache.get(&format!("key_{}", i)).await;
        assert!(value.is_none(), "Cache key_{} should be cleared", i);
    }
}

/// 测试并发缓存访问
#[tokio::test]
async fn test_concurrent_cache_access() {
    let cache_config = CacheConfig::default();
    let cache = create_cache(&cache_config).expect("Failed to create cache");
    let cache_arc: Arc<dyn crates_docs::cache::Cache> = Arc::from(cache);

    let doc_service = Arc::new(DocService::new(cache_arc).expect("Failed to create DocService"));

    // 创建多个并发任务
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
            assert_eq!(
                retrieved,
                Some(value),
                "Concurrent access failed for key {}",
                i
            );
        });
        handles.push(handle);
    }

    // 等待所有任务完成
    for handle in handles {
        handle.await.expect("Task failed");
    }
}

/// 测试 DocService with_full_config
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

    // 验证服务可用
    let _client = doc_service.client();
    let _cache = doc_service.cache();
    let _doc_cache = doc_service.doc_cache();
}

/// 测试 DocCache TTL 配置
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

    // 验证 TTL 配置
    assert_eq!(ttl.crate_docs_secs, 7200);
    assert_eq!(ttl.item_docs_secs, 1800);
    assert_eq!(ttl.search_results_secs, 300);
}

/// 测试缓存统计（如果支持）
#[tokio::test]
async fn test_cache_statistics() {
    let cache_config = CacheConfig::default();
    let cache = create_cache(&cache_config).expect("Failed to create cache");
    let cache_arc: Arc<dyn crates_docs::cache::Cache> = Arc::from(cache);

    let doc_service = DocService::new(cache_arc).expect("Failed to create DocService");
    let cache = doc_service.cache();

    // 执行一些缓存操作
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
    cache.get("nonexistent").await; // 缓存未命中

    // 缓存应该正常工作
    assert!(cache.get("stats_key1").await.is_some());
    assert!(cache.get("stats_key2").await.is_some());
    assert!(cache.get("nonexistent").await.is_none());
}
