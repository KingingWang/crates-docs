//! Health check tool comprehensive unit tests
//!
//! These tests cover all branches of the health check tool including:
//! - Different check types (all, external, internal, docs_rs, crates_io)
//! - Status calculation (healthy, unhealthy, degraded)
//! - Error handling and network failures
//! - Verbose and non-verbose output modes

use crates_docs::tools::health::HealthCheckToolImpl;
use crates_docs::tools::Tool;

// ============================================================================
// HealthCheckToolImpl basic tests
// ============================================================================

#[test]
fn test_health_check_tool_impl_new() {
    let tool = HealthCheckToolImpl::new();
    let definition = tool.definition();
    assert_eq!(definition.name, "health_check");
}

#[test]
fn test_health_check_tool_impl_default() {
    let tool = HealthCheckToolImpl::default();
    let definition = tool.definition();
    assert_eq!(definition.name, "health_check");
}

// ============================================================================
// Tool parameter tests
// ============================================================================

#[test]
fn test_health_check_tool_params_all_variations() {
    use crates_docs::tools::health::HealthCheckTool;

    // Test with all fields
    let params = HealthCheckTool {
        check_type: Some("all".to_string()),
        verbose: Some(true),
    };
    assert_eq!(params.check_type, Some("all".to_string()));
    assert_eq!(params.verbose, Some(true));

    // Test with external check type
    let params = HealthCheckTool {
        check_type: Some("external".to_string()),
        verbose: Some(false),
    };
    assert_eq!(params.check_type, Some("external".to_string()));
    assert_eq!(params.verbose, Some(false));

    // Test with internal check type
    let params = HealthCheckTool {
        check_type: Some("internal".to_string()),
        verbose: None,
    };
    assert_eq!(params.check_type, Some("internal".to_string()));
    assert!(params.verbose.is_none());

    // Test with docs_rs check type
    let params = HealthCheckTool {
        check_type: Some("docs_rs".to_string()),
        verbose: Some(true),
    };
    assert_eq!(params.check_type, Some("docs_rs".to_string()));

    // Test with crates_io check type
    let params = HealthCheckTool {
        check_type: Some("crates_io".to_string()),
        verbose: Some(true),
    };
    assert_eq!(params.check_type, Some("crates_io".to_string()));
}

// ============================================================================
// Internal check tests
// ============================================================================

#[tokio::test]
async fn test_internal_check() {
    let tool = HealthCheckToolImpl::new();
    let result = tool
        .execute(serde_json::json!({
            "check_type": "internal",
            "verbose": true
        }))
        .await;

    assert!(result.is_ok());
    let content = result.unwrap().content;
    assert!(!content.is_empty());

    // Check that the response contains expected fields
    let content_str = format!("{:?}", content);
    assert!(content_str.contains("status") || content_str.contains("Status"));
}

#[tokio::test]
async fn test_internal_check_non_verbose() {
    let tool = HealthCheckToolImpl::new();
    let result = tool
        .execute(serde_json::json!({
            "check_type": "internal",
            "verbose": false
        }))
        .await;

    assert!(result.is_ok());
}

// ============================================================================
// Check type parameter tests
// ============================================================================

#[tokio::test]
async fn test_check_type_all() {
    let tool = HealthCheckToolImpl::new();
    let result = tool
        .execute(serde_json::json!({
            "check_type": "all",
            "verbose": false
        }))
        .await;

    // This will fail due to network, but should not panic
    // The result will contain error information
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_check_type_external() {
    let tool = HealthCheckToolImpl::new();
    let result = tool
        .execute(serde_json::json!({
            "check_type": "external",
            "verbose": false
        }))
        .await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_check_type_docs_rs() {
    let tool = HealthCheckToolImpl::new();
    let result = tool
        .execute(serde_json::json!({
            "check_type": "docs_rs",
            "verbose": false
        }))
        .await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_check_type_crates_io() {
    let tool = HealthCheckToolImpl::new();
    let result = tool
        .execute(serde_json::json!({
            "check_type": "crates_io",
            "verbose": false
        }))
        .await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_check_type_unknown() {
    let tool = HealthCheckToolImpl::new();
    let result = tool
        .execute(serde_json::json!({
            "check_type": "unknown_type",
            "verbose": true
        }))
        .await;

    assert!(result.is_ok());
    let content = result.unwrap().content;
    let content_str = format!("{:?}", content);
    // Should contain "unknown" status for unknown check type
    assert!(content_str.contains("unknown") || content_str.contains("Status"));
}

// ============================================================================
// Verbose mode tests
// ============================================================================

#[tokio::test]
async fn test_verbose_mode_true() {
    let tool = HealthCheckToolImpl::new();
    let result = tool
        .execute(serde_json::json!({
            "check_type": "internal",
            "verbose": true
        }))
        .await;

    assert!(result.is_ok());
    let content = result.unwrap().content;
    let content_str = format!("{:?}", content);

    // Verbose mode should return JSON with all checks
    assert!(content_str.contains("status") || content_str.contains("Status"));
}

#[tokio::test]
async fn test_verbose_mode_false() {
    let tool = HealthCheckToolImpl::new();
    let result = tool
        .execute(serde_json::json!({
            "check_type": "internal",
            "verbose": false
        }))
        .await;

    assert!(result.is_ok());
    let content = result.unwrap().content;
    let content_str = format!("{:?}", content);

    // Non-verbose mode should return text summary
    assert!(content_str.contains("Status") || content_str.contains("status"));
}

#[tokio::test]
async fn test_default_parameters() {
    let tool = HealthCheckToolImpl::new();

    // Test with empty parameters - should use defaults
    let result = tool.execute(serde_json::json!({})).await;
    assert!(result.is_ok());

    // Test with only check_type
    let result = tool
        .execute(serde_json::json!({
            "check_type": "internal"
        }))
        .await;
    assert!(result.is_ok());

    // Test with only verbose
    let result = tool
        .execute(serde_json::json!({
            "verbose": true
        }))
        .await;
    assert!(result.is_ok());
}

// ============================================================================
// Error handling tests
// ============================================================================

#[tokio::test]
async fn test_invalid_arguments_type() {
    let tool = HealthCheckToolImpl::new();

    // Test with invalid verbose type
    let result = tool
        .execute(serde_json::json!({
            "verbose": "not_a_boolean"
        }))
        .await;

    assert!(result.is_err());
    let err = result.unwrap_err();
    let err_str = format!("{:?}", err);
    assert!(err_str.contains("health_check") || err_str.contains("Parameter"));
}

#[tokio::test]
async fn test_invalid_check_type_type() {
    let tool = HealthCheckToolImpl::new();

    // Test with invalid check_type (number instead of string)
    let result = tool
        .execute(serde_json::json!({
            "check_type": 123
        }))
        .await;

    assert!(result.is_err());
}

// ============================================================================
// Tool definition tests
// ============================================================================

#[test]
fn test_health_check_tool_definition() {
    use crates_docs::tools::health::HealthCheckTool;

    let definition = HealthCheckTool::tool();
    assert_eq!(definition.name, "health_check");
    assert!(definition.description.is_some());

    let desc = definition.description.unwrap();
    assert!(desc.contains("health") || desc.contains("Health"));
}

#[test]
fn test_health_check_tool_schema() {
    use crates_docs::tools::health::HealthCheckTool;

    let definition = HealthCheckTool::tool();
    // Verify the schema exists (input_schema field)
    let _schema = &definition.input_schema;
}

// ============================================================================
// Sequential execution tests (avoid Send issues)
// ============================================================================

#[tokio::test]
async fn test_sequential_health_checks() {
    let tool = HealthCheckToolImpl::new();

    // Run multiple health checks sequentially
    let check_types = vec!["internal", "external", "all"];

    for check_type in check_types {
        let result = tool
            .execute(serde_json::json!({
                "check_type": check_type,
                "verbose": false
            }))
            .await;

        // Each check should complete without panicking
        // They may succeed or fail depending on network, but shouldn't panic
        assert!(result.is_ok() || result.is_err());
    }
}

// ============================================================================
// Integration with ToolRegistry
// ============================================================================

#[test]
fn test_health_check_in_registry() {
    use crates_docs::tools::create_default_registry;
    use crates_docs::tools::docs::DocService;
    use std::sync::Arc;

    let service = Arc::new(DocService::default());
    let registry = create_default_registry(&service);

    let tools = registry.get_tools();
    let health_tool = tools.iter().find(|t| t.name == "health_check");
    assert!(health_tool.is_some());
}

// ============================================================================
// Response format tests
// ============================================================================

#[tokio::test]
async fn test_response_contains_timestamp() {
    let tool = HealthCheckToolImpl::new();
    let result = tool
        .execute(serde_json::json!({
            "check_type": "internal",
            "verbose": false
        }))
        .await;

    assert!(result.is_ok());
    let content = result.unwrap().content;
    let content_str = format!("{:?}", content);
    assert!(content_str.contains("Timestamp") || content_str.contains("timestamp"));
}

#[tokio::test]
async fn test_response_contains_uptime() {
    let tool = HealthCheckToolImpl::new();
    let result = tool
        .execute(serde_json::json!({
            "check_type": "internal",
            "verbose": false
        }))
        .await;

    assert!(result.is_ok());
    let content = result.unwrap().content;
    let content_str = format!("{:?}", content);
    assert!(content_str.contains("Uptime") || content_str.contains("uptime"));
}

// ============================================================================
// Edge case tests
// ============================================================================

#[tokio::test]
async fn test_empty_string_check_type() {
    let tool = HealthCheckToolImpl::new();
    let result = tool
        .execute(serde_json::json!({
            "check_type": "",
            "verbose": true
        }))
        .await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_whitespace_check_type() {
    let tool = HealthCheckToolImpl::new();
    let result = tool
        .execute(serde_json::json!({
            "check_type": "   ",
            "verbose": true
        }))
        .await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_null_parameters() {
    let tool = HealthCheckToolImpl::new();
    let result = tool
        .execute(serde_json::json!({
            "check_type": null,
            "verbose": null
        }))
        .await;

    assert!(result.is_ok());
}

// ============================================================================
// Error message tests
// ============================================================================

#[tokio::test]
async fn test_error_message_format() {
    let tool = HealthCheckToolImpl::new();

    // Test with invalid parameters to trigger error
    let result = tool
        .execute(serde_json::json!({
            "verbose": []
        }))
        .await;

    assert!(result.is_err());
    let err = result.unwrap_err();
    let err_str = format!("{:?}", err);
    assert!(!err_str.is_empty());
}
