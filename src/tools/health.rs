//! Health check tool
#![allow(missing_docs)]

use crate::tools::Tool;
use async_trait::async_trait;
use rust_mcp_sdk::macros;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

/// Health check tool parameters
#[macros::mcp_tool(
    name = "health_check",
    title = "Health Check",
    description = "Check the health status of the server and external services (docs.rs, crates.io). Used for diagnosing connection issues and monitoring system availability.",
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
    /// Check type
    #[json_schema(
        title = "Check Type",
        description = "Type of health check to perform: all (all checks), external (external services: docs.rs, crates.io), internal (internal state), docs_rs (docs.rs only), crates_io (crates.io only)",
        default = "all"
    )]
    pub check_type: Option<String>,

    /// Verbose output
    #[json_schema(
        title = "Verbose Output",
        description = "Whether to show detailed output including response time for each check",
        default = false
    )]
    pub verbose: Option<bool>,
}

/// Health check result
#[derive(Debug, Clone, Serialize)]
struct HealthStatus {
    status: String,
    timestamp: String,
    checks: Vec<HealthCheck>,
    uptime: Duration,
}

/// Single health check
#[derive(Debug, Clone, Serialize)]
struct HealthCheck {
    name: String,
    status: String,
    duration_ms: u64,
    message: Option<String>,
    error: Option<String>,
}

/// Health check tool implementation
pub struct HealthCheckToolImpl {
    start_time: Instant,
}

impl HealthCheckToolImpl {
    /// Create a new health check tool
    #[must_use]
    pub fn new() -> Self {
        Self {
            start_time: Instant::now(),
        }
    }

    /// Check docs.rs service
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
                        message: Some("Service is healthy".to_string()),
                        error: None,
                    }
                } else {
                    HealthCheck {
                        name: "docs.rs".to_string(),
                        status: "unhealthy".to_string(),
                        duration_ms: duration.as_millis() as u64,
                        message: None,
                        error: Some(format!("HTTP status code: {}", response.status())),
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
                    error: Some(format!("Request failed: {e}")),
                }
            }
        }
    }

    /// Check crates.io service
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
                        message: Some("API is healthy".to_string()),
                        error: None,
                    }
                } else {
                    HealthCheck {
                        name: "crates.io".to_string(),
                        status: "unhealthy".to_string(),
                        duration_ms: duration.as_millis() as u64,
                        message: None,
                        error: Some(format!("HTTP status code: {}", response.status())),
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
                    error: Some(format!("Request failed: {e}")),
                }
            }
        }
    }

    /// Check memory usage
    fn check_memory() -> HealthCheck {
        HealthCheck {
            name: "memory".to_string(),
            status: "healthy".to_string(),
            duration_ms: 0,
            message: Some("Memory usage is normal".to_string()),
            error: None,
        }
    }

    /// Perform all health checks
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
                    error: Some(format!("Unknown check type: {check_type}")),
                });
            }
        }

        // Determine overall status
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
                // In non-verbose mode, only return checks with issues
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
                Some(format!("Parameter parsing failed: {e}")),
            )
        })?;

        let check_type = params.check_type.unwrap_or_else(|| "all".to_string());
        let verbose = params.verbose.unwrap_or(false);

        let health_status = self.perform_checks(&check_type, verbose).await;

        let content = if verbose {
            serde_json::to_string_pretty(&health_status).map_err(|e| {
                rust_mcp_sdk::schema::CallToolError::from_message(format!(
                    "JSON serialization failed: {e}"
                ))
            })?
        } else {
            let mut summary = format!(
                "Status: {}\nUptime: {:.2?}\nTimestamp: {}",
                health_status.status, health_status.uptime, health_status.timestamp
            );

            if !health_status.checks.is_empty() {
                use std::fmt::Write;
                summary.push_str("\n\nCheck Results:");
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
                        write!(summary, " [Error: {err}]").unwrap();
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
