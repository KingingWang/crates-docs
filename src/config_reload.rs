//! Configuration hot-reload functionality
//!
//! Provides file watching and configuration reloading capabilities for
//! runtime configuration updates without server restart.

use crate::config::AppConfig;
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::Path;
use std::sync::mpsc::{channel, Receiver};
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info, warn};

/// Configuration reloader for hot-reload support
///
/// Watches configuration file for changes and notifies when reload is needed.
pub struct ConfigReloader {
    /// Path to the configuration file
    config_path: Arc<Path>,
    /// File system watcher
    watcher: RecommendedWatcher,
    /// Event receiver
    receiver: Receiver<Result<Event, notify::Error>>,
    /// Current configuration (for comparison)
    current_config: AppConfig,
    /// Debounce timer to avoid rapid reloads
    last_reload: std::time::Instant,
}

impl ConfigReloader {
    /// Create a new configuration reloader
    ///
    /// # Arguments
    ///
    /// * `config_path` - Path to the configuration file
    /// * `current_config` - Current configuration for comparison
    ///
    /// # Errors
    ///
    /// Returns an error if the watcher cannot be created
    pub fn new(
        config_path: Arc<Path>,
        current_config: AppConfig,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let (sender, receiver) = channel();

        // Create file watcher
        let mut watcher = notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
            if let Err(e) = sender.send(res) {
                error!("Failed to send file system event: {}", e);
            }
        })?;

        // Watch the configuration file
        watcher.watch(&config_path, RecursiveMode::NonRecursive)?;

        info!(
            "Configuration hot-reload enabled, watching: {}",
            config_path.display()
        );

        Ok(Self {
            config_path,
            watcher,
            receiver,
            current_config,
            last_reload: std::time::Instant::now()
                .checked_sub(Duration::from_secs(10))
                .unwrap_or_else(std::time::Instant::now),
        })
    }

    /// Check for configuration changes
    ///
    /// This method should be called periodically in the event loop.
    ///
    /// # Returns
    ///
    /// Returns `Some(new_config)` if the configuration has changed and should be reloaded,
    /// or `None` if no changes were detected.
    pub fn check_for_changes(&mut self) -> Option<ConfigChange> {
        // Check for file system events (non-blocking)
        match self.receiver.try_recv() {
            Ok(Ok(event)) => {
                // Only process modification events
                if matches!(
                    event.kind,
                    EventKind::Modify(_) | EventKind::Create(_) | EventKind::Any
                ) {
                    // Debounce: only reload once per second
                    if self.last_reload.elapsed() < Duration::from_secs(1) {
                        return None;
                    }

                    self.last_reload = std::time::Instant::now();

                    info!("Configuration file changed, reloading...");

                    // Try to reload the configuration
                    match self.reload_config() {
                        Ok(change) => {
                            return Some(change);
                        }
                        Err(e) => {
                            error!("Failed to reload configuration: {}", e);
                            return None;
                        }
                    }
                }
            }
            Ok(Err(e)) => {
                warn!("File system watcher error: {}", e);
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                // No events available
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                warn!("File system watcher disconnected");
            }
        }

        None
    }

    /// Reload configuration from file
    ///
    /// # Errors
    ///
    /// Returns an error if the configuration cannot be loaded or parsed
    fn reload_config(&mut self) -> Result<ConfigChange, Box<dyn std::error::Error>> {
        let new_config = AppConfig::from_file(&self.config_path)?;

        // Detect what changed
        let change = self.detect_changes(&new_config);

        // Update current configuration
        self.current_config = new_config;

        Ok(change)
    }

    /// Detect changes between old and new configuration
    ///
    /// # Arguments
    ///
    /// * `new_config` - New configuration to compare
    ///
    /// # Returns
    ///
    /// Returns a description of what changed
    ///
    /// # Hot Reload Support Scope
    ///
    /// This method detects changes in the following configuration items (supports hot reload):
    /// - API Key authentication configuration (requires api-key feature)
    /// - OAuth configuration
    /// - Log level and toggles
    /// - Cache TTL configuration
    /// - Rate limiting and concurrent request limits
    /// - Metrics and compression toggles
    ///
    /// Note: Server basic configuration (host, port, etc.) changes will be detected and logged,
    /// but these configurations require server restart to take effect.
    #[allow(clippy::too_many_lines)]
    fn detect_changes(&self, new_config: &AppConfig) -> ConfigChange {
        let mut changes: Vec<String> = Vec::new();

        // Check API key changes
        #[cfg(feature = "api-key")]
        {
            if self.current_config.auth.api_key.enabled != new_config.auth.api_key.enabled {
                changes.push(if new_config.auth.api_key.enabled {
                    "API key authentication enabled".to_string()
                } else {
                    "API key authentication disabled".to_string()
                });
            }

            if self.current_config.auth.api_key.keys != new_config.auth.api_key.keys {
                let old_count = self.current_config.auth.api_key.keys.len();
                let new_count = new_config.auth.api_key.keys.len();
                changes.push(format!(
                    "API keys changed: {old_count} keys -> {new_count} keys"
                ));

                // Log added keys
                for key in &new_config.auth.api_key.keys {
                    if !self.current_config.auth.api_key.keys.contains(key) {
                        let key_type = if key.starts_with("legacy:") {
                            "Legacy Hash"
                        } else if key.starts_with("$argon2") {
                            "Argon2 Hash"
                        } else {
                            "Plaintext"
                        };
                        info!("  + Added API key ({})", key_type);
                    }
                }

                // Log removed keys
                for key in &self.current_config.auth.api_key.keys {
                    if !new_config.auth.api_key.keys.contains(key) {
                        let key_type = if key.starts_with("legacy:") {
                            "Legacy Hash"
                        } else if key.starts_with("$argon2") {
                            "Argon2 Hash"
                        } else {
                            "Plaintext"
                        };
                        info!("  - Removed API key ({})", key_type);
                    }
                }
            }

            if self.current_config.auth.api_key.header_name != new_config.auth.api_key.header_name {
                changes.push(format!(
                    "API key header name changed: {} -> {}",
                    self.current_config.auth.api_key.header_name,
                    new_config.auth.api_key.header_name
                ));
            }

            if self.current_config.auth.api_key.allow_query_param
                != new_config.auth.api_key.allow_query_param
            {
                changes.push(format!(
                    "API key query param allowed: {} -> {}",
                    self.current_config.auth.api_key.allow_query_param,
                    new_config.auth.api_key.allow_query_param
                ));
            }

            if self.current_config.auth.api_key.key_prefix != new_config.auth.api_key.key_prefix {
                changes.push(format!(
                    "API key prefix changed: {} -> {}",
                    self.current_config.auth.api_key.key_prefix, new_config.auth.api_key.key_prefix
                ));
            }
        }

        // Check OAuth configuration changes
        if self.current_config.oauth.enabled != new_config.oauth.enabled {
            changes.push(if new_config.oauth.enabled {
                "OAuth authentication enabled".to_string()
            } else {
                "OAuth authentication disabled".to_string()
            });
        }

        if self.current_config.oauth.client_id != new_config.oauth.client_id {
            changes.push("OAuth client ID changed".to_string());
        }

        if self.current_config.oauth.provider != new_config.oauth.provider {
            changes.push(format!(
                "OAuth provider changed: {:?} -> {:?}",
                self.current_config.oauth.provider, new_config.oauth.provider
            ));
        }

        // Check logging configuration changes (all fields support hot-reload)
        if self.current_config.logging.level != new_config.logging.level {
            changes.push(format!(
                "Log level changed: {} -> {}",
                self.current_config.logging.level, new_config.logging.level
            ));
        }

        if self.current_config.logging.enable_console != new_config.logging.enable_console {
            changes.push(format!(
                "Console logging {}",
                if new_config.logging.enable_console {
                    "enabled"
                } else {
                    "disabled"
                }
            ));
        }

        if self.current_config.logging.enable_file != new_config.logging.enable_file {
            changes.push(format!(
                "File logging {}",
                if new_config.logging.enable_file {
                    "enabled"
                } else {
                    "disabled"
                }
            ));
        }

        if self.current_config.logging.file_path != new_config.logging.file_path {
            changes.push(format!(
                "Log file path changed: {:?} -> {:?}",
                self.current_config.logging.file_path, new_config.logging.file_path
            ));
        }

        if self.current_config.logging.max_file_size_mb != new_config.logging.max_file_size_mb {
            changes.push(format!(
                "Max log file size changed: {}MB -> {}MB",
                self.current_config.logging.max_file_size_mb, new_config.logging.max_file_size_mb
            ));
        }

        if self.current_config.logging.max_files != new_config.logging.max_files {
            changes.push(format!(
                "Max log files changed: {} -> {}",
                self.current_config.logging.max_files, new_config.logging.max_files
            ));
        }

        // Check cache TTL configuration changes (support hot-reload)
        if self.current_config.cache.default_ttl != new_config.cache.default_ttl {
            changes.push(format!(
                "Cache default TTL changed: {:?} -> {:?}",
                self.current_config.cache.default_ttl, new_config.cache.default_ttl
            ));
        }

        if self.current_config.cache.crate_docs_ttl_secs != new_config.cache.crate_docs_ttl_secs {
            changes.push(format!(
                "Crate docs cache TTL changed: {:?} -> {:?}",
                self.current_config.cache.crate_docs_ttl_secs, new_config.cache.crate_docs_ttl_secs
            ));
        }

        if self.current_config.cache.item_docs_ttl_secs != new_config.cache.item_docs_ttl_secs {
            changes.push(format!(
                "Item docs cache TTL changed: {:?} -> {:?}",
                self.current_config.cache.item_docs_ttl_secs, new_config.cache.item_docs_ttl_secs
            ));
        }

        if self.current_config.cache.search_results_ttl_secs
            != new_config.cache.search_results_ttl_secs
        {
            changes.push(format!(
                "Search results cache TTL changed: {:?} -> {:?}",
                self.current_config.cache.search_results_ttl_secs,
                new_config.cache.search_results_ttl_secs
            ));
        }

        // Check performance configuration changes (hot-reloadable fields only)
        if self.current_config.performance.rate_limit_per_second
            != new_config.performance.rate_limit_per_second
        {
            changes.push(format!(
                "Rate limit changed: {} -> {} req/s",
                self.current_config.performance.rate_limit_per_second,
                new_config.performance.rate_limit_per_second
            ));
        }

        if self.current_config.performance.concurrent_request_limit
            != new_config.performance.concurrent_request_limit
        {
            changes.push(format!(
                "Concurrent request limit changed: {} -> {}",
                self.current_config.performance.concurrent_request_limit,
                new_config.performance.concurrent_request_limit
            ));
        }

        if self.current_config.performance.enable_metrics != new_config.performance.enable_metrics {
            changes.push(format!(
                "Prometheus metrics {}",
                if new_config.performance.enable_metrics {
                    "enabled"
                } else {
                    "disabled"
                }
            ));
        }

        if self.current_config.performance.enable_response_compression
            != new_config.performance.enable_response_compression
        {
            changes.push(format!(
                "Response compression {}",
                if new_config.performance.enable_response_compression {
                    "enabled"
                } else {
                    "disabled"
                }
            ));
        }

        // Check server configuration changes (require restart)
        // These are detected for logging purposes but require server restart
        let mut restart_required = false;

        if self.current_config.server.host != new_config.server.host {
            changes.push(format!(
                "[RESTART REQUIRED] Server host changed: {} -> {}",
                self.current_config.server.host, new_config.server.host
            ));
            restart_required = true;
        }

        if self.current_config.server.port != new_config.server.port {
            changes.push(format!(
                "[RESTART REQUIRED] Server port changed: {} -> {}",
                self.current_config.server.port, new_config.server.port
            ));
            restart_required = true;
        }

        if self.current_config.server.transport_mode != new_config.server.transport_mode {
            changes.push(format!(
                "[RESTART REQUIRED] Transport mode changed: {} -> {}",
                self.current_config.server.transport_mode, new_config.server.transport_mode
            ));
            restart_required = true;
        }

        if self.current_config.server.max_connections != new_config.server.max_connections {
            changes.push(format!(
                "[RESTART REQUIRED] Max connections changed: {} -> {}",
                self.current_config.server.max_connections, new_config.server.max_connections
            ));
            restart_required = true;
        }

        if restart_required {
            warn!("Some configuration changes require server restart to take effect");
        }

        if changes.is_empty() {
            ConfigChange::NoChange
        } else {
            ConfigChange::Changed {
                changes,
                new_config: Box::new(new_config.clone()),
            }
        }
    }

    /// Get current configuration
    #[must_use]
    pub fn current_config(&self) -> &AppConfig {
        &self.current_config
    }

    /// Stop watching for changes
    pub fn stop(mut self) {
        let _ = self.watcher.unwatch(&self.config_path);
    }
}

/// Configuration change description
#[derive(Debug, Clone)]
pub enum ConfigChange {
    /// No changes detected
    NoChange,
    /// Configuration has changed
    Changed {
        /// List of changes detected
        changes: Vec<String>,
        /// New configuration
        new_config: Box<AppConfig>,
    },
}

impl ConfigChange {
    /// Check if configuration has changed
    #[must_use]
    pub fn is_changed(&self) -> bool {
        matches!(self, ConfigChange::Changed { .. })
    }

    /// Get new configuration if changed
    #[must_use]
    pub fn new_config(&self) -> Option<&AppConfig> {
        match self {
            ConfigChange::Changed { new_config, .. } => Some(new_config),
            ConfigChange::NoChange => None,
        }
    }

    /// Get change descriptions
    #[must_use]
    pub fn changes(&self) -> Option<&[String]> {
        match self {
            ConfigChange::Changed { changes, .. } => Some(changes),
            ConfigChange::NoChange => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_config_change_detection_no_change() {
        let config1 = AppConfig::default();
        let config2 = AppConfig::default();

        // Create a temporary file for testing
        let mut temp_file = NamedTempFile::new().expect("Failed to create temp file");
        writeln!(temp_file, "[server]").expect("Failed to write to temp file");
        temp_file.flush().expect("Failed to flush temp file");

        let temp_path = temp_file.path();

        // Test no change - we create a reloader just to test detect_changes
        // Note: file watching won't work in tests, but we can test the logic
        let reloader = ConfigReloader::new(Arc::from(temp_path.to_path_buf()), config1.clone())
            .expect("Failed to create reloader");

        let change = reloader.detect_changes(&config2);
        assert!(matches!(change, ConfigChange::NoChange));
    }

    #[test]
    #[cfg(feature = "api-key")]
    fn test_config_change_detection_api_key_change() {
        let config1 = AppConfig::default();
        let mut config2 = AppConfig::default();

        // Test API key change
        config2.auth.api_key.keys.push("test_key".to_string());

        // Create a temporary file for testing
        let mut temp_file = NamedTempFile::new().expect("Failed to create temp file");
        writeln!(temp_file, "[server]").expect("Failed to write to temp file");
        temp_file.flush().expect("Failed to flush temp file");

        let temp_path = temp_file.path();

        let reloader = ConfigReloader::new(Arc::from(temp_path.to_path_buf()), config1.clone())
            .expect("Failed to create reloader");

        let change = reloader.detect_changes(&config2);
        assert!(matches!(change, ConfigChange::Changed { .. }));

        if let ConfigChange::Changed { changes, .. } = change {
            assert!(!changes.is_empty());
            assert!(changes[0].contains("API keys changed"));
        }
    }

    #[test]
    fn test_config_change_is_changed() {
        assert!(!ConfigChange::NoChange.is_changed());

        let change = ConfigChange::Changed {
            changes: vec!["test".to_string()],
            new_config: Box::new(AppConfig::default()),
        };
        assert!(change.is_changed());
    }

    #[test]
    fn test_config_change_new_config() {
        let change = ConfigChange::NoChange;
        assert!(change.new_config().is_none());

        let config = AppConfig::default();
        let change = ConfigChange::Changed {
            changes: vec!["test".to_string()],
            new_config: Box::new(config.clone()),
        };
        assert!(change.new_config().is_some());
    }

    #[test]
    fn test_config_change_changes() {
        let change = ConfigChange::NoChange;
        assert!(change.changes().is_none());

        let change = ConfigChange::Changed {
            changes: vec!["test".to_string()],
            new_config: Box::new(AppConfig::default()),
        };
        assert!(change.changes().is_some());
        assert_eq!(change.changes().unwrap().len(), 1);
    }
}
