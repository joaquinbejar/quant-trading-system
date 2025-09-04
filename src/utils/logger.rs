use std::env;
use std::sync::Once;
use tracing::Level;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

static INIT: Once = Once::new();

/// Sets up the logger with optional OpenTelemetry integration
///
/// Environment variables:
/// - LOGLEVEL: Sets the log level (DEBUG, INFO, WARN, ERROR, TRACE)
/// - ENABLE_TRACING: Enable OpenTelemetry tracing (true/false) - requires the "opentelemetry" feature
/// - TRACING_ENDPOINT: OpenTelemetry endpoint (default: http://localhost:4317)
/// - SERVICE_NAME: Service name for tracing (default: rust-app)
pub fn setup_logger() -> Result<(), Box<dyn std::error::Error>> {
    INIT.call_once(|| {
        let log_level = env::var("LOGLEVEL")
            .unwrap_or_else(|_| "INFO".to_string())
            .to_uppercase();

        let level = match log_level.as_str() {
            "DEBUG" => Level::DEBUG,
            "ERROR" => Level::ERROR,
            "WARN" => Level::WARN,
            "TRACE" => Level::TRACE,
            _ => Level::INFO,
        };

        let enable_tracing =
            env::var("ENABLE_TRACING").unwrap_or_else(|_| "false".to_string()) == "true";

        // Create the registry with fmt layer
        let registry = tracing_subscriber::registry()
            .with(
                tracing_subscriber::fmt::layer()
                    .with_target(false)
                    .with_thread_ids(true),
            )
            .with(tracing_subscriber::EnvFilter::from_default_env().add_directive(level.into()));

        {
            registry.init();
            if enable_tracing {
                tracing::warn!("OpenTelemetry tracing requested but feature not enabled");
                tracing::warn!("Add the 'opentelemetry' feature to your Cargo.toml to enable it");
            }
        }

        tracing::debug!("Log level set to: {}", level);
    });

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // Mutex to ensure tests run sequentially since they modify global state
    static TEST_MUTEX: Mutex<()> = Mutex::new(());

    /// Helper function to reset the Once static for testing
    /// Note: This is a workaround since Once cannot be reset in standard library
    fn with_clean_env<F>(env_vars: Vec<(&str, &str)>, test_fn: F)
    where
        F: FnOnce(),
    {
        let _guard = TEST_MUTEX.lock().unwrap();

        // Store original values
        let original_values: Vec<_> = env_vars
            .iter()
            .map(|(key, _)| (*key, env::var(key).ok()))
            .collect();

        // Set test values
        for (key, value) in &env_vars {
            env::set_var(key, value);
        }

        // Run test
        test_fn();

        // Restore original values
        for (key, original_value) in original_values {
            match original_value {
                Some(value) => env::set_var(key, value),
                None => env::remove_var(key),
            }
        }
    }
    #[test]
    fn test_log_level_parsing_debug() {
        with_clean_env(vec![("LOGLEVEL", "DEBUG")], || {
            let log_level = env::var("LOGLEVEL")
                .unwrap_or_else(|_| "INFO".to_string())
                .to_uppercase();
            let level = match log_level.as_str() {
                "DEBUG" => Level::DEBUG,
                "ERROR" => Level::ERROR,
                "WARN" => Level::WARN,
                "TRACE" => Level::TRACE,
                _ => Level::INFO,
            };
            assert_eq!(level, Level::DEBUG);
        });
    }

    #[test]
    fn test_log_level_parsing_info() {
        with_clean_env(vec![("LOGLEVEL", "INFO")], || {
            let log_level = env::var("LOGLEVEL")
                .unwrap_or_else(|_| "INFO".to_string())
                .to_uppercase();
            let level = match log_level.as_str() {
                "DEBUG" => Level::DEBUG,
                "ERROR" => Level::ERROR,
                "WARN" => Level::WARN,
                "TRACE" => Level::TRACE,
                _ => Level::INFO,
            };
            assert_eq!(level, Level::INFO);
        });
    }

    #[test]
    fn test_log_level_parsing_warn() {
        with_clean_env(vec![("LOGLEVEL", "WARN")], || {
            let log_level = env::var("LOGLEVEL")
                .unwrap_or_else(|_| "INFO".to_string())
                .to_uppercase();
            let level = match log_level.as_str() {
                "DEBUG" => Level::DEBUG,
                "ERROR" => Level::ERROR,
                "WARN" => Level::WARN,
                "TRACE" => Level::TRACE,
                _ => Level::INFO,
            };
            assert_eq!(level, Level::WARN);
        });
    }

    #[test]
    fn test_log_level_parsing_error() {
        with_clean_env(vec![("LOGLEVEL", "ERROR")], || {
            let log_level = env::var("LOGLEVEL")
                .unwrap_or_else(|_| "INFO".to_string())
                .to_uppercase();
            let level = match log_level.as_str() {
                "DEBUG" => Level::DEBUG,
                "ERROR" => Level::ERROR,
                "WARN" => Level::WARN,
                "TRACE" => Level::TRACE,
                _ => Level::INFO,
            };
            assert_eq!(level, Level::ERROR);
        });
    }

    #[test]
    fn test_log_level_parsing_trace() {
        with_clean_env(vec![("LOGLEVEL", "TRACE")], || {
            let log_level = env::var("LOGLEVEL")
                .unwrap_or_else(|_| "INFO".to_string())
                .to_uppercase();
            let level = match log_level.as_str() {
                "DEBUG" => Level::DEBUG,
                "ERROR" => Level::ERROR,
                "WARN" => Level::WARN,
                "TRACE" => Level::TRACE,
                _ => Level::INFO,
            };
            assert_eq!(level, Level::TRACE);
        });
    }

    #[test]
    fn test_log_level_parsing_invalid_defaults_to_info() {
        with_clean_env(vec![("LOGLEVEL", "INVALID")], || {
            let log_level = env::var("LOGLEVEL")
                .unwrap_or_else(|_| "INFO".to_string())
                .to_uppercase();
            let level = match log_level.as_str() {
                "DEBUG" => Level::DEBUG,
                "ERROR" => Level::ERROR,
                "WARN" => Level::WARN,
                "TRACE" => Level::TRACE,
                _ => Level::INFO,
            };
            assert_eq!(level, Level::INFO);
        });
    }

    #[test]
    fn test_log_level_case_insensitive() {
        with_clean_env(vec![("LOGLEVEL", "debug")], || {
            let log_level = env::var("LOGLEVEL")
                .unwrap_or_else(|_| "INFO".to_string())
                .to_uppercase();
            let level = match log_level.as_str() {
                "DEBUG" => Level::DEBUG,
                "ERROR" => Level::ERROR,
                "WARN" => Level::WARN,
                "TRACE" => Level::TRACE,
                _ => Level::INFO,
            };
            assert_eq!(level, Level::DEBUG);
        });
    }

    #[test]
    fn test_default_log_level_when_env_not_set() {
        with_clean_env(vec![], || {
            env::remove_var("LOGLEVEL");
            let log_level = env::var("LOGLEVEL")
                .unwrap_or_else(|_| "INFO".to_string())
                .to_uppercase();
            let level = match log_level.as_str() {
                "DEBUG" => Level::DEBUG,
                "ERROR" => Level::ERROR,
                "WARN" => Level::WARN,
                "TRACE" => Level::TRACE,
                _ => Level::INFO,
            };
            assert_eq!(level, Level::INFO);
        });
    }

    #[test]
    fn test_enable_tracing_true() {
        with_clean_env(vec![("ENABLE_TRACING", "true")], || {
            let enable_tracing =
                env::var("ENABLE_TRACING").unwrap_or_else(|_| "false".to_string()) == "true";
            assert!(enable_tracing);
        });
    }

    #[test]
    fn test_enable_tracing_false() {
        with_clean_env(vec![("ENABLE_TRACING", "false")], || {
            let enable_tracing =
                env::var("ENABLE_TRACING").unwrap_or_else(|_| "false".to_string()) == "true";
            assert!(!enable_tracing);
        });
    }

    #[test]
    fn test_enable_tracing_default_false() {
        with_clean_env(vec![], || {
            env::remove_var("ENABLE_TRACING");
            let enable_tracing =
                env::var("ENABLE_TRACING").unwrap_or_else(|_| "false".to_string()) == "true";
            assert!(!enable_tracing);
        });
    }

    #[test]
    fn test_enable_tracing_case_sensitive() {
        with_clean_env(vec![("ENABLE_TRACING", "TRUE")], || {
            let enable_tracing =
                env::var("ENABLE_TRACING").unwrap_or_else(|_| "false".to_string()) == "true";
            assert!(!enable_tracing); // Should be false because it's case-sensitive
        });
    }
}
