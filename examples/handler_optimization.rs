//! Handler 优化示例
//!
//! 展示如何使用重构后的 Handler，包括：
//! - 使用 HandlerConfig 进行配置
//! - 使用 merge 功能合并配置
//! - 集成 metrics
//!
//! # 运行示例
//!
//! ```bash
//! cargo run --example handler_optimization
//! ```

use std::sync::Arc;

use crates_docs::config::AppConfig;
use crates_docs::metrics::ServerMetrics;
use crates_docs::server::{
    handler::{CratesDocsHandler, CratesDocsHandlerCore, HandlerConfig, HandlerCore},
    CratesDocsServer,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Handler 优化示例\n");

    // 1. 创建服务器实例
    let config = AppConfig::default();
    let server = Arc::new(CratesDocsServer::new(config)?);

    println!("✅ 服务器创建成功\n");

    // 2. 示例 1: 使用默认配置创建 Handler
    println!("📝 示例 1: 默认配置的 Handler");
    let handler_default = CratesDocsHandler::new(server.clone());
    println!(
        "   - 详细日志: {}",
        handler_default.core().config().verbose_logging
    );
    println!(
        "   - Metrics: {}",
        handler_default.core().config().enable_metrics
    );
    println!();

    // 3. 示例 2: 使用自定义配置创建 Handler
    println!("📝 示例 2: 自定义配置的 Handler");
    let custom_config = HandlerConfig::new().with_verbose_logging();
    let handler_custom = CratesDocsHandler::with_config(server.clone(), custom_config);
    println!(
        "   - 详细日志: {}",
        handler_custom.core().config().verbose_logging
    );
    println!();

    // 4. 示例 3: 使用 merge 功能合并配置
    println!("📝 示例 3: 使用 merge 合并配置");
    let base_config = HandlerConfig::default();
    let override_config = HandlerConfig::new().with_verbose_logging().with_metrics();
    let handler_merged =
        CratesDocsHandler::with_merged_config(server.clone(), base_config, Some(override_config));
    println!("   - 基础配置: verbose_logging=false, enable_metrics=false");
    println!("   - 覆盖配置: verbose_logging=true, enable_metrics=true");
    println!(
        "   - 合并结果: verbose_logging={}, enable_metrics={}",
        handler_merged.core().config().verbose_logging,
        handler_merged.core().config().enable_metrics
    );
    println!();

    // 5. 示例 4: 配置链式调用
    println!("📝 示例 4: 配置链式调用");
    let config_chained = HandlerConfig::new().with_verbose_logging().with_metrics();
    println!("   - 详细日志: {}", config_chained.verbose_logging);
    println!("   - Metrics: {}", config_chained.enable_metrics);
    println!();

    // 6. 示例 5: 集成 Metrics
    println!("📝 示例 5: 集成 Metrics");
    let metrics = Arc::new(ServerMetrics::new());
    let handler_with_metrics = CratesDocsHandler::new(server.clone()).with_metrics(metrics.clone());

    // 模拟工具执行（仅展示结构）
    println!("   - Handler 已关联 Metrics 实例");
    let list_tools = handler_with_metrics.core().list_tools();
    println!("   - 可用工具数量: {}", list_tools.tools.len());

    // 检查 metrics 是否记录
    let metrics_output = metrics.export()?;
    println!("   - Metrics 已生成: {}", !metrics_output.is_empty());
    println!();

    // 7. 示例 6: 使用 HandlerCore 直接操作
    println!("📝 示例 6: 使用 HandlerCore");
    let core = HandlerCore::new(server.clone());
    let tools = core.list_tools();
    println!("   - 工具列表:");
    for tool in &tools.tools {
        if let Some(desc) = &tool.description {
            println!("     * {} - {}", tool.name, desc);
        }
    }
    println!();

    // 8. 示例 7: CratesDocsHandlerCore 使用
    println!("📝 示例 7: CratesDocsHandlerCore");
    let _core_handler = CratesDocsHandlerCore::new(server);
    println!("   - 核心处理器已创建，支持细粒度控制");
    println!();

    // 9. 示例 8: 测试 HandlerConfig merge 的覆盖逻辑
    println!("📝 示例 8: 测试 merge 覆盖逻辑");

    // 测试 1: None 覆盖（返回原配置）
    let config1 = HandlerConfig::default();
    let merged1 = config1.merge(None);
    println!(
        "   - merge(None): verbose_logging={}, enable_metrics={}",
        merged1.verbose_logging, merged1.enable_metrics
    );

    // 测试 2: 部分覆盖
    let config_base = HandlerConfig::default();
    let config2 = HandlerConfig::new().with_verbose_logging();
    let merged2 = config_base.merge(Some(config2));
    println!(
        "   - merge(部分覆盖): verbose_logging={}, enable_metrics={}",
        merged2.verbose_logging, merged2.enable_metrics
    );

    // 测试 3: 完全覆盖
    let config_base2 = HandlerConfig::default();
    let config3 = HandlerConfig::new().with_verbose_logging().with_metrics();
    let merged3 = config_base2.merge(Some(config3));
    println!(
        "   - merge(完全覆盖): verbose_logging={}, enable_metrics={}",
        merged3.verbose_logging, merged3.enable_metrics
    );
    println!();

    println!("🎉 所有示例运行完成！\n");

    Ok(())
}
