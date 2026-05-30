//! Health check tool
//!
//! Provides functionality to check the health status of the server
//! and external services (docs.rs, crates.io). Used for diagnosing
//! connection issues and monitoring system availability.

#![allow(missing_docs)]

use crate::tools::Tool;
use async_trait::async_trait;
use rust_mcp_sdk::macros;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

/// The set of valid `check_type` values accepted by the `health_check` tool.
/// Kept in sync with the schema description and the `perform_checks` match.
const VALID_CHECK_TYPES: &[&str] = &["all", "external", "internal", "docs_rs", "crates_io"];

/// Parameters for the `health_check` tool
///
/// Defines the input parameters for performing health checks,
/// including the type of check to perform and verbosity level.
#[macros::mcp_tool(
    name = "health_check",
    title = "Health Check",
    description = "Check the health status of the server and external services (docs.rs, crates.io). Used for diagnosing connection issues and monitoring system availability.",
    destructive_hint = false,
    idempotent_hint = true,
    open_world_hint = false,
    read_only_hint = true,
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

/// Overall health check result containing all check results
#[derive(Debug, Clone, Serialize)]
struct HealthStatus {
    /// Overall status: "healthy", "unhealthy", or "degraded"
    status: String,
    /// Timestamp of the health check in RFC3339 format
    timestamp: String,
    /// Individual check results
    checks: Vec<HealthCheck>,
    /// Server uptime duration
    uptime: Duration,
}

/// Result of a single health check
#[derive(Debug, Clone, Serialize)]
struct HealthCheck {
    /// Name of the service checked
    name: String,
    /// Status: "healthy", "unhealthy", or "unknown"
    status: String,
    /// Duration of the check in milliseconds
    duration_ms: u64,
    /// Optional success message
    message: Option<String>,
    /// Optional error message if check failed
    error: Option<String>,
}

/// Implementation of the health check tool
///
/// Handles the execution of health checks for the server and external services,
/// including docs.rs and crates.io availability checks.
pub struct HealthCheckToolImpl {
    /// Server start time for uptime calculation
    start_time: Instant,
}

impl HealthCheckToolImpl {
    /// Creates a new health check tool instance
    ///
    /// Initializes the tool with the current time as the server start time
    /// for uptime calculation purposes.
    #[must_use]
    pub fn new() -> Self {
        Self {
            start_time: Instant::now(),
        }
    }

    #[allow(clippy::cast_possible_truncation)]
    async fn check_http_service(
        name: &'static str,
        url: &str,
        healthy_msg: &'static str,
    ) -> HealthCheck {
        let start = Instant::now();
        // Use global HTTP client singleton for connection pool reuse
        let client = match crate::utils::get_or_init_global_http_client() {
            Ok(client) => client,
            Err(e) => {
                return HealthCheck {
                    name: name.to_string(),
                    status: "unhealthy".to_string(),
                    duration_ms: start.elapsed().as_millis() as u64,
                    message: None,
                    error: Some(format!("Failed to initialize HTTP client: {e}")),
                };
            }
        };

        match client
            .get(url)
            .header("User-Agent", crate::user_agent())
            .timeout(Duration::from_secs(5))
            .send()
            .await
        {
            Ok(response) => {
                let duration = start.elapsed();
                if response.status().is_success() {
                    HealthCheck {
                        name: name.to_string(),
                        status: "healthy".to_string(),
                        duration_ms: duration.as_millis() as u64,
                        message: Some(healthy_msg.to_string()),
                        error: None,
                    }
                } else {
                    HealthCheck {
                        name: name.to_string(),
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
                    name: name.to_string(),
                    status: "unhealthy".to_string(),
                    duration_ms: duration.as_millis() as u64,
                    message: None,
                    error: Some(format!("Request failed: {e}")),
                }
            }
        }
    }

    #[inline]
    async fn check_docs_rs(&self) -> HealthCheck {
        Self::check_http_service("docs.rs", "https://docs.rs/", "Service is healthy").await
    }

    #[inline]
    async fn check_crates_io(&self) -> HealthCheck {
        Self::check_http_service(
            "crates.io",
            "https://crates.io/api/v1/crates?q=serde&per_page=1",
            "API is healthy",
        )
        .await
    }

    /// Check memory usage.
    ///
    /// On Linux this reports the process resident set size (RSS) read from
    /// `/proc/self/statm` so the "internal" health check carries real, useful
    /// information instead of a hard-coded "normal" verdict. On other platforms
    /// it reports that the metric is unavailable rather than fabricating one.
    fn check_memory() -> HealthCheck {
        let message = Self::memory_message();
        HealthCheck {
            name: "memory".to_string(),
            status: "healthy".to_string(),
            duration_ms: 0,
            message: Some(message),
            error: None,
        }
    }

    #[cfg(target_os = "linux")]
    fn memory_message() -> String {
        match Self::read_process_rss_bytes() {
            Some(bytes) => {
                // Integer math keeps this precise and avoids lossy float casts.
                let mib = bytes / (1024 * 1024);
                let frac = (bytes % (1024 * 1024)) * 10 / (1024 * 1024);
                format!("Resident set size: {mib}.{frac} MiB")
            }
            None => "Memory metrics unavailable (could not read /proc/self/statm)".to_string(),
        }
    }

    #[cfg(not(target_os = "linux"))]
    fn memory_message() -> String {
        "Memory metrics are not implemented on this platform".to_string()
    }

    /// Read the current process resident set size in bytes from `/proc`.
    #[cfg(target_os = "linux")]
    fn read_process_rss_bytes() -> Option<u64> {
        let statm = std::fs::read_to_string("/proc/self/statm").ok()?;
        // Field 2 (index 1) is the resident set size measured in memory pages.
        let resident_pages: u64 = statm.split_whitespace().nth(1)?.parse().ok()?;
        // SAFETY: `sysconf` is a pure libc query with no memory-safety impact.
        let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
        let page_size = u64::try_from(page_size).unwrap_or(4096);
        Some(resident_pages.saturating_mul(page_size))
    }

    async fn perform_checks(&self, check_type: &str, verbose: bool) -> HealthStatus {
        let checks = match check_type {
            "all" => {
                let (docs_rs, crates_io) =
                    tokio::join!(self.check_docs_rs(), self.check_crates_io());
                vec![docs_rs, crates_io, Self::check_memory()]
            }
            "external" => {
                let (docs_rs, crates_io) =
                    tokio::join!(self.check_docs_rs(), self.check_crates_io());
                vec![docs_rs, crates_io]
            }
            "internal" => vec![Self::check_memory()],
            "docs_rs" => vec![self.check_docs_rs().await],
            "crates_io" => vec![self.check_crates_io().await],
            _ => vec![HealthCheck {
                name: "unknown_check".to_string(),
                status: "unknown".to_string(),
                duration_ms: 0,
                message: None,
                error: Some(format!("Unknown check type: {check_type}")),
            }],
        };

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

    /// Render a [`HealthStatus`] into a report string.
    ///
    /// In verbose mode this returns pretty-printed JSON; otherwise a concise
    /// human-readable summary. This is shared by the MCP tool execution path
    /// and the CLI `health` command so their output stays consistent.
    fn render_report(health_status: &HealthStatus, verbose: bool) -> String {
        if verbose {
            serde_json::to_string_pretty(health_status)
                .unwrap_or_else(|e| format!("JSON serialization failed: {e}"))
        } else {
            let mut summary = format!(
                "Status: {}\nUptime: {:.2?}\nTimestamp: {}",
                health_status.status, health_status.uptime, health_status.timestamp
            );

            if !health_status.checks.is_empty() {
                use std::fmt::Write;
                summary.push_str("\n\nCheck Results:");
                for check in &health_status.checks {
                    let _ = write!(
                        summary,
                        "\n- {}: {} ({:.2}ms)",
                        check.name, check.status, check.duration_ms
                    );
                    if let Some(ref msg) = check.message {
                        let _ = write!(summary, " - {msg}");
                    }
                    if let Some(ref err) = check.error {
                        let _ = write!(summary, " [Error: {err}]");
                    }
                }
            }

            summary
        }
    }

    /// Run a health check for CLI usage.
    ///
    /// Returns the rendered report and whether the overall status is healthy
    /// (`true` only when every individual check is healthy). Callers can use the
    /// boolean to set a process exit code so container/orchestrator health
    /// probes behave correctly.
    pub async fn run_check_report(&self, check_type: &str, verbose: bool) -> (String, bool) {
        let health_status = self.perform_checks(check_type, verbose).await;
        let is_healthy = health_status.status == "healthy";
        (Self::render_report(&health_status, verbose), is_healthy)
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
        // Validate up front (fail-fast) like the other tools, rather than
        // emitting a misleading "degraded" report with an "unknown_check" for a
        // simple typo such as "al" instead of "all".
        if !VALID_CHECK_TYPES.contains(&check_type.as_str()) {
            return Err(rust_mcp_sdk::schema::CallToolError::invalid_arguments(
                "health_check",
                Some(format!(
                    "Invalid check_type '{check_type}'. Expected one of: {}",
                    VALID_CHECK_TYPES.join(", ")
                )),
            ));
        }
        let verbose = params.verbose.unwrap_or(false);

        let health_status = self.perform_checks(&check_type, verbose).await;

        let content = Self::render_report(&health_status, verbose);

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
