//! 搜索 crate 工具
#![allow(missing_docs)]

use crate::tools::Tool;
use async_trait::async_trait;
use rust_mcp_sdk::macros;
use rust_mcp_sdk::schema::CallToolError;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// 搜索 crate 的工具参数
#[macros::mcp_tool(
    name = "search_crates",
    title = "搜索 Crates",
    description = "从 crates.io 搜索 Rust crate。返回匹配的 crate 列表，包括名称、描述、版本、下载量等信息。适用于发现和比较可用的 Rust 库。",
    destructive_hint = false,
    idempotent_hint = true,
    open_world_hint = false,
    read_only_hint = true,
    execution(task_support = "optional"),
    icons = [
        (src = "https://crates.io/favicon.ico", mime_type = "image/x-icon", sizes = ["32x32"], theme = "light"),
        (src = "https://crates.io/favicon.ico", mime_type = "image/x-icon", sizes = ["32x32"], theme = "dark")
    ]
)]
#[derive(Debug, Clone, Deserialize, Serialize, macros::JsonSchema)]
pub struct SearchCratesTool {
    /// 搜索查询
    #[json_schema(
        title = "搜索查询",
        description = "搜索关键词，例如：web framework、async、http client、serialization"
    )]
    pub query: String,

    /// 结果数量限制
    #[json_schema(
        title = "结果限制",
        description = "返回的最大结果数量，范围 1-100",
        minimum = 1,
        maximum = 100,
        default = 10
    )]
    pub limit: Option<u32>,

    /// 输出格式
    #[json_schema(
        title = "输出格式",
        description = "搜索结果输出格式：markdown（默认）、text（纯文本）、json（原始 JSON）",
        default = "markdown"
    )]
    pub format: Option<String>,
}

/// 搜索 crate 工具实现
pub struct SearchCratesToolImpl {
    service: Arc<super::DocService>,
}

impl SearchCratesToolImpl {
    /// 创建新的工具实例
    #[must_use]
    pub fn new(service: Arc<super::DocService>) -> Self {
        Self { service }
    }

    /// 搜索 crate
    async fn search_crates(
        &self,
        query: &str,
        limit: u32,
    ) -> std::result::Result<Vec<CrateInfo>, CallToolError> {
        // 构建缓存键
        let cache_key = format!("search:{query}:{limit}");

        // 检查缓存
        if let Some(cached) = self.service.cache().get(&cache_key).await {
            return serde_json::from_str(&cached)
                .map_err(|e| CallToolError::from_message(format!("缓存解析失败: {e}")));
        }

        // 构建 crates.io API URL
        let url = format!(
            "https://crates.io/api/v1/crates?q={}&per_page={}",
            urlencoding::encode(query),
            limit
        );

        // 发送 HTTP 请求
        let response = self
            .service
            .client()
            .get(&url)
            .header("User-Agent", format!("CratesDocsMCP/{}", crate::VERSION))
            .send()
            .await
            .map_err(|e| CallToolError::from_message(format!("HTTP 请求失败: {e}")))?;

        if !response.status().is_success() {
            return Err(CallToolError::from_message(format!(
                "搜索失败，状态码: {}",
                response.status()
            )));
        }

        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| CallToolError::from_message(format!("JSON 解析失败: {e}")))?;

        // 解析响应
        let crates = parse_crates_response(&json, limit as usize);

        // 缓存结果（5分钟）
        let cache_value = serde_json::to_string(&crates)
            .map_err(|e| CallToolError::from_message(format!("序列化失败: {e}")))?;

        self.service
            .cache()
            .set(
                cache_key,
                cache_value,
                Some(std::time::Duration::from_secs(300)),
            )
            .await;

        Ok(crates)
    }
}

/// Crate 信息
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CrateInfo {
    name: String,
    description: Option<String>,
    version: String,
    downloads: u64,
    repository: Option<String>,
    documentation: Option<String>,
}

/// 解析 crates.io API 响应
fn parse_crates_response(json: &serde_json::Value, limit: usize) -> Vec<CrateInfo> {
    let mut crates = Vec::new();

    if let Some(crates_array) = json.get("crates").and_then(|c| c.as_array()) {
        for crate_item in crates_array.iter().take(limit) {
            let name = crate_item
                .get("name")
                .and_then(|n| n.as_str())
                .unwrap_or("Unknown")
                .to_string();

            let description = crate_item
                .get("description")
                .and_then(|d| d.as_str())
                .map(std::string::ToString::to_string);

            let version = crate_item
                .get("max_version")
                .and_then(|v| v.as_str())
                .unwrap_or("0.0.0")
                .to_string();

            let downloads = crate_item
                .get("downloads")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0);

            let repository = crate_item
                .get("repository")
                .and_then(|r| r.as_str())
                .map(std::string::ToString::to_string);

            let documentation = crate_item
                .get("documentation")
                .and_then(|d| d.as_str())
                .map(std::string::ToString::to_string);

            crates.push(CrateInfo {
                name,
                description,
                version,
                downloads,
                repository,
                documentation,
            });
        }
    }

    crates
}

/// 格式化搜索结果
fn format_search_results(crates: &[CrateInfo], format: &str) -> String {
    match format {
        "json" => serde_json::to_string_pretty(crates).unwrap_or_else(|_| "[]".to_string()),
        "markdown" => {
            use std::fmt::Write;
            let mut output = String::from("# 搜索结果\n\n");

            for (i, crate_info) in crates.iter().enumerate() {
                writeln!(output, "## {}. {}", i + 1, crate_info.name).unwrap();
                writeln!(output, "**版本**: {}", crate_info.version).unwrap();
                writeln!(output, "**下载量**: {}", crate_info.downloads).unwrap();

                if let Some(desc) = &crate_info.description {
                    writeln!(output, "**描述**: {desc}").unwrap();
                }

                if let Some(repo) = &crate_info.repository {
                    writeln!(output, "**仓库**: [链接]({repo})").unwrap();
                }

                if let Some(docs) = &crate_info.documentation {
                    writeln!(output, "**文档**: [链接]({docs})").unwrap();
                }

                writeln!(
                    output,
                    "**Docs.rs**: [https://docs.rs/{}/](https://docs.rs/{}/)\n",
                    crate_info.name, crate_info.name
                )
                .unwrap();
            }

            output
        }
        "text" => {
            use std::fmt::Write;
            let mut output = String::new();

            for (i, crate_info) in crates.iter().enumerate() {
                writeln!(output, "{}. {}", i + 1, crate_info.name).unwrap();
                writeln!(output, "   版本: {}", crate_info.version).unwrap();
                writeln!(output, "   下载量: {}", crate_info.downloads).unwrap();

                if let Some(desc) = &crate_info.description {
                    writeln!(output, "   描述: {desc}").unwrap();
                }

                writeln!(output, "   Docs.rs: https://docs.rs/{}/", crate_info.name).unwrap();
                writeln!(output).unwrap();
            }

            output
        }
        _ => {
            // 默认使用 markdown
            format_search_results(crates, "markdown")
        }
    }
}

#[async_trait]
impl Tool for SearchCratesToolImpl {
    fn definition(&self) -> rust_mcp_sdk::schema::Tool {
        SearchCratesTool::tool()
    }

    async fn execute(
        &self,
        arguments: serde_json::Value,
    ) -> std::result::Result<
        rust_mcp_sdk::schema::CallToolResult,
        rust_mcp_sdk::schema::CallToolError,
    > {
        let params: SearchCratesTool = serde_json::from_value(arguments).map_err(|e| {
            rust_mcp_sdk::schema::CallToolError::invalid_arguments(
                "search_crates",
                Some(format!("参数解析失败: {e}")),
            )
        })?;

        let limit = params.limit.unwrap_or(10).min(100); // 限制最大100个结果
        let crates = self.search_crates(&params.query, limit).await?;

        let format = params.format.unwrap_or_else(|| "markdown".to_string());
        let content = format_search_results(&crates, &format);

        Ok(rust_mcp_sdk::schema::CallToolResult::text_content(vec![
            content.into(),
        ]))
    }
}

impl Default for SearchCratesToolImpl {
    fn default() -> Self {
        Self::new(Arc::new(super::DocService::default()))
    }
}
