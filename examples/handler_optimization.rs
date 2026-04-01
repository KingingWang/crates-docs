//! Handler optimization example
//!
//! Demonstrates how to use the simplified Handler, including:
//! - Configuration using HandlerConfig
//! - Merging configurations using merge functionality
//! - Metrics integration
//!
//! # Running the example
//!
//! ```bash
//! cargo run --example handler_optimization
//! ```

use std::sync::Arc;

use crates_docs::config::AppConfig;
use crates_docs::metrics::ServerMetrics;
use crates_docs::server::{
    handler::{CratesDocsHandler, HandlerConfig},
    CratesDocsServer,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Handler Optimization Example\n");

    // 1. Create server instance
    let config = AppConfig::default();
    let server = Arc::new(CratesDocsServer::new(config)?);

    println!("✅ Server created successfully\n");

    // 2. Example 1: Create Handler with default configuration
    println!("📝 Example 1: Handler with default configuration");
    let handler_default = CratesDocsHandler::new(server.clone());
    println!(
        "   - Verbose logging: {}",
        handler_default.config().verbose_logging
    );
    println!("   - Metrics: {}", handler_default.config().enable_metrics);
    println!();

    // 3. Example 2: Create Handler with custom configuration
    println!("📝 Example 2: Handler with custom configuration");
    let custom_config = HandlerConfig::new().with_verbose_logging();
    let handler_custom = CratesDocsHandler::with_config(server.clone(), custom_config);
    println!(
        "   - Verbose logging: {}",
        handler_custom.config().verbose_logging
    );
    println!();

    // 4. Example 3: Merge configurations using merge functionality
    println!("📝 Example 3: Merging configurations with merge");
    let base_config = HandlerConfig::default();
    let override_config = HandlerConfig::new().with_verbose_logging().with_metrics();
    let handler_merged =
        CratesDocsHandler::with_merged_config(server.clone(), base_config, Some(override_config));
    println!("   - Base config: verbose_logging=false, enable_metrics=false");
    println!("   - Override config: verbose_logging=true, enable_metrics=true");
    println!(
        "   - Merged result: verbose_logging={}, enable_metrics={}",
        handler_merged.config().verbose_logging,
        handler_merged.config().enable_metrics
    );
    println!();

    // 5. Example 4: Configuration method chaining
    println!("📝 Example 4: Configuration method chaining");
    let config_chained = HandlerConfig::new().with_verbose_logging().with_metrics();
    println!("   - Verbose logging: {}", config_chained.verbose_logging);
    println!("   - Metrics: {}", config_chained.enable_metrics);
    println!();

    // 6. Example 5: Metrics integration
    println!("📝 Example 5: Metrics integration");
    let metrics = Arc::new(ServerMetrics::new());
    let handler_with_metrics = CratesDocsHandler::new(server.clone()).with_metrics(metrics.clone());

    // Simulate tool execution (structure demonstration only)
    println!("   - Handler has Metrics instance attached");
    let list_tools = handler_with_metrics.list_tools();
    println!("   - Available tools count: {}", list_tools.tools.len());

    // Check if metrics are recorded
    let metrics_output = metrics.export()?;
    println!("   - Metrics generated: {}", !metrics_output.is_empty());
    println!();

    // 7. Example 6: Direct tool listing
    println!("📝 Example 6: Direct tool listing");
    let tools = handler_with_metrics.list_tools();
    println!("   - Tool list:");
    for tool in &tools.tools {
        if let Some(desc) = &tool.description {
            println!("     * {} - {}", tool.name, desc);
        }
    }
    println!();

    // 8. Example 7: Test HandlerConfig merge override logic
    println!("📝 Example 7: Test merge override logic");

    // Test 1: None override (returns original config)
    let config1 = HandlerConfig::default();
    let merged1 = config1.merge(None);
    println!(
        "   - merge(None): verbose_logging={}, enable_metrics={}",
        merged1.verbose_logging, merged1.enable_metrics
    );

    // Test 2: Partial override
    let config_base = HandlerConfig::default();
    let config2 = HandlerConfig::new().with_verbose_logging();
    let merged2 = config_base.merge(Some(config2));
    println!(
        "   - merge(partial override): verbose_logging={}, enable_metrics={}",
        merged2.verbose_logging, merged2.enable_metrics
    );

    // Test 3: Full override
    let config_base2 = HandlerConfig::default();
    let config3 = HandlerConfig::new().with_verbose_logging().with_metrics();
    let merged3 = config_base2.merge(Some(config3));
    println!(
        "   - merge(full override): verbose_logging={}, enable_metrics={}",
        merged3.verbose_logging, merged3.enable_metrics
    );
    println!();

    println!("🎉 All examples completed!\n");

    Ok(())
}
