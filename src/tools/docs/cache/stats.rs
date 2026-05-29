//! Cache statistics for document cache

use std::sync::atomic::{AtomicU64, Ordering};

/// Cache statistics tracker
#[derive(Debug, Default)]
pub struct CacheStats {
    /// Total cache hits
    hits: AtomicU64,
    /// Total cache misses
    misses: AtomicU64,
    /// Total cache sets
    sets: AtomicU64,
}

impl CacheStats {
    /// Create new cache statistics
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a cache hit
    pub fn record_hit(&self) {
        self.hits.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a cache miss
    pub fn record_miss(&self) {
        self.misses.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a cache set operation
    pub fn record_set(&self) {
        self.sets.fetch_add(1, Ordering::Relaxed);
    }

    /// Get total hits
    #[must_use]
    pub fn hits(&self) -> u64 {
        self.hits.load(Ordering::Relaxed)
    }

    /// Get total misses
    #[must_use]
    pub fn misses(&self) -> u64 {
        self.misses.load(Ordering::Relaxed)
    }

    /// Get total sets
    #[must_use]
    pub fn sets(&self) -> u64 {
        self.sets.load(Ordering::Relaxed)
    }

    /// Increment and get current hits count (atomic operation)
    #[must_use]
    pub fn inc_hits(&self) -> u64 {
        self.hits.fetch_add(1, Ordering::Relaxed) + 1
    }

    /// Increment and get current misses count (atomic operation)
    #[must_use]
    pub fn inc_misses(&self) -> u64 {
        self.misses.fetch_add(1, Ordering::Relaxed) + 1
    }

    /// Increment and get current sets count (atomic operation)
    #[must_use]
    pub fn inc_sets(&self) -> u64 {
        self.sets.fetch_add(1, Ordering::Relaxed) + 1
    }

    /// Get total requests (hits + misses)
    #[must_use]
    pub fn total_requests(&self) -> u64 {
        self.hits().saturating_add(self.misses())
    }

    /// Calculate hit rate (0.0 to 1.0)
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn hit_rate(&self) -> f64 {
        let total = self.total_requests();
        if total == 0 {
            return 0.0;
        }
        self.hits() as f64 / total as f64
    }

    /// Get all stats as a tuple (hits, misses, sets)
    #[must_use]
    pub fn as_tuple(&self) -> (u64, u64, u64) {
        (self.hits(), self.misses(), self.sets())
    }

    /// Reset all statistics
    pub fn reset(&self) {
        self.hits.store(0, Ordering::Relaxed);
        self.misses.store(0, Ordering::Relaxed);
        self.sets.store(0, Ordering::Relaxed);
    }
}

impl Clone for CacheStats {
    fn clone(&self) -> Self {
        Self {
            hits: AtomicU64::new(self.hits.load(Ordering::Relaxed)),
            misses: AtomicU64::new(self.misses.load(Ordering::Relaxed)),
            sets: AtomicU64::new(self.sets.load(Ordering::Relaxed)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_stats_new() {
        let stats = CacheStats::new();
        assert_eq!(stats.hits(), 0);
        assert_eq!(stats.misses(), 0);
        assert_eq!(stats.sets(), 0);
    }

    #[test]
    fn test_cache_stats_record() {
        let stats = CacheStats::new();

        stats.record_hit();
        stats.record_hit();
        stats.record_miss();
        stats.record_set();
        stats.record_set();
        stats.record_set();

        assert_eq!(stats.hits(), 2);
        assert_eq!(stats.misses(), 1);
        assert_eq!(stats.sets(), 3);
    }

    #[test]
    fn test_cache_stats_hit_rate() {
        let stats = CacheStats::new();

        assert!((stats.hit_rate() - 0.0).abs() < f64::EPSILON);

        stats.record_hit();
        stats.record_hit();
        stats.record_miss();

        let rate = stats.hit_rate();
        assert!((rate - 0.666_666_666_666_666_6).abs() < f64::EPSILON);
    }

    #[test]
    fn test_cache_stats_total_requests() {
        let stats = CacheStats::new();

        stats.record_hit();
        stats.record_hit();
        stats.record_miss();
        stats.record_miss();

        assert_eq!(stats.total_requests(), 4);
    }

    #[test]
    fn test_cache_stats_reset() {
        let stats = CacheStats::new();

        stats.record_hit();
        stats.record_miss();
        stats.record_set();

        stats.reset();

        assert_eq!(stats.hits(), 0);
        assert_eq!(stats.misses(), 0);
        assert_eq!(stats.sets(), 0);
    }

    #[test]
    fn test_cache_stats_clone() {
        let stats = CacheStats::new();
        stats.record_hit();
        stats.record_miss();

        let cloned = stats.clone();

        assert_eq!(cloned.hits(), 1);
        assert_eq!(cloned.misses(), 1);
    }
}
