//! 工具模块单元测试

use crates_docs::tools::docs::lookup_crate::LookupCrateToolImpl;
use crates_docs::tools::docs::lookup_item::LookupItemToolImpl;
use crates_docs::tools::docs::search::SearchCratesToolImpl;
use crates_docs::tools::docs::DocService;
use crates_docs::tools::health::HealthCheckToolImpl;
use crates_docs::tools::Tool;
use crates_docs::tools::{create_default_registry, ToolRegistry};
use std::sync::Arc;

// ============================================================================
// 工具参数测试
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

#[test]
fn test_lookup_item_tool_params() {
    use crates_docs::tools::docs::lookup_item::LookupItemTool;

    let params = LookupItemTool {
        crate_name: "serde".to_string(),
        item_path: "serde::Serialize".to_string(),
        version: None,
        format: Some("text".to_string()),
    };

    assert_eq!(params.crate_name, "serde");
    assert_eq!(params.item_path, "serde::Serialize");
    assert!(params.version.is_none());
    assert_eq!(params.format, Some("text".to_string()));
}

#[test]
fn test_search_crates_tool_params() {
    use crates_docs::tools::docs::search::SearchCratesTool;

    let params = SearchCratesTool {
        query: "web framework".to_string(),
        limit: Some(20),
        format: Some("json".to_string()),
    };

    assert_eq!(params.query, "web framework");
    assert_eq!(params.limit, Some(20));
    assert_eq!(params.format, Some("json".to_string()));
}

#[test]
fn test_health_check_tool_params() {
    use crates_docs::tools::health::HealthCheckTool;

    let params = HealthCheckTool {
        check_type: Some("external".to_string()),
        verbose: Some(true),
    };

    assert_eq!(params.check_type, Some("external".to_string()));
    assert_eq!(params.verbose, Some(true));
}

// ============================================================================
// ToolRegistry 测试
// ============================================================================

#[test]
fn test_tool_registry_default() {
    let registry = ToolRegistry::default();
    assert!(registry.get_tools().is_empty());
}

#[test]
fn test_tool_registry_default_and_unknown_tool() {
    let service = Arc::new(DocService::default());
    let registry = create_default_registry(&service);
    let tools = registry.get_tools();
    assert_eq!(tools.len(), 4);
    assert!(tools.iter().any(|t| t.name == "lookup_crate"));
    assert!(tools.iter().any(|t| t.name == "lookup_item"));
    assert!(tools.iter().any(|t| t.name == "search_crates"));
    assert!(tools.iter().any(|t| t.name == "health_check"));

    let rt = tokio::runtime::Runtime::new().unwrap();
    let err = rt
        .block_on(async {
            registry
                .execute_tool("does_not_exist", serde_json::Value::Null)
                .await
        })
        .unwrap_err();
    assert!(err.to_string().contains("does_not_exist"));
}

// ============================================================================
// 工具执行错误路径测试
// ============================================================================

#[test]
fn test_health_check_tool_invalid_arguments() {
    let tool = HealthCheckToolImpl::new();
    let rt = tokio::runtime::Runtime::new().unwrap();
    let err = rt
        .block_on(async { tool.execute(serde_json::json!({"verbose": "bad"})).await })
        .unwrap_err();
    assert!(err.to_string().contains("health_check"));
}

#[test]
fn test_lookup_and_search_tools_invalid_arguments() {
    let service = Arc::new(DocService::default());
    let crate_tool = LookupCrateToolImpl::new(service.clone());
    let item_tool = LookupItemToolImpl::new(service.clone());
    let search_tool = SearchCratesToolImpl::new(service);
    let rt = tokio::runtime::Runtime::new().unwrap();

    let err = rt
        .block_on(async { crate_tool.execute(serde_json::json!({"version": 1})).await })
        .unwrap_err();
    assert!(err.to_string().contains("lookup_crate"));

    let err = rt
        .block_on(async {
            item_tool
                .execute(serde_json::json!({"crate_name": "serde"}))
                .await
        })
        .unwrap_err();
    assert!(err.to_string().contains("lookup_item"));

    let err = rt
        .block_on(async { search_tool.execute(serde_json::json!({"limit": "x"})).await })
        .unwrap_err();
    assert!(err.to_string().contains("search_crates"));
}

// ============================================================================
// DocService 测试
// ============================================================================

#[test]
fn test_doc_service_accessors_and_default() {
    use crates_docs::cache::{create_cache, CacheConfig};

    let cache = create_cache(&CacheConfig::default()).unwrap();
    let cache: Arc<dyn crates_docs::cache::Cache> = Arc::from(cache);
    let service = DocService::new(cache.clone());

    let _client = service.client();
    assert!(Arc::ptr_eq(service.cache(), &cache));
    let _doc_cache = service.doc_cache();

    let default_service = DocService::default();
    let _ = default_service.client();
    let _ = default_service.cache();
}

// ============================================================================
// 工具定义测试
// ============================================================================

#[test]
fn test_lookup_crate_tool_definition() {
    use crates_docs::tools::docs::lookup_crate::LookupCrateTool;

    let definition = LookupCrateTool::tool();
    assert_eq!(definition.name, "lookup_crate");
    assert!(definition.description.is_some());
}

#[test]
fn test_lookup_item_tool_definition() {
    use crates_docs::tools::docs::lookup_item::LookupItemTool;

    let definition = LookupItemTool::tool();
    assert_eq!(definition.name, "lookup_item");
    assert!(definition.description.is_some());
}

#[test]
fn test_search_crates_tool_definition() {
    use crates_docs::tools::docs::search::SearchCratesTool;

    let definition = SearchCratesTool::tool();
    assert_eq!(definition.name, "search_crates");
    assert!(definition.description.is_some());
}

#[test]
fn test_health_check_tool_definition() {
    use crates_docs::tools::health::HealthCheckTool;

    let definition = HealthCheckTool::tool();
    assert_eq!(definition.name, "health_check");
    assert!(definition.description.is_some());
}

// ============================================================================
// 工具默认值测试
// ============================================================================

#[test]
fn test_lookup_crate_tool_default() {
    let tool = LookupCrateToolImpl::default();
    let definition = tool.definition();
    assert_eq!(definition.name, "lookup_crate");
}

#[test]
fn test_lookup_item_tool_default() {
    let tool = LookupItemToolImpl::default();
    let definition = tool.definition();
    assert_eq!(definition.name, "lookup_item");
}

#[test]
fn test_search_crates_tool_default() {
    let tool = SearchCratesToolImpl::default();
    let definition = tool.definition();
    assert_eq!(definition.name, "search_crates");
}

#[test]
fn test_health_check_tool_default() {
    let tool = HealthCheckToolImpl::default();
    let definition = tool.definition();
    assert_eq!(definition.name, "health_check");
}
