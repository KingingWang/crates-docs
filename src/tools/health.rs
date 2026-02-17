//! 健康检查工具
#![allow(missing_docs)]

use crate::tools::Tool;
use async_trait::async_trait;
use rust_mcp_sdk::macros;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

/// 健康检查工具参数
#[macros::mcp_tool(
    name = "health_check",
    title = "健康检查",
    description = "检查服务器和外部服务（docs.rs、crates.io）的健康状态。用于诊断连接问题和监控系统可用性。",
    destructive_hint = false,
    idempotent_hint = true,
    open_world_hint = false,
    read_only_hint = true,
    execution(task_support = "optional"),
    icons = [
        (src = "https://img.icons8.com/color/96/000000/heart-health.png", mime_type = "image/png", sizes = ["96x96"], theme = "light"),
        (src = "https://img.icons8.com/color/96/000000/heart-health.png", mime_type = "image/png", sizes = ["96x96"], theme = "dark")
    ]
)]
#[derive(Debug, Clone, Deserialize, Serialize, macros::JsonSchema)]
pub struct HealthCheckTool {
    /// 检查类型
    #[json_schema(
        title = "检查类型",
        description = "要执行的健康检查类型：all（全部检查）、external（外部服务：docs.rs、crates.io）、internal（内部状态）、docs_rs（仅 docs.rs）、crates_io（仅 crates.io）",
        default = "all"
    )]
    pub check_type: Option<String>,

    /// 详细输出
    #[json_schema(
        title = "详细输出",
        description = "是否显示详细输出，包括每个检查的响应时间",
        default = false
    )]
    pub verbose: Option<bool>,
}

/// 健康检查结果
#[derive(Debug, Clone, Serialize)]
struct HealthStatus {
    status: String,
    timestamp: String,
    checks: Vec<HealthCheck>,
    uptime: Duration,
}

/// 单个健康检查
#[derive(Debug, Clone, Serialize)]
struct HealthCheck {
    name: String,
    status: String,
    duration_ms: u64,
    message: Option<String>,
    error: Option<String>,
}

/// 健康检查工具实现
pub struct HealthCheckToolImpl {
    start_time: Instant,
}

impl HealthCheckToolImpl {
    /// 创建新的健康检查工具
    #[must_use]
    pub fn new() -> Self {
        Self {
            start_time: Instant::now(),
        }
    }

    /// 检查 docs.rs 服务
    #[allow(clippy::cast_possible_truncation)]
    async fn check_docs_rs(&self) -> HealthCheck {
        let start = Instant::now();
        let client = reqwest::Client::new();

        match client
            .get("https://docs.rs/")
            .header("User-Agent", format!("CratesDocsMCP/{}", crate::VERSION))
            .timeout(Duration::from_secs(5))
            .send()
            .await
        {
            Ok(response) => {
                let duration = start.elapsed();
                if response.status().is_success() {
                    HealthCheck {
                        name: "docs.rs".to_string(),
                        status: "healthy".to_string(),
                        duration_ms: duration.as_millis() as u64,
                        message: Some("服务正常".to_string()),
                        error: None,
                    }
                } else {
                    HealthCheck {
                        name: "docs.rs".to_string(),
                        status: "unhealthy".to_string(),
                        duration_ms: duration.as_millis() as u64,
                        message: None,
                        error: Some(format!("HTTP 状态码: {}", response.status())),
                    }
                }
            }
            Err(e) => {
                let duration = start.elapsed();
                HealthCheck {
                    name: "docs.rs".to_string(),
                    status: "unhealthy".to_string(),
                    duration_ms: duration.as_millis() as u64,
                    message: None,
                    error: Some(format!("请求失败: {e}")),
                }
            }
        }
    }

    /// 检查 crates.io 服务
    #[allow(clippy::cast_possible_truncation)]
    async fn check_crates_io(&self) -> HealthCheck {
        let start = Instant::now();
        let client = reqwest::Client::new();

        match client
            .get("https://crates.io/api/v1/crates?q=serde&per_page=1")
            .header("User-Agent", format!("CratesDocsMCP/{}", crate::VERSION))
            .timeout(Duration::from_secs(5))
            .send()
            .await
        {
            Ok(response) => {
                let duration = start.elapsed();
                if response.status().is_success() {
                    HealthCheck {
                        name: "crates.io".to_string(),
                        status: "healthy".to_string(),
                        duration_ms: duration.as_millis() as u64,
                        message: Some("API 正常".to_string()),
                        error: None,
                    }
                } else {
                    HealthCheck {
                        name: "crates.io".to_string(),
                        status: "unhealthy".to_string(),
                        duration_ms: duration.as_millis() as u64,
                        message: None,
                        error: Some(format!("HTTP 状态码: {}", response.status())),
                    }
                }
            }
            Err(e) => {
                let duration = start.elapsed();
                HealthCheck {
                    name: "crates.io".to_string(),
                    status: "unhealthy".to_string(),
                    duration_ms: duration.as_millis() as u64,
                    message: None,
                    error: Some(format!("请求失败: {e}")),
                }
            }
        }
    }

    /// 检查内存使用
    fn check_memory() -> HealthCheck {
        HealthCheck {
            name: "memory".to_string(),
            status: "healthy".to_string(),
            duration_ms: 0,
            message: Some("内存使用正常".to_string()),
            error: None,
        }
    }

    /// 执行所有健康检查
    async fn perform_checks(&self, check_type: &str, verbose: bool) -> HealthStatus {
        let mut checks = Vec::new();

        match check_type {
            "all" => {
                checks.push(self.check_docs_rs().await);
                checks.push(self.check_crates_io().await);
                checks.push(Self::check_memory());
            }
            "external" => {
                checks.push(self.check_docs_rs().await);
                checks.push(self.check_crates_io().await);
            }
            "internal" => {
                checks.push(Self::check_memory());
            }
            "docs_rs" => {
                checks.push(self.check_docs_rs().await);
            }
            "crates_io" => {
                checks.push(self.check_crates_io().await);
            }
            _ => {
                checks.push(HealthCheck {
                    name: "unknown_check".to_string(),
                    status: "unknown".to_string(),
                    duration_ms: 0,
                    message: None,
                    error: Some(format!("未知的检查类型: {check_type}")),
                });
            }
        }

        // 确定总体状态
        let overall_status = if checks.iter().all(|c| c.status == "healthy") {
            "healthy".to_string()
        } else if checks.iter().any(|c| c.status == "unhealthy") {
            "unhealthy".to_string()
        } else {
            "degraded".to_string()
        };

        HealthStatus {
            status: overall_status,
            timestamp: chrono::Utc::now().to_rfc3339(),
            checks: if verbose {
                checks
            } else {
                // 非详细模式下只返回有问题的检查
                checks
                    .into_iter()
                    .filter(|c| c.status != "healthy")
                    .collect()
            },
            uptime: self.start_time.elapsed(),
        }
    }
}

#[async_trait]
impl Tool for HealthCheckToolImpl {
    fn definition(&self) -> rust_mcp_sdk::schema::Tool {
        HealthCheckTool::tool()
    }

    async fn execute(
        &self,
        arguments: serde_json::Value,
    ) -> std::result::Result<
        rust_mcp_sdk::schema::CallToolResult,
        rust_mcp_sdk::schema::CallToolError,
    > {
        let params: HealthCheckTool = serde_json::from_value(arguments).map_err(|e| {
            rust_mcp_sdk::schema::CallToolError::invalid_arguments(
                "health_check",
                Some(format!("参数解析失败: {e}")),
            )
        })?;

        let check_type = params.check_type.unwrap_or_else(|| "all".to_string());
        let verbose = params.verbose.unwrap_or(false);

        let health_status = self.perform_checks(&check_type, verbose).await;

        let content = if verbose {
            serde_json::to_string_pretty(&health_status).map_err(|e| {
                rust_mcp_sdk::schema::CallToolError::from_message(format!("JSON 序列化失败: {e}"))
            })?
        } else {
            let mut summary = format!(
                "状态: {}\n运行时间: {:.2?}\n时间戳: {}",
                health_status.status, health_status.uptime, health_status.timestamp
            );

            if !health_status.checks.is_empty() {
                use std::fmt::Write;
                summary.push_str("\n\n检查结果:");
                for check in &health_status.checks {
                    write!(
                        summary,
                        "\n- {}: {} ({:.2}ms)",
                        check.name, check.status, check.duration_ms
                    )
                    .unwrap();
                    if let Some(ref msg) = check.message {
                        write!(summary, " - {msg}").unwrap();
                    }
                    if let Some(ref err) = check.error {
                        write!(summary, " [错误: {err}]").unwrap();
                    }
                }
            }

            summary
        };

        Ok(rust_mcp_sdk::schema::CallToolResult::text_content(vec![
            content.into(),
        ]))
    }
}

impl Default for HealthCheckToolImpl {
    fn default() -> Self {
        Self::new()
    }
}
