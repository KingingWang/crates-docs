//! TTL (Time-To-Live) management for document cache

use std::time::Duration;

/// Default TTL jitter ratio (10%)
///
/// # Value
///
/// 0.1 (10%)
///
/// # Rationale
///
/// A 10% jitter helps prevent cache stampede when multiple requests expire simultaneously.
/// This spreads the load over time while maintaining reasonable cache consistency.
/// Configurable via `DocCacheTtl::jitter_ratio` field.
const DEFAULT_JITTER_RATIO: f64 = 0.1;

/// Minimum valid jitter ratio
///
/// Must be > 0.0 to have any effect. A value of 0.0 disables jitter.
const MIN_JITTER_RATIO: f64 = 0.0;

/// Maximum valid jitter ratio
///
/// Must be <= 1.0 (100%) to prevent negative or excessive TTL values.
const MAX_JITTER_RATIO: f64 = 1.0;

/// Default crate documentation TTL in seconds
///
/// # Value
///
/// 3600 seconds (1 hour)
///
/// # Rationale
///
/// Crate documentation changes infrequently and is relatively large, making it suitable for longer caching.
/// This reduces load on docs.rs while ensuring reasonable freshness.
/// Configurable via `CacheConfig::crate_docs_ttl_secs`.
const DEFAULT_CRATE_DOCS_TTL_SECS: u64 = 3600;

/// Default search results TTL in seconds
///
/// # Value
///
/// 300 seconds (5 minutes)
///
/// # Rationale
///
/// Search results change frequently as new crates are published.
/// Short TTL ensures users see recent additions while still benefiting from caching.
/// Configurable via `CacheConfig::search_results_ttl_secs`.
const DEFAULT_SEARCH_RESULTS_TTL_SECS: u64 = 300;

/// Default item documentation TTL in seconds
///
/// # Value
///
/// 1800 seconds (30 minutes)
///
/// # Rationale
///
/// Item documentation (functions, structs) changes moderately often.
/// Medium TTL balances freshness with performance.
/// Configurable via `CacheConfig::item_docs_ttl_secs`.
const DEFAULT_ITEM_DOCS_TTL_SECS: u64 = 1800;

/// Document cache TTL configuration
///
/// Configure independent TTL for different document types.
///
/// # Fields
///
/// - `crate_docs_secs`: Crate document cache duration (seconds)
/// - `search_results_secs`: search results cache duration (seconds)
/// - `item_docs_secs`: item docs cache duration (seconds)
/// - `jitter_ratio`: TTL jitter ratio(0.0-1.0),used to prevent cache stampede
#[derive(Debug, Clone, Copy)]
pub struct DocCacheTtl {
    /// Crate document TTL (seconds)
    pub crate_docs_secs: u64,
    /// Search results TTL (seconds)
    pub search_results_secs: u64,
    /// Item documentation TTL (seconds)
    pub item_docs_secs: u64,
    /// TTL jitter ratio (0.0-1.0), default 0.1 (10%)
    ///
    /// Actual TTL = `base_ttl * (1 + random(-jitter_ratio, jitter_ratio))`
    /// for example:`base_ttl=3600`, `jitter_ratio=0.1` => Actual TTL range `[3240, 3960]`
    ///
    /// Use `set_jitter_ratio()` to modify this value with validation.
    jitter_ratio: f64,
}

impl Default for DocCacheTtl {
    fn default() -> Self {
        Self {
            crate_docs_secs: DEFAULT_CRATE_DOCS_TTL_SECS,
            search_results_secs: DEFAULT_SEARCH_RESULTS_TTL_SECS,
            item_docs_secs: DEFAULT_ITEM_DOCS_TTL_SECS,
            jitter_ratio: DEFAULT_JITTER_RATIO,
        }
    }
}

impl DocCacheTtl {
    /// Create TTL configuration from `CacheConfig`
    ///
    /// # Arguments
    ///
    /// * `config` - cache configuration
    ///
    /// # Returns
    ///
    /// Returns TTL configuration based on config with validated `jitter_ratio`
    #[must_use]
    pub fn from_cache_config(config: &crate::cache::CacheConfig) -> Self {
        Self {
            crate_docs_secs: config
                .crate_docs_ttl_secs
                .unwrap_or(DEFAULT_CRATE_DOCS_TTL_SECS),
            search_results_secs: config
                .search_results_ttl_secs
                .unwrap_or(DEFAULT_SEARCH_RESULTS_TTL_SECS),
            item_docs_secs: config
                .item_docs_ttl_secs
                .unwrap_or(DEFAULT_ITEM_DOCS_TTL_SECS),
            jitter_ratio: DEFAULT_JITTER_RATIO,
        }
    }

    /// Create TTL configuration with custom jitter ratio
    ///
    /// # Arguments
    ///
    /// * `crate_docs_secs` - Crate docs TTL in seconds
    /// * `search_results_secs` - Search results TTL in seconds
    /// * `item_docs_secs` - Item docs TTL in seconds
    /// * `jitter_ratio` - Jitter ratio (0.0-1.0), out-of-range values are clamped
    ///
    /// # Returns
    ///
    /// Returns TTL configuration with validated and clamped `jitter_ratio`
    #[must_use]
    pub fn with_jitter(
        crate_docs_secs: u64,
        search_results_secs: u64,
        item_docs_secs: u64,
        jitter_ratio: f64,
    ) -> Self {
        Self {
            crate_docs_secs,
            search_results_secs,
            item_docs_secs,
            jitter_ratio: Self::validate_jitter_ratio(jitter_ratio),
        }
    }

    /// Validate and clamp jitter ratio to valid range
    ///
    /// Ensures `jitter_ratio` is within `[MIN_JITTER_RATIO, MAX_JITTER_RATIO]`.
    /// Values outside this range are clamped to the nearest valid value.
    ///
    /// # Arguments
    ///
    /// * `ratio` - The jitter ratio to validate
    ///
    /// # Returns
    ///
    /// Returns clamped jitter ratio in range [0.0, 1.0]
    #[must_use]
    fn validate_jitter_ratio(ratio: f64) -> f64 {
        if ratio.is_nan() || ratio < MIN_JITTER_RATIO {
            MIN_JITTER_RATIO
        } else if ratio > MAX_JITTER_RATIO {
            MAX_JITTER_RATIO
        } else {
            ratio
        }
    }

    /// Get the current jitter ratio
    #[must_use]
    pub const fn jitter_ratio(&self) -> f64 {
        self.jitter_ratio
    }

    /// Set the jitter ratio with validation
    ///
    /// Values outside [0.0, 1.0] range are clamped to the nearest valid value.
    /// NaN values are treated as 0.0.
    ///
    /// # Arguments
    ///
    /// * `ratio` - The jitter ratio to set
    ///
    /// # Examples
    ///
    /// ```
    /// use crates_docs::tools::docs::cache::DocCacheTtl;
    ///
    /// let mut ttl = DocCacheTtl::default();
    /// ttl.set_jitter_ratio(0.2);
    /// assert!((ttl.jitter_ratio() - 0.2).abs() < f64::EPSILON);
    ///
    /// // Out of range values are clamped
    /// ttl.set_jitter_ratio(1.5);
    /// assert!((ttl.jitter_ratio() - 1.0).abs() < f64::EPSILON);
    /// ```
    pub fn set_jitter_ratio(&mut self, ratio: f64) {
        self.jitter_ratio = Self::validate_jitter_ratio(ratio);
    }

    /// Calculate actual TTL with jitter
    ///
    /// # Arguments
    ///
    /// * `base_ttl` - Base TTL (seconds)
    ///
    /// # Returns
    ///
    /// Returns jittered TTL (seconds)
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    #[allow(clippy::cast_sign_loss)]
    #[allow(clippy::cast_precision_loss)]
    pub fn apply_jitter(&self, base_ttl: u64) -> u64 {
        // Clamp jitter_ratio to valid range for safety (handles direct struct construction)
        let ratio = self.jitter_ratio.clamp(MIN_JITTER_RATIO, MAX_JITTER_RATIO);

        if ratio <= MIN_JITTER_RATIO {
            return base_ttl;
        }

        let rng = fastrand::f64();
        let offset = (rng * 2.0 - 1.0) * ratio;

        (base_ttl as f64 * (1.0 + offset)).max(1.0) as u64
    }

    /// Get TTL duration for crate docs with jitter applied
    #[must_use]
    pub fn crate_docs_duration(&self) -> Duration {
        Duration::from_secs(self.apply_jitter(self.crate_docs_secs))
    }

    /// Get TTL duration for search results with jitter applied
    #[must_use]
    pub fn search_results_duration(&self) -> Duration {
        Duration::from_secs(self.apply_jitter(self.search_results_secs))
    }

    /// Get TTL duration for item docs with jitter applied
    #[must_use]
    pub fn item_docs_duration(&self) -> Duration {
        Duration::from_secs(self.apply_jitter(self.item_docs_secs))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_doc_cache_ttl_default() {
        let ttl = DocCacheTtl::default();
        assert_eq!(ttl.crate_docs_secs, DEFAULT_CRATE_DOCS_TTL_SECS);
        assert_eq!(ttl.search_results_secs, DEFAULT_SEARCH_RESULTS_TTL_SECS);
        assert_eq!(ttl.item_docs_secs, DEFAULT_ITEM_DOCS_TTL_SECS);
        assert!((ttl.jitter_ratio() - DEFAULT_JITTER_RATIO).abs() < f64::EPSILON);
    }

    #[test]
    fn test_doc_cache_ttl_from_config() {
        let config = crate::cache::CacheConfig {
            cache_type: "memory".to_string(),
            memory_size: Some(1000),
            redis_url: None,
            key_prefix: String::new(),
            default_ttl: Some(DEFAULT_CRATE_DOCS_TTL_SECS),
            crate_docs_ttl_secs: Some(7200),
            item_docs_ttl_secs: Some(DEFAULT_CRATE_DOCS_TTL_SECS),
            search_results_ttl_secs: Some(600),
        };
        let ttl = DocCacheTtl::from_cache_config(&config);
        assert_eq!(ttl.crate_docs_secs, 7200);
        assert_eq!(ttl.item_docs_secs, DEFAULT_CRATE_DOCS_TTL_SECS);
        assert_eq!(ttl.search_results_secs, 600);
    }

    #[test]
    fn test_apply_jitter_no_jitter() {
        let mut ttl = DocCacheTtl::default();
        ttl.set_jitter_ratio(0.0);
        assert_eq!(ttl.apply_jitter(1000), 1000);
    }

    #[test]
    fn test_apply_jitter_with_jitter() {
        let mut ttl = DocCacheTtl::default();
        ttl.set_jitter_ratio(0.5);

        for _ in 0..100 {
            let jittered = ttl.apply_jitter(1000);
            assert!((500..=1500).contains(&jittered));
        }
    }

    #[test]
    fn test_durations() {
        let mut ttl = DocCacheTtl::default();
        ttl.set_jitter_ratio(0.0);
        ttl.crate_docs_secs = DEFAULT_CRATE_DOCS_TTL_SECS;
        ttl.search_results_secs = DEFAULT_SEARCH_RESULTS_TTL_SECS;
        ttl.item_docs_secs = DEFAULT_ITEM_DOCS_TTL_SECS;

        assert_eq!(
            ttl.crate_docs_duration(),
            Duration::from_secs(DEFAULT_CRATE_DOCS_TTL_SECS)
        );
        assert_eq!(
            ttl.search_results_duration(),
            Duration::from_secs(DEFAULT_SEARCH_RESULTS_TTL_SECS)
        );
        assert_eq!(
            ttl.item_docs_duration(),
            Duration::from_secs(DEFAULT_ITEM_DOCS_TTL_SECS)
        );
    }

    #[test]
    fn test_jitter_ratio_setter_validation() {
        let mut ttl = DocCacheTtl::default();

        // Valid values should be accepted
        ttl.set_jitter_ratio(0.5);
        assert!((ttl.jitter_ratio() - 0.5).abs() < f64::EPSILON);

        // Value at boundaries should be accepted
        ttl.set_jitter_ratio(0.0);
        assert!((ttl.jitter_ratio()).abs() < f64::EPSILON);

        ttl.set_jitter_ratio(1.0);
        assert!((ttl.jitter_ratio() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_jitter_ratio_clamping() {
        let mut ttl = DocCacheTtl::default();

        // Values > 1.0 should be clamped to 1.0
        ttl.set_jitter_ratio(1.5);
        assert!((ttl.jitter_ratio() - 1.0).abs() < f64::EPSILON);

        ttl.set_jitter_ratio(100.0);
        assert!((ttl.jitter_ratio() - 1.0).abs() < f64::EPSILON);

        // Negative values should be clamped to 0.0
        ttl.set_jitter_ratio(-0.1);
        assert!(ttl.jitter_ratio().abs() < f64::EPSILON);

        ttl.set_jitter_ratio(-100.0);
        assert!(ttl.jitter_ratio().abs() < f64::EPSILON);
    }

    #[test]
    fn test_jitter_ratio_nan_handling() {
        let mut ttl = DocCacheTtl::default();

        // NaN should be treated as 0.0
        ttl.set_jitter_ratio(f64::NAN);
        assert!(ttl.jitter_ratio().abs() < f64::EPSILON);
    }

    #[test]
    fn test_jitter_ratio_infinity_handling() {
        let mut ttl = DocCacheTtl::default();

        // Positive infinity should be clamped to 1.0
        ttl.set_jitter_ratio(f64::INFINITY);
        assert!((ttl.jitter_ratio() - 1.0).abs() < f64::EPSILON);

        // Negative infinity should be clamped to 0.0
        ttl.set_jitter_ratio(f64::NEG_INFINITY);
        assert!(ttl.jitter_ratio().abs() < f64::EPSILON);
    }

    #[test]
    fn test_apply_jitter_with_extreme_values() {
        // Test with jitter_ratio = 0.0 (no jitter)
        let mut ttl = DocCacheTtl::default();
        ttl.set_jitter_ratio(0.0);
        assert_eq!(ttl.apply_jitter(1000), 1000);

        // Test with jitter_ratio = 1.0 (max jitter, range [0, 2000])
        ttl.set_jitter_ratio(1.0);
        for _ in 0..100 {
            let jittered = ttl.apply_jitter(1000);
            assert!((0..=2000).contains(&jittered));
        }
    }
}
