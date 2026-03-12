//! Test command implementation

use rust_mcp_sdk::schema::ContentBlock;
use std::sync::Arc;

/// Test tool command
#[allow(clippy::too_many_arguments)]
pub async fn run_test_command(
    tool: &str,
    crate_name: Option<&str>,
    item_path: Option<&str>,
    query: Option<&str>,
    version: Option<&str>,
    limit: u32,
    format: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    tracing::info!("Testing tool: {}", tool);

    // Create cache
    let cache_config = crates_docs::cache::CacheConfig {
        cache_type: "memory".to_string(),
        memory_size: Some(1000),
        default_ttl: Some(3600),
        redis_url: None,
    };

    let cache = crates_docs::cache::create_cache(&cache_config)?;
    let cache_arc: Arc<dyn crates_docs::cache::Cache> = Arc::from(cache);

    // Create document service
    let doc_service = Arc::new(crates_docs::tools::docs::DocService::new(cache_arc));

    // Create tool registry
    let registry = crates_docs::tools::create_default_registry(&doc_service);

    match tool {
        "lookup_crate" => {
            execute_lookup_crate(crate_name, version, format, &registry).await?;
        }
        "search_crates" => {
            execute_search_crates(query, limit, format, &registry).await?;
        }
        "lookup_item" => {
            execute_lookup_item(crate_name, item_path, version, format, &registry).await?;
        }
        "health_check" => {
            execute_health_check(&registry).await?;
        }
        _ => {
            return Err(format!("Unknown tool: {}", tool).into());
        }
    }

    println!("Tool test completed");
    Ok(())
}

/// Execute lookup_crate tool
async fn execute_lookup_crate(
    crate_name: Option<&str>,
    version: Option<&str>,
    format: &str,
    registry: &crates_docs::tools::ToolRegistry,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(name) = crate_name {
        println!("Testing crate lookup: {} (version: {:?})", name, version);
        println!("Output format: {}", format);

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
            Err(e) => eprintln!("Tool execution failed: {}", e),
        }
    } else {
        return Err("lookup_crate requires --crate-name parameter".into());
    }
    Ok(())
}

/// Execute search_crates tool
async fn execute_search_crates(
    query: Option<&str>,
    limit: u32,
    format: &str,
    registry: &crates_docs::tools::ToolRegistry,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(q) = query {
        println!("Testing crate search: {} (limit: {})", q, limit);
        println!("Output format: {}", format);

        // Prepare arguments
        let arguments = serde_json::json!({
            "query": q,
            "limit": limit,
            "format": format
        });

        // Execute tool
        match registry.execute_tool("search_crates", arguments).await {
            Ok(result) => print_tool_result(&result),
            Err(e) => eprintln!("Tool execution failed: {}", e),
        }
    } else {
        return Err("search_crates requires --query parameter".into());
    }
    Ok(())
}

/// Execute lookup_item tool
async fn execute_lookup_item(
    crate_name: Option<&str>,
    item_path: Option<&str>,
    version: Option<&str>,
    format: &str,
    registry: &crates_docs::tools::ToolRegistry,
) -> Result<(), Box<dyn std::error::Error>> {
    if let (Some(name), Some(path)) = (crate_name, item_path) {
        println!(
            "Testing item lookup: {}::{} (version: {:?})",
            name, path, version
        );
        println!("Output format: {}", format);

        // Prepare arguments
        let mut arguments = serde_json::json!({
            "crate_name": name,
            "itemPath": path,
            "format": format
        });

        if let Some(v) = version {
            arguments["version"] = serde_json::Value::String(v.to_string());
        }

        // Execute tool
        match registry.execute_tool("lookup_item", arguments).await {
            Ok(result) => print_tool_result(&result),
            Err(e) => eprintln!("Tool execution failed: {}", e),
        }
    } else {
        return Err("lookup_item requires --crate-name and --item-path parameters".into());
    }
    Ok(())
}

/// Execute health_check tool
async fn execute_health_check(
    registry: &crates_docs::tools::ToolRegistry,
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
        Err(e) => eprintln!("Tool execution failed: {}", e),
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
                println!("Non-text content: {:?}", other);
            }
        }
    }
}
