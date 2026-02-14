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
    async fn fetch_crate_docs(
        &self,
        crate_name: &str,
        version: Option<&str>,
    ) -> std::result::Result<String, CallToolError> {
        // 尝试从缓存获取
        if let Some(cached) = self
            .service
            .doc_cache()
            .get_crate_docs(crate_name, version)
            .await
        {
            return Ok(cached);
        }

        // 构建 URL
        let url = if let Some(ver) = version {
            format!("https://docs.rs/{crate_name}/{ver}/")
        } else {
            format!("https://docs.rs/{crate_name}/")
        };

        // 发送 HTTP 请求（复用 DocService 的客户端）
        let response = self
            .service
            .client()
            .get(&url)
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

    /// 获取原始 HTML 文档（用于 text 格式）
    async fn fetch_raw_html(
        &self,
        crate_name: &str,
        version: Option<&str>,
    ) -> std::result::Result<String, CallToolError> {
        // 构建 URL
        let url = if let Some(ver) = version {
            format!("https://docs.rs/{crate_name}/{ver}/")
        } else {
            format!("https://docs.rs/{crate_name}/")
        };

        // 发送 HTTP 请求（复用 DocService 的客户端）
        let response = self
            .service
            .client()
            .get(&url)
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

        Ok(html)
    }
}

/// 从 HTML 中提取文档内容
fn extract_documentation(html: &str) -> String {
    // 先清理 HTML（移除 script, style, noscript 等标签及内容）
    let cleaned_html = clean_html(html);
    // 使用 html2md 库将清理后的 HTML 转换为 Markdown
    html2md::parse_html(&cleaned_html)
}

/// 清理 HTML，移除不需要的标签（script, style, noscript, iframe）及其内容
fn clean_html(html: &str) -> String {
    let mut result = String::new();
    let mut i = 0;
    let chars: Vec<char> = html.chars().collect();
    let len = chars.len();
    let mut skip_depth = 0; // 跟跳过标签的嵌套深度

    while i < len {
        let c = chars[i];

        if c == '<' {
            let start = i;
            let mut j = i + 1;

            // 收集标签名
            let mut tag_name = String::new();
            while j < len && chars[j] != '>' && !chars[j].is_whitespace() {
                tag_name.push(chars[j]);
                j += 1;
            }

            let tag_lower = tag_name.to_lowercase();
            let pure_tag = tag_lower.trim_start_matches('/');

            // 检查是否是需要跳过内容的标签
            let is_skip_tag = pure_tag == "script"
                || pure_tag == "style"
                || pure_tag == "noscript"
                || pure_tag == "iframe";

            if is_skip_tag {
                if tag_lower.starts_with('/') {
                    // 结束标签
                    if skip_depth > 0 {
                        skip_depth -= 1;
                    }
                    // 跳过整个标签
                    while j < len && chars[j] != '>' {
                        j += 1;
                    }
                    if j < len {
                        j += 1;
                    }
                    i = j;
                    continue;
                }

                // 开始标签
                skip_depth += 1;
                // 跳过整个标签
                while j < len && chars[j] != '>' {
                    j += 1;
                }
                if j < len {
                    j += 1;
                }
                i = j;
                continue;
            }

            // 跳过直到 '>'
            while j < len && chars[j] != '>' {
                j += 1;
            }
            if j < len {
                j += 1;
            }

            // 保留不是跳过标签的内容
            if skip_depth == 0 {
                result.extend(chars[start..j].iter().copied());
            }

            i = j;
        } else {
            if skip_depth == 0 {
                result.push(c);
            }
            i += 1;
        }
    }

    result
}

/// 将 HTML 转换为纯文本（移除所有 HTML 标签）
fn html_to_text(html: &str) -> String {
    let mut result = String::new();
    let mut skip_content = false; // 是否跳过标签内容（如 script, style）
    let mut i = 0;
    let chars: Vec<char> = html.chars().collect();
    let len = chars.len();

    while i < len {
        let c = chars[i];

        match c {
            '<' => {
                // 跳过标签
                let mut j = i + 1;
                let mut tag_name = String::new();

                // 收集标签名
                while j < len && chars[j] != '>' && !chars[j].is_whitespace() {
                    tag_name.push(chars[j]);
                    j += 1;
                }

                let tag_lower = tag_name.to_lowercase();
                let is_closing = tag_lower.starts_with('/');
                let pure_tag = tag_lower.trim_start_matches('/');

                // 检查是否是需要跳过内容的标签
                if !is_closing && !skip_content {
                    skip_content = pure_tag == "script"
                        || pure_tag == "style"
                        || pure_tag == "noscript"
                        || pure_tag == "iframe";
                } else if is_closing {
                    skip_content = false;
                }

                // 跳过整个标签
                while j < len && chars[j] != '>' {
                    j += 1;
                }
                if j < len {
                    j += 1; // 跳过 '>'
                }

                i = j;

                // 标签后添加空格（如果是块级元素）
                if !skip_content {
                    result.push(' ');
                }
            }
            '&' => {
                // 处理 HTML 实体
                let mut j = i + 1;
                let mut entity = String::new();
                while j < len && chars[j] != ';' {
                    entity.push(chars[j]);
                    j += 1;
                }
                if j < len {
                    j += 1; // 跳过 ';'
                }

                // 常见 HTML 实体映射
                let replacement = match entity.as_str() {
                    "lt" => "<",
                    "gt" => ">",
                    "amp" => "&",
                    "quot" => "\"",
                    "apos" => "'",
                    "nbsp" => " ",
                    _ => "",
                };
                if !replacement.is_empty() {
                    result.push_str(replacement);
                }
                i = j;
            }
            _ => {
                if !skip_content {
                    result.push(c);
                }
                i += 1;
            }
        }
    }

    // 清理多余的空白
    clean_whitespace(&result)
}

/// 清理多余的空白字符
fn clean_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[async_trait]
impl Tool for LookupCrateToolImpl {
    fn definition(&self) -> rust_mcp_sdk::schema::Tool {
        LookupCrateTool::tool()
    }

    async fn execute(
        &self,
        arguments: serde_json::Value,
    ) -> std::result::Result<
        rust_mcp_sdk::schema::CallToolResult,
        rust_mcp_sdk::schema::CallToolError,
    > {
        let params: LookupCrateTool = serde_json::from_value(arguments).map_err(|e| {
            rust_mcp_sdk::schema::CallToolError::invalid_arguments(
                "lookup_crate",
                Some(format!("参数解析失败: {e}")),
            )
        })?;

        let format = params.format.unwrap_or_else(|| "markdown".to_string());
        let content = match format.as_str() {
            "text" => {
                // 获取原始 HTML 并转换为纯文本
                let html = self
                    .fetch_raw_html(&params.crate_name, params.version.as_deref())
                    .await?;
                html_to_text(&html)
            }
            _ => {
                // "markdown" 和其他格式都返回原始文档
                self.fetch_crate_docs(&params.crate_name, params.version.as_deref())
                    .await?
            }
        };

        Ok(rust_mcp_sdk::schema::CallToolResult::text_content(vec![
            content.into(),
        ]))
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
pub struct LookupItemTool {
    /// crate 名称
    #[json_schema(title = "Crate 名称", description = "要查找的 crate 名称")]
    pub crate_name: String,

    /// 项目路径（例如 `std::collections::HashMap`）
    #[json_schema(
        title = "项目路径",
        description = "要查找的项目路径（例如 'std::collections::HashMap'）"
    )]
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
    async fn fetch_item_docs(
        &self,
        crate_name: &str,
        item_path: &str,
        version: Option<&str>,
    ) -> std::result::Result<String, CallToolError> {
        // 尝试从缓存获取
        if let Some(cached) = self
            .service
            .doc_cache()
            .get_item_docs(crate_name, item_path, version)
            .await
        {
            return Ok(cached);
        }

        // 构建搜索 URL
        let url = if let Some(ver) = version {
            format!(
                "https://docs.rs/{}/{}/?search={}",
                crate_name,
                ver,
                urlencoding::encode(item_path)
            )
        } else {
            format!(
                "https://docs.rs/{}/?search={}",
                crate_name,
                urlencoding::encode(item_path)
            )
        };

        // 发送 HTTP 请求（复用 DocService 的客户端）
        let response = self
            .service
            .client()
            .get(&url)
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

    /// 获取原始 HTML（用于 text 格式）
    async fn fetch_raw_html_for_item(
        &self,
        crate_name: &str,
        item_path: &str,
        version: Option<&str>,
    ) -> std::result::Result<String, CallToolError> {
        // 构建搜索 URL
        let url = if let Some(ver) = version {
            format!(
                "https://docs.rs/{}/{}/?search={}",
                crate_name,
                ver,
                urlencoding::encode(item_path)
            )
        } else {
            format!(
                "https://docs.rs/{}/?search={}",
                crate_name,
                urlencoding::encode(item_path)
            )
        };

        // 发送 HTTP 请求（复用 DocService 的客户端）
        let response = self
            .service
            .client()
            .get(&url)
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

        Ok(html)
    }
}

/// 从 HTML 中提取搜索结果
fn extract_search_results(html: &str, item_path: &str) -> String {
    // 先清理 HTML（移除 script, style, noscript 等标签及内容）
    let cleaned_html = clean_html(html);
    // 使用 html2md 库将清理后的 HTML 转换为 Markdown
    let markdown = html2md::parse_html(&cleaned_html);

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

    async fn execute(
        &self,
        arguments: serde_json::Value,
    ) -> std::result::Result<
        rust_mcp_sdk::schema::CallToolResult,
        rust_mcp_sdk::schema::CallToolError,
    > {
        let params: LookupItemTool = serde_json::from_value(arguments).map_err(|e| {
            rust_mcp_sdk::schema::CallToolError::invalid_arguments(
                "lookup_item",
                Some(format!("参数解析失败: {e}")),
            )
        })?;

        let format = params.format.unwrap_or_else(|| "markdown".to_string());
        let content = match format.as_str() {
            "text" => {
                // 获取原始 HTML 并转换为纯文本
                let html = self
                    .fetch_raw_html_for_item(
                        &params.crate_name,
                        &params.item_path,
                        params.version.as_deref(),
                    )
                    .await?;
                format!("搜索结果: {}\n\n{}", params.item_path, html_to_text(&html))
            }
            _ => {
                // "markdown" 和其他格式都返回原始文档
                self.fetch_item_docs(
                    &params.crate_name,
                    &params.item_path,
                    params.version.as_deref(),
                )
                .await?
            }
        };

        Ok(rust_mcp_sdk::schema::CallToolResult::text_content(vec![
            content.into(),
        ]))
    }
}

impl Default for LookupItemToolImpl {
    fn default() -> Self {
        Self::new(Arc::new(super::DocService::default()))
    }
}
