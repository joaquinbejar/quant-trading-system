use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::time::{Duration, Instant};

/// Metrics exporter for external monitoring systems
#[derive(Debug, Clone)]
pub struct MetricsExporter {
    start_time: Instant,
    export_interval: Duration,
}

impl MetricsExporter {
    /// Creates a new metrics exporter with specified export interval
    pub fn new(export_interval: Duration) -> Self {
        Self {
            start_time: Instant::now(),
            export_interval,
        }
    }

    /// Returns the uptime since exporter creation
    pub fn uptime(&self) -> Duration {
        self.start_time.elapsed()
    }

    /// Returns the configured export interval
    pub fn export_interval(&self) -> Duration {
        self.export_interval
    }

    /// Export metrics in Prometheus format
    pub fn export_prometheus(&self, metrics: &HashMap<String, f64>) -> String {
        let mut output = String::new();

        for (name, value) in metrics {
            output.push_str(&format!("# TYPE {} gauge\n", name));
            output.push_str(&format!("{} {}\n", name, value));
        }

        output
    }

    /// Export metrics in JSON format
    pub fn export_json(&self, metrics: &HashMap<String, f64>) -> Result<String, serde_json::Error> {
        let export_data = MetricsExport {
            timestamp: chrono::Utc::now(),
            uptime_seconds: self.uptime().as_secs(),
            metrics: metrics.clone(),
        };

        serde_json::to_string_pretty(&export_data)
    }
}

impl Default for MetricsExporter {
    fn default() -> Self {
        Self::new(Duration::from_secs(60))
    }
}

/// Metrics export data structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsExport {
    /// Timestamp when metrics were exported
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// System uptime in seconds
    pub uptime_seconds: u64,
    /// Map of metric names to values
    pub metrics: HashMap<String, f64>,
}

/// Health status information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthStatus {
    /// Overall system status
    pub status: String,
    /// System version
    pub version: String,
    /// System uptime in seconds
    pub uptime_seconds: u64,
    /// Timestamp of health check
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Health status of individual components
    pub components: HashMap<String, ComponentHealth>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Health status of an individual system component
pub struct ComponentHealth {
    /// Component status (healthy/unhealthy)
    pub status: String,
    /// Timestamp of last health check
    pub last_check: chrono::DateTime<chrono::Utc>,
    /// Optional details about component status
    pub details: Option<String>,
}

impl HealthStatus {
    /// Creates a new health status with the given version and uptime
    pub fn new(version: &str, uptime: Duration) -> Self {
        Self {
            status: "healthy".to_string(),
            version: version.to_string(),
            uptime_seconds: uptime.as_secs(),
            timestamp: chrono::Utc::now(),
            components: HashMap::new(),
        }
    }

    /// Adds a component health status
    pub fn add_component(&mut self, name: String, health: ComponentHealth) {
        self.components.insert(name, health);
    }

    /// Returns true if all components are healthy
    pub fn is_healthy(&self) -> bool {
        self.status == "healthy" && self.components.values().all(|c| c.status == "healthy")
    }
}

impl ComponentHealth {
    /// Creates a healthy component status
    pub fn healthy() -> Self {
        Self {
            status: "healthy".to_string(),
            last_check: chrono::Utc::now(),
            details: None,
        }
    }

    /// Creates an unhealthy component status with details
    pub fn unhealthy(details: String) -> Self {
        Self {
            status: "unhealthy".to_string(),
            last_check: chrono::Utc::now(),
            details: Some(details),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_metrics_exporter() {
        let exporter = MetricsExporter::new(Duration::from_secs(30));
        assert_eq!(exporter.export_interval(), Duration::from_secs(30));
        // Just verify uptime is available
        let _uptime = exporter.uptime();
    }

    #[test]
    fn test_prometheus_export() {
        let exporter = MetricsExporter::default();
        let mut metrics = HashMap::new();
        metrics.insert("test_metric".to_string(), 42.0);
        metrics.insert("another_metric".to_string(), std::f64::consts::PI);

        let prometheus_output = exporter.export_prometheus(&metrics);
        assert!(prometheus_output.contains("# TYPE test_metric gauge"));
        assert!(prometheus_output.contains("test_metric 42"));
        assert!(prometheus_output.contains("another_metric 3.14"));
    }

    #[test]
    fn test_json_export() {
        let exporter = MetricsExporter::default();
        let mut metrics = HashMap::new();
        metrics.insert("test_metric".to_string(), 42.0);

        let json_output = exporter.export_json(&metrics).unwrap();
        assert!(json_output.contains("test_metric"));
        assert!(json_output.contains("42"));
        assert!(json_output.contains("timestamp"));
        assert!(json_output.contains("uptime_seconds"));
    }

    #[test]
    fn test_health_status() {
        let mut health = HealthStatus::new("1.0.0", Duration::from_secs(3600));
        assert_eq!(health.status, "healthy");
        assert_eq!(health.version, "1.0.0");
        assert_eq!(health.uptime_seconds, 3600);
        assert!(health.is_healthy());

        health.add_component("orderbook".to_string(), ComponentHealth::healthy());
        assert!(health.is_healthy());

        health.add_component(
            "amm".to_string(),
            ComponentHealth::unhealthy("Connection lost".to_string()),
        );
        assert!(!health.is_healthy());
    }

    #[test]
    fn test_component_health() {
        let healthy = ComponentHealth::healthy();
        assert_eq!(healthy.status, "healthy");
        assert!(healthy.details.is_none());

        let unhealthy = ComponentHealth::unhealthy("Database connection failed".to_string());
        assert_eq!(unhealthy.status, "unhealthy");
        assert_eq!(
            unhealthy.details,
            Some("Database connection failed".to_string())
        );
    }
}
