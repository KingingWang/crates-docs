//! Handler configuration

/// Handler configuration (supports merging)
///
/// Used to configure handler behavior, such as metrics integration, log level, etc.
#[derive(Debug, Clone, Default)]
pub struct HandlerConfig {
    /// Whether to enable verbose logging
    pub verbose_logging: bool,
    /// Whether to record metrics
    pub enable_metrics: bool,
}

impl HandlerConfig {
    /// Create new configuration
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Enable verbose logging
    #[must_use]
    pub fn with_verbose_logging(self) -> Self {
        Self {
            verbose_logging: true,
            ..self
        }
    }

    /// Enable metrics
    #[must_use]
    pub fn with_metrics(self) -> Self {
        Self {
            enable_metrics: true,
            ..self
        }
    }

    /// Merge configuration (other takes precedence over self)
    #[must_use]
    pub fn merge(self, other: Option<Self>) -> Self {
        match other {
            Some(other) => Self {
                verbose_logging: other.verbose_logging || self.verbose_logging,
                enable_metrics: other.enable_metrics || self.enable_metrics,
            },
            None => self,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handler_config_merge() {
        // Test with override case
        let base = HandlerConfig::default();
        let override_config = HandlerConfig::new().with_verbose_logging().with_metrics();

        let merged = base.merge(Some(override_config));
        assert!(merged.verbose_logging);
        assert!(merged.enable_metrics);

        // Test empty override (use new base)
        let base2 = HandlerConfig::default();
        let merged_empty = base2.merge(None);
        assert!(!merged_empty.verbose_logging);
        assert!(!merged_empty.enable_metrics);
    }

    #[test]
    fn test_handler_config_chained() {
        let config = HandlerConfig::new().with_verbose_logging().with_metrics();

        assert!(config.verbose_logging);
        assert!(config.enable_metrics);
    }
}
