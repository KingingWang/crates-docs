//! Utility functions module

use crate::error::{Error, Result};
use reqwest::Client;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;

/// HTTP client builder
pub struct HttpClientBuilder {
    timeout: Duration,
    connect_timeout: Duration,
    pool_max_idle_per_host: usize,
    user_agent: String,
    enable_gzip: bool,
    enable_brotli: bool,
}

impl Default for HttpClientBuilder {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(30),
            connect_timeout: Duration::from_secs(10),
            pool_max_idle_per_host: 10,
            user_agent: format!("CratesDocsMCP/{}", crate::VERSION),
            enable_gzip: true,
            enable_brotli: true,
        }
    }
}

impl HttpClientBuilder {
    /// Create a new HTTP client builder
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set request timeout
    #[must_use]
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Set connection timeout
    #[must_use]
    pub fn connect_timeout(mut self, connect_timeout: Duration) -> Self {
        self.connect_timeout = connect_timeout;
        self
    }

    /// Set connection pool size
    #[must_use]
    pub fn pool_max_idle_per_host(mut self, max_idle: usize) -> Self {
        self.pool_max_idle_per_host = max_idle;
        self
    }

    /// Set User-Agent
    #[must_use]
    pub fn user_agent(mut self, user_agent: String) -> Self {
        self.user_agent = user_agent;
        self
    }

    /// Enable/disable Gzip compression
    #[must_use]
    pub fn enable_gzip(mut self, enable: bool) -> Self {
        self.enable_gzip = enable;
        self
    }

    /// Enable/disable Brotli compression
    #[must_use]
    pub fn enable_brotli(mut self, enable: bool) -> Self {
        self.enable_brotli = enable;
        self
    }

    /// Build HTTP client
    pub fn build(self) -> Result<Client> {
        let mut builder = Client::builder()
            .timeout(self.timeout)
            .connect_timeout(self.connect_timeout)
            .pool_max_idle_per_host(self.pool_max_idle_per_host)
            .user_agent(&self.user_agent);

        // reqwest 0.13 enables gzip and brotli by default
        // To disable, use .no_gzip() and .no_brotli()
        if !self.enable_gzip {
            builder = builder.no_gzip();
        }

        if !self.enable_brotli {
            builder = builder.no_brotli();
        }

        builder
            .build()
            .map_err(|e| Error::HttpRequest(e.to_string()))
    }
}

/// Rate limiter
pub struct RateLimiter {
    semaphore: Arc<Semaphore>,
    max_permits: usize,
}

impl RateLimiter {
    /// Create a new rate limiter
    #[must_use]
    pub fn new(max_permits: usize) -> Self {
        Self {
            semaphore: Arc::new(Semaphore::new(max_permits)),
            max_permits,
        }
    }

    /// Acquire permit (blocks until available)
    pub async fn acquire(&self) -> Result<tokio::sync::SemaphorePermit<'_>> {
        self.semaphore
            .acquire()
            .await
            .map_err(|e| Error::Other(format!("Failed to acquire rate limit permit: {e}")))
    }

    /// Try to acquire permit (non-blocking)
    #[must_use]
    pub fn try_acquire(&self) -> Option<tokio::sync::SemaphorePermit<'_>> {
        self.semaphore.try_acquire().ok()
    }

    /// Get current number of available permits
    #[must_use]
    pub fn available_permits(&self) -> usize {
        self.semaphore.available_permits()
    }

    /// Get maximum number of permits
    #[must_use]
    pub fn max_permits(&self) -> usize {
        self.max_permits
    }
}

/// Response compression utilities
pub mod compression {
    use crate::error::{Error, Result};
    use flate2::write::GzEncoder;
    use flate2::Compression;
    use std::io::Write;

    /// Compress data (Gzip)
    pub fn gzip_compress(data: &[u8]) -> Result<Vec<u8>> {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder
            .write_all(data)
            .map_err(|e| Error::Other(format!("Gzip compression failed: {e}")))?;
        encoder
            .finish()
            .map_err(|e| Error::Other(format!("Gzip compression finalize failed: {e}")))
    }

    /// Decompress data (Gzip)
    pub fn gzip_decompress(data: &[u8]) -> Result<Vec<u8>> {
        let mut decoder = flate2::read::GzDecoder::new(data);
        let mut decompressed = Vec::new();
        std::io::Read::read_to_end(&mut decoder, &mut decompressed)
            .map_err(|e| Error::Other(format!("Gzip decompression failed: {e}")))?;
        Ok(decompressed)
    }
}

/// String utilities
pub mod string {
    /// Truncate string and add ellipsis
    #[must_use]
    pub fn truncate_with_ellipsis(s: &str, max_len: usize) -> String {
        if s.len() <= max_len {
            return s.to_string();
        }

        if max_len <= 3 {
            return "...".to_string();
        }

        format!("{}...", &s[..max_len - 3])
    }

    /// Safely parse number
    pub fn parse_number<T: std::str::FromStr>(s: &str, default: T) -> T {
        s.parse().unwrap_or(default)
    }

    /// Check if string is empty or blank
    #[must_use]
    pub fn is_blank(s: &str) -> bool {
        s.trim().is_empty()
    }
}

/// Time utilities
pub mod time {
    use chrono::{DateTime, Utc};

    /// Get current timestamp (milliseconds)
    #[must_use]
    pub fn current_timestamp_ms() -> i64 {
        Utc::now().timestamp_millis()
    }

    /// Format datetime
    #[must_use]
    pub fn format_datetime(dt: &DateTime<Utc>) -> String {
        dt.format("%Y-%m-%d %H:%M:%S%.3f").to_string()
    }

    /// Calculate elapsed time (milliseconds)
    #[must_use]
    pub fn elapsed_ms(start: std::time::Instant) -> u128 {
        start.elapsed().as_millis()
    }
}

/// Validation utilities
pub mod validation {
    use crate::error::Error;

    /// Validate crate name
    pub fn validate_crate_name(name: &str) -> Result<(), Error> {
        if name.is_empty() {
            return Err(Error::Other("Crate name cannot be empty".to_string()));
        }

        if name.len() > 100 {
            return Err(Error::Other("Crate name is too long".to_string()));
        }

        // Basic validation: only allow letters, digits, underscores, hyphens
        if !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        {
            return Err(Error::Other("Crate name contains invalid characters".to_string()));
        }

        Ok(())
    }

    /// Validate version number
    pub fn validate_version(version: &str) -> Result<(), Error> {
        if version.is_empty() {
            return Err(Error::Other("Version cannot be empty".to_string()));
        }

        if version.len() > 50 {
            return Err(Error::Other("Version is too long".to_string()));
        }

        // Simple validation: should contain digits and dots
        if !version.chars().any(|c| c.is_ascii_digit()) {
            return Err(Error::Other("Version must contain digits".to_string()));
        }

        Ok(())
    }

    /// Validate search query
    pub fn validate_search_query(query: &str) -> Result<(), Error> {
        if query.is_empty() {
            return Err(Error::Other("Search query cannot be empty".to_string()));
        }

        if query.len() > 200 {
            return Err(Error::Other("Search query is too long".to_string()));
        }

        Ok(())
    }
}

/// Performance monitoring
pub mod metrics {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Arc;
    use std::time::Instant;

    /// Performance counter
    #[derive(Clone)]
    pub struct PerformanceCounter {
        total_requests: Arc<AtomicU64>,
        successful_requests: Arc<AtomicU64>,
        failed_requests: Arc<AtomicU64>,
        total_response_time_ms: Arc<AtomicU64>,
    }

    impl PerformanceCounter {
        /// Create a new performance counter
        #[must_use]
        pub fn new() -> Self {
            Self {
                total_requests: Arc::new(AtomicU64::new(0)),
                successful_requests: Arc::new(AtomicU64::new(0)),
                failed_requests: Arc::new(AtomicU64::new(0)),
                total_response_time_ms: Arc::new(AtomicU64::new(0)),
            }
        }

        /// Record request start
        #[must_use]
        pub fn record_request_start(&self) -> Instant {
            self.total_requests.fetch_add(1, Ordering::Relaxed);
            Instant::now()
        }

        /// Record request completion
        #[allow(clippy::cast_possible_truncation)]
        pub fn record_request_complete(&self, start: Instant, success: bool) {
            let duration_ms = start.elapsed().as_millis() as u64;
            self.total_response_time_ms
                .fetch_add(duration_ms, Ordering::Relaxed);

            if success {
                self.successful_requests.fetch_add(1, Ordering::Relaxed);
            } else {
                self.failed_requests.fetch_add(1, Ordering::Relaxed);
            }
        }

        /// Get statistics
        #[must_use]
        pub fn get_stats(&self) -> PerformanceStats {
            let total = self.total_requests.load(Ordering::Relaxed);
            let success = self.successful_requests.load(Ordering::Relaxed);
            let failed = self.failed_requests.load(Ordering::Relaxed);
            let total_time = self.total_response_time_ms.load(Ordering::Relaxed);

            #[allow(clippy::cast_precision_loss)]
            let avg_response_time = if total > 0 {
                total_time as f64 / total as f64
            } else {
                0.0
            };

            #[allow(clippy::cast_precision_loss)]
            let success_rate = if total > 0 {
                success as f64 / total as f64 * 100.0
            } else {
                0.0
            };

            PerformanceStats {
                total_requests: total,
                successful_requests: success,
                failed_requests: failed,
                average_response_time_ms: avg_response_time,
                success_rate_percent: success_rate,
            }
        }

        /// Reset counter
        pub fn reset(&self) {
            self.total_requests.store(0, Ordering::Relaxed);
            self.successful_requests.store(0, Ordering::Relaxed);
            self.failed_requests.store(0, Ordering::Relaxed);
            self.total_response_time_ms.store(0, Ordering::Relaxed);
        }
    }

    impl Default for PerformanceCounter {
        fn default() -> Self {
            Self::new()
        }
    }

    /// Performance statistics
    #[derive(Debug, Clone, serde::Serialize)]
    pub struct PerformanceStats {
        /// Total requests
        pub total_requests: u64,
        /// Successful requests
        pub successful_requests: u64,
        /// Failed requests
        pub failed_requests: u64,
        /// Average response time (milliseconds)
        pub average_response_time_ms: f64,
        /// Success rate (percentage)
        pub success_rate_percent: f64,
    }
}
