//! Test command implementation

use rust_mcp_sdk::schema::ContentBlock;
use std::path::Path;
use std::sync::Arc;

/// Test tool command
#[allow(clippy::too_many_arguments)]
pub async fn run_test_command(
    config_path: &Path,
    tool: &str,
    crate_name: Option<&str>,
    item_path: Option<&str>,
    query: Option<&str>,
    sort: Option<&str>,
    version: Option<&str>,
    limit: u32,
    format: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    tracing::info!("Testing tool: {}", tool);

    // Honor the global `--config` flag: load cache and performance settings
    // from the config file when present, falling back to defaults otherwise.
    let app_config = if config_path.exists() {
        crate::config::AppConfig::from_file(config_path)
            .map_err(|e| format!("Failed to load config file: {e}"))?
    } else {
        crate::config::AppConfig::default()
    };

    // Initialize the global HTTP client from the configured performance
    // settings (timeouts, user-agent, pool). Ignore the error if it was
    // already initialized elsewhere in the process.
    let _ = crate::utils::init_global_http_client(&app_config.performance);

    let cache = crate::cache::create_cache(&app_config.cache)?;
    let cache_arc: Arc<dyn crate::cache::Cache> = Arc::from(cache);

    // Create document service honoring the configured cache TTLs.
    let doc_service = Arc::new(crate::tools::docs::DocService::with_config(
        cache_arc,
        &app_config.cache,
    )?);

    // Create tool registry
    let registry = crate::tools::create_default_registry(&doc_service);

    match tool {
        "lookup_crate" => {
            execute_lookup_crate(crate_name, version, format, &registry).await?;
        }
        "search_crates" => {
            execute_search_crates(query, sort, limit, format, &registry).await?;
        }
        "lookup_item" => {
            execute_lookup_item(crate_name, item_path, version, format, &registry).await?;
        }
        "health_check" => {
            execute_health_check(&registry).await?;
        }
        _ => {
            return Err(format!("Unknown tool: {tool}").into());
        }
    }

    println!("Tool test completed");
    Ok(())
}

/// Execute `lookup_crate` tool
async fn execute_lookup_crate(
    crate_name: Option<&str>,
    version: Option<&str>,
    format: &str,
    registry: &crate::tools::ToolRegistry,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(name) = crate_name {
        println!("Testing crate lookup: {name} (version: {version:?})");
        println!("Output format: {format}");

        // Prepare arguments
        let mut arguments = serde_json::json!({
            "crate_name": name,
            "format": format
        });

        if let Some(v) = version {
            arguments["version"] = serde_json::Value::String(v.to_string());
        }

        // Execute tool
        match registry.execute_tool("lookup_crate", arguments).await {
            Ok(result) => print_tool_result(&result),
            Err(e) => return Err(format!("Tool execution failed: {e}").into()),
        }
    } else {
        return Err("lookup_crate requires --crate-name parameter".into());
    }
    Ok(())
}

/// Execute `search_crates` tool
async fn execute_search_crates(
    query: Option<&str>,
    sort: Option<&str>,
    limit: u32,
    format: &str,
    registry: &crate::tools::ToolRegistry,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(q) = query {
        println!("Testing crate search: {q} (limit: {limit})");
        println!("Sort order: {}", sort.unwrap_or("relevance"));
        println!("Output format: {format}");

        // Prepare arguments
        let mut arguments = serde_json::json!({
            "query": q,
            "limit": limit,
            "format": format
        });

        if let Some(sort) = sort {
            arguments["sort"] = serde_json::Value::String(sort.to_string());
        }

        // Execute tool
        match registry.execute_tool("search_crates", arguments).await {
            Ok(result) => print_tool_result(&result),
            Err(e) => return Err(format!("Tool execution failed: {e}").into()),
        }
    } else {
        return Err("search_crates requires --query parameter".into());
    }
    Ok(())
}

/// Execute `lookup_item` tool
async fn execute_lookup_item(
    crate_name: Option<&str>,
    item_path: Option<&str>,
    version: Option<&str>,
    format: &str,
    registry: &crate::tools::ToolRegistry,
) -> Result<(), Box<dyn std::error::Error>> {
    if let (Some(name), Some(path)) = (crate_name, item_path) {
        println!("Testing item lookup: {name}::{path} (version: {version:?})");
        println!("Output format: {format}");

        // Prepare arguments
        let mut arguments = serde_json::json!({
            "crate_name": name,
            "item_path": path,
            "format": format
        });

        if let Some(v) = version {
            arguments["version"] = serde_json::Value::String(v.to_string());
        }

        // Execute tool
        match registry.execute_tool("lookup_item", arguments).await {
            Ok(result) => print_tool_result(&result),
            Err(e) => return Err(format!("Tool execution failed: {e}").into()),
        }
    } else {
        return Err("lookup_item requires --crate-name and --item-path parameters".into());
    }
    Ok(())
}

/// Execute `health_check` tool
async fn execute_health_check(
    registry: &crate::tools::ToolRegistry,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("Testing health check");

    // Prepare arguments
    let arguments = serde_json::json!({
        "check_type": "all",
        "verbose": true
    });

    // Execute tool
    match registry.execute_tool("health_check", arguments).await {
        Ok(result) => print_tool_result(&result),
        Err(e) => return Err(format!("Tool execution failed: {e}").into()),
    }
    Ok(())
}

/// Print tool execution result
fn print_tool_result(result: &rust_mcp_sdk::schema::CallToolResult) {
    println!("Tool executed successfully:");
    if let Some(content) = result.content.first() {
        match content {
            ContentBlock::TextContent(text_content) => {
                println!("{}", text_content.text);
            }
            other => {
                println!("Non-text content: {other:?}");
            }
        }
    }
}
