//! 查找 crate 文档工具
#![allow(clippy::no_effect_replace)]
#![allow(missing_docs)]

use crate::tools::docs::DocService;
use crate::tools::Tool;
use async_trait::async_trait;
use rust_mcp_sdk::schema::CallToolError;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// 查找 crate 文档工具
#[rust_mcp_sdk::macros::mcp_tool(
    name = "lookup_crate",
    title = "查找 Crate 文档",
    description = "从 docs.rs 获取 Rust crate 的文档",
    destructive_hint = false,
    idempotent_hint = true,
    open_world_hint = false,
    read_only_hint = true,
    execution(task_support = "optional"),
    icons = [
        (src = "https://docs.rs/favicon.ico", mime_type = "image/x-icon", sizes = ["32x32"], theme = "light"),
        (src = "https://docs.rs/favicon.ico", mime_type = "image/x-icon", sizes = ["32x32"], theme = "dark")
    ]
)]
#[derive(Debug, Clone, Deserialize, Serialize, rust_mcp_sdk::macros::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LookupCrateTool {
    /// crate 名称
    #[json_schema(title = "Crate 名称", description = "要查找的 crate 名称")]
    pub crate_name: String,
    
    /// 版本号（可选，默认为最新版本）
    #[json_schema(title = "版本号", description = "crate 版本号（可选，默认为最新版本）")]
    pub version: Option<String>,
    
    /// 输出格式：markdown、text 或 html
    #[json_schema(title = "输出格式", description = "文档输出格式", default = "markdown")]
    pub format: Option<String>,
}

/// 查找 crate 文档工具实现
pub struct LookupCrateToolImpl {
    service: Arc<DocService>,
}

impl LookupCrateToolImpl {
    /// 创建新的查找工具实例
    #[must_use]
    pub fn new(service: Arc<DocService>) -> Self {
        Self { service }
    }

    /// 获取 crate 文档
    async fn fetch_crate_docs(&self, crate_name: &str, version: Option<&str>) -> std::result::Result<String, CallToolError> {
        // 尝试从缓存获取
        if let Some(cached) = self.service.doc_cache().get_crate_docs(crate_name, version).await {
            return Ok(cached);
        }

        // 构建 URL
        let url = if let Some(ver) = version {
            format!("https://docs.rs/{crate_name}/{ver}/")
        } else {
            format!("https://docs.rs/{crate_name}/")
        };

        // 发送 HTTP 请求
        let client = reqwest::Client::new();
        let response = client
            .get(&url)
            .header("User-Agent", "crates-docs-mcp/0.1.0")
            .send()
            .await
            .map_err(|e| CallToolError::from_message(format!("HTTP 请求失败: {e}")))?;

        if !response.status().is_success() {
            return Err(CallToolError::from_message(format!(
                "获取文档失败: HTTP {} - {}",
                response.status(),
                response.text().await.unwrap_or_default()
            )));
        }

        let html = response
            .text()
            .await
            .map_err(|e| CallToolError::from_message(format!("读取响应失败: {e}")))?;

        // 提取文档内容
        let docs = extract_documentation(&html);

        // 缓存结果
        self.service
            .doc_cache()
            .set_crate_docs(crate_name, version, docs.clone())
            .await;

        Ok(docs)
    }
}

/// 从 HTML 中提取文档内容
fn extract_documentation(html: &str) -> String {
    // 使用 html2md 库将 HTML 转换为 Markdown
    html2md::parse_html(html)
}

#[async_trait]
impl Tool for LookupCrateToolImpl {
    fn definition(&self) -> rust_mcp_sdk::schema::Tool {
        LookupCrateTool::tool()
    }
    
    async fn execute(&self, arguments: serde_json::Value) -> std::result::Result<rust_mcp_sdk::schema::CallToolResult, rust_mcp_sdk::schema::CallToolError> {
        let params: LookupCrateTool = serde_json::from_value(arguments)
            .map_err(|e| rust_mcp_sdk::schema::CallToolError::invalid_arguments("lookup_crate", Some(format!("参数解析失败: {e}"))))?;
        
        let docs = self.fetch_crate_docs(&params.crate_name, params.version.as_deref()).await?;
        
        let format = params.format.unwrap_or_else(|| "markdown".to_string());
        let content = match format.as_str() {
            "text" => html2md::parse_html(&docs),
            "html" => format!("<pre><code>{}</code></pre>", docs.replace('<', "<").replace('>', ">")),
            _ => docs, // "markdown" 和其他格式都返回原始文档
        };
        
        Ok(rust_mcp_sdk::schema::CallToolResult::text_content(vec![content.into()]))
    }
}

impl Default for LookupCrateToolImpl {
    fn default() -> Self {
        Self::new(Arc::new(super::DocService::default()))
    }
}

/// 查找 crate 中的特定项目工具
#[rust_mcp_sdk::macros::mcp_tool(
    name = "lookup_item",
    title = "查找 Crate 项目文档",
    description = "从 docs.rs 获取 Rust crate 中特定项目的文档",
    destructive_hint = false,
    idempotent_hint = true,
    open_world_hint = false,
    read_only_hint = true,
    execution(task_support = "optional"),
    icons = [
        (src = "https://docs.rs/favicon.ico", mime_type = "image/x-icon", sizes = ["32x32"], theme = "light"),
        (src = "https://docs.rs/favicon.ico", mime_type = "image/x-icon", sizes = ["32x32"], theme = "dark")
    ]
)]
#[derive(Debug, Clone, Deserialize, Serialize, rust_mcp_sdk::macros::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LookupItemTool {
    /// crate 名称
    #[json_schema(title = "Crate 名称", description = "要查找的 crate 名称")]
    pub crate_name: String,
    
    /// 项目路径（例如 `std::collections::HashMap`）
    #[json_schema(title = "项目路径", description = "要查找的项目路径（例如 'std::collections::HashMap'）")]
    pub item_path: String,
    
    /// 版本号（可选，默认为最新版本）
    #[json_schema(title = "版本号", description = "crate 版本号（可选，默认为最新版本）")]
    pub version: Option<String>,
    
    /// 输出格式：markdown、text 或 html
    #[json_schema(title = "输出格式", description = "文档输出格式", default = "markdown")]
    pub format: Option<String>,
}

/// 查找 crate 中的特定项目工具实现
pub struct LookupItemToolImpl {
    service: Arc<DocService>,
}

impl LookupItemToolImpl {
    /// 创建新的查找项目工具实例
    #[must_use]
    pub fn new(service: Arc<DocService>) -> Self {
        Self { service }
    }

    /// 获取项目文档
    async fn fetch_item_docs(&self, crate_name: &str, item_path: &str, version: Option<&str>) -> std::result::Result<String, CallToolError> {
        // 尝试从缓存获取
        if let Some(cached) = self.service.doc_cache().get_item_docs(crate_name, item_path, version).await {
            return Ok(cached);
        }

        // 构建搜索 URL
        let url = if let Some(ver) = version {
            format!("https://docs.rs/{}/{}/?search={}", crate_name, ver, urlencoding::encode(item_path))
        } else {
            format!("https://docs.rs/{}/?search={}", crate_name, urlencoding::encode(item_path))
        };

        // 发送 HTTP 请求
        let client = reqwest::Client::new();
        let response = client
            .get(&url)
            .header("User-Agent", "crates-docs-mcp/0.1.0")
            .send()
            .await
            .map_err(|e| CallToolError::from_message(format!("HTTP 请求失败: {e}")))?;

        if !response.status().is_success() {
            return Err(CallToolError::from_message(format!(
                "获取项目文档失败: HTTP {} - {}",
                response.status(),
                response.text().await.unwrap_or_default()
            )));
        }

        let html = response
            .text()
            .await
            .map_err(|e| CallToolError::from_message(format!("读取响应失败: {e}")))?;

        // 提取搜索结果
        let docs = extract_search_results(&html, item_path);

        // 缓存结果
        self.service
            .doc_cache()
            .set_item_docs(crate_name, item_path, version, docs.clone())
            .await;

        Ok(docs)
    }
}

/// 从 HTML 中提取搜索结果
fn extract_search_results(html: &str, item_path: &str) -> String {
    // 使用 html2md 库将 HTML 转换为 Markdown
    let markdown = html2md::parse_html(html);
    
    // 如果搜索结果为空，返回提示信息
    if markdown.trim().is_empty() {
        format!("未找到项目 '{item_path}' 的文档")
    } else {
        format!("## 搜索结果: {item_path}\n\n{markdown}")
    }
}

#[async_trait]
impl Tool for LookupItemToolImpl {
    fn definition(&self) -> rust_mcp_sdk::schema::Tool {
        LookupItemTool::tool()
    }
    
    async fn execute(&self, arguments: serde_json::Value) -> std::result::Result<rust_mcp_sdk::schema::CallToolResult, rust_mcp_sdk::schema::CallToolError> {
        let params: LookupItemTool = serde_json::from_value(arguments)
            .map_err(|e| rust_mcp_sdk::schema::CallToolError::invalid_arguments("lookup_item", Some(format!("参数解析失败: {e}"))))?;
        
        let docs = self.fetch_item_docs(&params.crate_name, &params.item_path, params.version.as_deref()).await?;
        
        let format = params.format.unwrap_or_else(|| "markdown".to_string());
        let content = match format.as_str() {
            "text" => html2md::parse_html(&docs),
            "html" => format!("<pre><code>{}</code></pre>", docs.replace('<', "<").replace('>', ">")),
            _ => docs, // "markdown" 和其他格式都返回原始文档
        };
        
        Ok(rust_mcp_sdk::schema::CallToolResult::text_content(vec![content.into()]))
    }
}

impl Default for LookupItemToolImpl {
    fn default() -> Self {
        Self::new(Arc::new(super::DocService::default()))
    }
}