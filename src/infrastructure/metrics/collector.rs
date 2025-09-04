use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

/// Performance metrics collector for system performance monitoring
#[derive(Debug, Default)]
pub struct MetricsCollector {
    counters: Arc<RwLock<HashMap<String, AtomicU64>>>,
    gauges: Arc<RwLock<HashMap<String, AtomicU64>>>,
}

impl MetricsCollector {
    /// Creates a new metrics collector
    pub fn new() -> Self {
        Self {
            counters: Arc::new(RwLock::new(HashMap::new())),
            gauges: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Increments a named counter by 1
    pub fn increment_counter(&self, name: &str) {
        if let Some(counter) = self.counters.read().unwrap().get(name) {
            counter.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Sets a gauge to a specific value
    pub fn set_gauge(&self, name: &str, value: u64) {
        if let Some(gauge) = self.gauges.read().unwrap().get(name) {
            gauge.store(value, Ordering::Relaxed);
        }
    }

    /// Gets the current value of a counter
    pub fn get_counter(&self, name: &str) -> u64 {
        self.counters
            .read()
            .unwrap()
            .get(name)
            .map(|c| c.load(Ordering::Relaxed))
            .unwrap_or(0)
    }

    /// Gets the current value of a gauge
    pub fn get_gauge(&self, name: &str) -> u64 {
        self.gauges
            .read()
            .unwrap()
            .get(name)
            .map(|g| g.load(Ordering::Relaxed))
            .unwrap_or(0)
    }
}

/// System-wide performance metrics snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemMetrics {
    /// System uptime in seconds
    pub uptime_seconds: u64,
    /// Total number of events processed
    pub total_events_processed: u64,
    /// Current events processing rate per second
    pub events_per_second: f64,
    /// Memory usage in megabytes
    pub memory_usage_mb: u64,
    /// CPU usage as a percentage
    pub cpu_usage_percent: f64,
    /// Number of active connections
    pub active_connections: u64,
}

impl SystemMetrics {
    /// Creates a new system metrics snapshot with default values
    pub fn new() -> Self {
        Self {
            uptime_seconds: 0,
            total_events_processed: 0,
            events_per_second: 0.0,
            memory_usage_mb: 0,
            cpu_usage_percent: 0.0,
            active_connections: 0,
        }
    }
}

impl Default for SystemMetrics {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_collector() {
        let collector = MetricsCollector::new();

        // Test counter operations
        collector.increment_counter("test_counter");
        assert_eq!(collector.get_counter("test_counter"), 0); // No counter registered

        // Test gauge operations
        collector.set_gauge("test_gauge", 42);
        assert_eq!(collector.get_gauge("test_gauge"), 0); // No gauge registered

        // Test non-existent metrics
        assert_eq!(collector.get_counter("non_existent"), 0);
        assert_eq!(collector.get_gauge("non_existent"), 0);
    }

    #[test]
    fn test_system_metrics() {
        let metrics = SystemMetrics::new();
        assert_eq!(metrics.uptime_seconds, 0);
        assert_eq!(metrics.total_events_processed, 0);
        assert_eq!(metrics.events_per_second, 0.0);
    }
}
