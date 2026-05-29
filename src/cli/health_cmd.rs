//! Health check command implementation

use crate::tools::health::HealthCheckToolImpl;
use std::path::Path;

/// Run the `health` CLI command.
///
/// Performs real health checks against the server's internal state and the
/// external services it depends on (docs.rs, crates.io), then prints a report.
///
/// Returns an error (so the process exits with a non-zero status) when the
/// overall status is not healthy. This makes the command usable as a container
/// or orchestrator health probe (e.g. the Docker Compose `healthcheck`).
///
/// Recognized `check_type` values: `all`, `external`, `internal`, `docs_rs`,
/// `crates_io`. Unknown values produce a degraded (non-healthy) report.
pub async fn run_health_command(
    config_path: &Path,
    check_type: &str,
    verbose: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    // Honor the global `--config` flag: initialize the shared HTTP client from
    // the configured performance settings (user-agent, timeouts, pool) so the
    // external probes behave like the running server. Falls back to defaults
    // when the file is absent, and ignores re-initialization races.
    if config_path.exists() {
        if let Ok(app_config) = crate::config::AppConfig::from_file(config_path) {
            let _ = crate::utils::init_global_http_client(&app_config.performance);
        }
    }

    let tool = HealthCheckToolImpl::new();
    let (report, is_healthy) = tool.run_check_report(check_type, verbose).await;

    println!("{report}");

    if is_healthy {
        Ok(())
    } else {
        Err(
            format!("Health check did not report a healthy status (check_type: {check_type})")
                .into(),
        )
    }
}
