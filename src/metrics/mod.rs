//! Metrics module for Prometheus monitoring
//!
//! Provides metrics collection and export functionality for the MCP server.

use prometheus_client::encoding::text::encode;
use prometheus_client::metrics::counter::Counter;
use prometheus_client::metrics::family::Family;
use prometheus_client::metrics::gauge::Gauge;
use prometheus_client::metrics::histogram::{exponential_buckets, Histogram};
use prometheus_client::registry::Registry;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use std::time::Instant;

/// Metrics labels for request tracking
#[derive(Clone, Debug, Hash, PartialEq, Eq, prometheus_client::encoding::EncodeLabelSet)]
pub struct RequestLabels {
    /// Tool name
    pub tool: String,
    /// Status: success or error
    pub status: String,
}

/// Metrics labels for cache operations
#[derive(Clone, Debug, Hash, PartialEq, Eq, prometheus_client::encoding::EncodeLabelSet)]
pub struct CacheLabels {
    /// Operation type: get, set, hit, miss
    pub operation: String,
    /// Cache type: memory, redis
    pub cache_type: String,
}

/// Metrics labels for HTTP requests
#[derive(Clone, Debug, Hash, PartialEq, Eq, prometheus_client::encoding::EncodeLabelSet)]
pub struct HttpLabels {
    /// HTTP method
    pub method: String,
    /// HTTP status code
    pub status: String,
    /// Target host
    pub host: String,
}

/// Server metrics collection
pub struct ServerMetrics {
    /// Request counter
    request_counter: Family<RequestLabels, Counter>,
    /// Request duration histogram
    request_duration: Family<RequestLabels, Histogram>,
    /// Cache operation counter
    cache_counter: Family<CacheLabels, Counter>,
    /// Cache hits gauge
    cache_hits: Gauge<u64, AtomicU64>,
    /// Cache misses gauge
    cache_misses: Gauge<u64, AtomicU64>,
    /// Cache sets gauge
    cache_sets: Gauge<u64, AtomicU64>,
    /// Cache hit rate gauge
    cache_hit_rate: Gauge<f64, AtomicU64>,
    /// HTTP request counter
    http_counter: Family<HttpLabels, Counter>,
    /// HTTP request duration
    http_duration: Family<HttpLabels, Histogram>,
    /// Active connections gauge
    active_connections: Gauge<u64, AtomicU64>,
    /// Error counter
    error_counter: Family<RequestLabels, Counter>,
    /// Registry
    registry: Arc<Registry>,
}

impl ServerMetrics {
    /// Create a new metrics collection
    #[must_use]
    pub fn new() -> Self {
        let mut registry = Registry::default();

        // Request counter
        let request_counter = Family::<RequestLabels, Counter>::default();
        registry.register(
            "mcp_requests_total",
            "Total number of MCP tool requests",
            request_counter.clone(),
        );

        // Request duration histogram (exponential buckets from 1ms to 30s)
        let request_duration = Family::<RequestLabels, Histogram>::new_with_constructor(|| {
            Histogram::new(exponential_buckets(0.001, 2.0, 15))
        });
        registry.register(
            "mcp_request_duration_seconds",
            "MCP tool request duration in seconds",
            request_duration.clone(),
        );

        // Cache operation counter
        let cache_counter = Family::<CacheLabels, Counter>::default();
        registry.register(
            "mcp_cache_operations_total",
            "Total number of cache operations",
            cache_counter.clone(),
        );

        // Cache hits gauge (count of cache hits)
        let cache_hits = Gauge::default();
        registry.register(
            "mcp_cache_hits",
            "Number of cache hits (gauge)",
            cache_hits.clone(),
        );

        // Cache misses gauge (count of cache misses)
        let cache_misses = Gauge::default();
        registry.register(
            "mcp_cache_misses",
            "Number of cache misses (gauge)",
            cache_misses.clone(),
        );

        // Cache sets gauge (count of cache set operations)
        let cache_sets = Gauge::default();
        registry.register(
            "mcp_cache_sets",
            "Number of cache set operations (gauge)",
            cache_sets.clone(),
        );

        // Cache hit rate gauge
        let cache_hit_rate = Gauge::default();
        registry.register(
            "mcp_cache_hit_rate",
            "Cache hit rate (0.0 to 1.0)",
            cache_hit_rate.clone(),
        );

        // HTTP request counter
        let http_counter = Family::<HttpLabels, Counter>::default();
        registry.register(
            "mcp_http_requests_total",
            "Total number of HTTP requests",
            http_counter.clone(),
        );

        // HTTP request duration
        let http_duration = Family::<HttpLabels, Histogram>::new_with_constructor(|| {
            Histogram::new(exponential_buckets(0.001, 2.0, 15))
        });
        registry.register(
            "mcp_http_request_duration_seconds",
            "HTTP request duration in seconds",
            http_duration.clone(),
        );

        // Active connections gauge
        let active_connections = Gauge::<u64, AtomicU64>::default();
        registry.register(
            "mcp_active_connections",
            "Number of active connections",
            active_connections.clone(),
        );

        // Error counter
        let error_counter = Family::<RequestLabels, Counter>::default();
        registry.register(
            "mcp_errors_total",
            "Total number of errors",
            error_counter.clone(),
        );

        Self {
            request_counter,
            request_duration,
            cache_counter,
            cache_hits,
            cache_misses,
            cache_sets,
            cache_hit_rate,
            http_counter,
            http_duration,
            active_connections,
            error_counter,
            registry: Arc::new(registry),
        }
    }

    /// Record a tool request
    pub fn record_request(&self, tool: &str, success: bool, duration: std::time::Duration) {
        let labels = RequestLabels {
            tool: tool.to_string(),
            status: if success {
                "success".to_string()
            } else {
                "error".to_string()
            },
        };

        self.request_counter.get_or_create(&labels).inc();
        self.request_duration
            .get_or_create(&labels)
            .observe(duration.as_secs_f64());

        if !success {
            self.error_counter.get_or_create(&labels).inc();
        }
    }

    /// Record a cache operation
    pub fn record_cache_operation(&self, operation: &str, cache_type: &str) {
        let labels = CacheLabels {
            operation: operation.to_string(),
            cache_type: cache_type.to_string(),
        };
        self.cache_counter.get_or_create(&labels).inc();
    }

    /// Record a cache hit
    pub fn record_cache_hit(&self, cache_type: &str) {
        self.record_cache_operation("hit", cache_type);
    }

    /// Record a cache miss
    pub fn record_cache_miss(&self, cache_type: &str) {
        self.record_cache_operation("miss", cache_type);
    }

    /// Update cache hit rate
    #[allow(clippy::cast_precision_loss)]
    pub fn update_cache_hit_rate(&self, hits: u64, misses: u64) {
        let total = hits + misses;
        if total > 0 {
            let rate = hits as f64 / total as f64;
            self.cache_hit_rate.set(rate);
        }
    }

    /// Update cache statistics gauges from provided counts
    ///
    /// # Arguments
    ///
    /// * `hits` - Total cache hits
    /// * `misses` - Total cache misses
    /// * `sets` - Total cache set operations
    ///
    /// # Note
    ///
    /// This method also updates the cache hit rate automatically.
    pub fn update_cache_stats(&self, hits: u64, misses: u64, sets: u64) {
        self.cache_hits.set(hits);
        self.cache_misses.set(misses);
        self.cache_sets.set(sets);
        self.update_cache_hit_rate(hits, misses);
    }

    /// Record an HTTP request
    pub fn record_http_request(
        &self,
        method: &str,
        status: u16,
        host: &str,
        duration: std::time::Duration,
    ) {
        let labels = HttpLabels {
            method: method.to_string(),
            status: status.to_string(),
            host: host.to_string(),
        };

        self.http_counter.get_or_create(&labels).inc();
        self.http_duration
            .get_or_create(&labels)
            .observe(duration.as_secs_f64());
    }

    /// Increment active connections
    pub fn inc_active_connections(&self) {
        self.active_connections.inc();
    }

    /// Decrement active connections
    pub fn dec_active_connections(&self) {
        self.active_connections.dec();
    }

    /// Export metrics as Prometheus text format
    ///
    /// # Errors
    ///
    /// Returns an error if encoding fails
    pub fn export(&self) -> crate::error::Result<String> {
        let mut output = String::new();
        encode(&mut output, self.registry.as_ref())
            .map_err(|e| crate::error::Error::Other(format!("Failed to encode metrics: {e}")))?;
        Ok(output)
    }

    /// Get the registry
    #[must_use]
    pub fn registry(&self) -> &Arc<Registry> {
        &self.registry
    }
}

impl Default for ServerMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// Request timer for tracking request duration
pub struct RequestTimer {
    start: Instant,
    tool: String,
    metrics: Option<Arc<ServerMetrics>>,
}

impl RequestTimer {
    /// Create a new request timer
    #[must_use]
    pub fn new(tool: &str, metrics: Option<Arc<ServerMetrics>>) -> Self {
        Self {
            start: Instant::now(),
            tool: tool.to_string(),
            metrics,
        }
    }

    /// Record successful completion
    pub fn success(self) {
        self.record(true);
    }

    /// Record failed completion
    pub fn failure(self) {
        self.record(false);
    }

    fn record(self, success: bool) {
        if let Some(metrics) = self.metrics {
            metrics.record_request(&self.tool, success, self.start.elapsed());
        }
    }
}

/// HTTP request timer
pub struct HttpRequestTimer {
    start: Instant,
    method: String,
    host: String,
    metrics: Option<Arc<ServerMetrics>>,
}

impl HttpRequestTimer {
    /// Create a new HTTP request timer
    #[must_use]
    pub fn new(method: &str, host: &str, metrics: Option<Arc<ServerMetrics>>) -> Self {
        Self {
            start: Instant::now(),
            method: method.to_string(),
            host: host.to_string(),
            metrics,
        }
    }

    /// Record request completion with status code
    pub fn finish(self, status: u16) {
        if let Some(metrics) = self.metrics {
            metrics.record_http_request(&self.method, status, &self.host, self.start.elapsed());
        }
    }
}

use std::sync::OnceLock;

/// Global metrics instance (optional, for simple use cases)
static GLOBAL_METRICS: OnceLock<Arc<ServerMetrics>> = OnceLock::new();

/// Initialize global metrics
pub fn init_global_metrics() {
    let _ = GLOBAL_METRICS.set(Arc::new(ServerMetrics::new()));
}

/// Get global metrics
///
/// # Panics
///
/// Panics if global metrics have not been initialized
#[must_use]
pub fn global_metrics() -> Arc<ServerMetrics> {
    GLOBAL_METRICS
        .get()
        .cloned()
        .expect("Global metrics not initialized")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_creation() {
        let metrics = ServerMetrics::new();
        let output = metrics.export();
        assert!(output.is_ok());
        assert!(!output.unwrap().is_empty());
    }

    #[test]
    fn test_request_recording() {
        let metrics = ServerMetrics::new();

        // Record successful request
        metrics.record_request("test_tool", true, std::time::Duration::from_millis(100));

        // Record failed request
        metrics.record_request("test_tool", false, std::time::Duration::from_millis(200));

        let output = metrics.export().unwrap();
        assert!(output.contains("mcp_requests_total"));
        assert!(output.contains("test_tool"));
    }

    #[test]
    fn test_cache_metrics() {
        let metrics = ServerMetrics::new();

        metrics.record_cache_hit("memory");
        metrics.record_cache_miss("memory");
        metrics.update_cache_hit_rate(1, 1);

        let output = metrics.export().unwrap();
        assert!(output.contains("mcp_cache_operations_total"));
    }

    #[test]
    fn test_http_metrics() {
        let metrics = ServerMetrics::new();

        metrics.record_http_request("GET", 200, "docs.rs", std::time::Duration::from_millis(500));

        let output = metrics.export().unwrap();
        assert!(output.contains("mcp_http_requests_total"));
    }

    #[test]
    fn test_request_timer() {
        let metrics = Arc::new(ServerMetrics::new());
        let timer = RequestTimer::new("test_tool", Some(metrics.clone()));
        timer.success();

        // Verify metrics were recorded
        let output = metrics.export().unwrap();
        assert!(output.contains("mcp_requests_total"));
    }

    #[test]
    fn test_active_connections() {
        let metrics = ServerMetrics::new();

        metrics.inc_active_connections();
        metrics.inc_active_connections();
        metrics.dec_active_connections();

        let output = metrics.export().unwrap();
        assert!(output.contains("mcp_active_connections"));
    }
}
