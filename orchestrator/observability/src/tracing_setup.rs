//! Tracing configuration and initialization.
//!
//! Provides structured logging with span-based context propagation.

use tracing::Level;
use tracing_subscriber::{
    fmt::{self, format::FmtSpan},
    layer::SubscriberExt,
    util::SubscriberInitExt,
    EnvFilter,
};

/// Configuration for tracing initialization.
#[derive(Debug, Clone)]
pub struct TracingConfig {
    /// Service name for identification
    pub service_name: String,
    /// Log level (trace, debug, info, warn, error)
    pub log_level: Level,
    /// Whether to include span events (enter, exit, close)
    pub include_span_events: bool,
    /// Whether to output in JSON format
    pub json_output: bool,
    /// Whether to include file and line numbers
    pub include_location: bool,
    /// Whether to include thread IDs
    pub include_thread_ids: bool,
    /// Whether to include target (module path)
    pub include_target: bool,
}

impl Default for TracingConfig {
    fn default() -> Self {
        Self {
            service_name: "orchestrator".to_string(),
            log_level: Level::INFO,
            include_span_events: false,
            json_output: false,
            include_location: true,
            include_thread_ids: false,
            include_target: true,
        }
    }
}

impl TracingConfig {
    /// Create a new config with the given service name.
    pub fn new(service_name: impl Into<String>) -> Self {
        Self {
            service_name: service_name.into(),
            ..Default::default()
        }
    }

    /// Set the log level.
    pub fn with_level(mut self, level: Level) -> Self {
        self.log_level = level;
        self
    }

    /// Enable JSON output format.
    pub fn with_json(mut self, json: bool) -> Self {
        self.json_output = json;
        self
    }

    /// Include span events in output.
    pub fn with_span_events(mut self, include: bool) -> Self {
        self.include_span_events = include;
        self
    }

    /// Build an EnvFilter from this config.
    fn build_filter(&self) -> EnvFilter {
        EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new(format!("{}", self.log_level)))
    }
}

/// Initialize tracing with the given configuration.
///
/// This should be called once at application startup.
///
/// # Example
///
/// ```no_run
/// use observability::{init_tracing, TracingConfig};
/// use tracing::Level;
///
/// init_tracing(TracingConfig::new("my-service").with_level(Level::DEBUG));
/// ```
pub fn init_tracing(config: TracingConfig) {
    let filter = config.build_filter();

    let span_events = if config.include_span_events {
        FmtSpan::NEW | FmtSpan::CLOSE
    } else {
        FmtSpan::NONE
    };

    if config.json_output {
        // JSON format for production/log aggregation
        let fmt_layer = fmt::layer()
            .json()
            .with_span_events(span_events)
            .with_file(config.include_location)
            .with_line_number(config.include_location)
            .with_thread_ids(config.include_thread_ids)
            .with_target(config.include_target);

        tracing_subscriber::registry()
            .with(filter)
            .with(fmt_layer)
            .init();
    } else {
        // Human-readable format for development
        let fmt_layer = fmt::layer()
            .with_span_events(span_events)
            .with_file(config.include_location)
            .with_line_number(config.include_location)
            .with_thread_ids(config.include_thread_ids)
            .with_target(config.include_target);

        tracing_subscriber::registry()
            .with(filter)
            .with(fmt_layer)
            .init();
    }

    tracing::info!(
        service = %config.service_name,
        level = %config.log_level,
        "Tracing initialized"
    );
}

/// Initialize tracing with default configuration.
pub fn init_default_tracing() {
    init_tracing(TracingConfig::default());
}

/// Create a span for an operation with standard fields.
#[macro_export]
macro_rules! operation_span {
    ($name:expr) => {
        tracing::info_span!($name)
    };
    ($name:expr, $($field:tt)*) => {
        tracing::info_span!($name, $($field)*)
    };
}

/// Create a span for workload operations.
#[macro_export]
macro_rules! workload_span {
    ($op:expr, $workload_id:expr) => {
        tracing::info_span!(
            "workload_operation",
            operation = $op,
            workload_id = %$workload_id
        )
    };
}

/// Create a span for node operations.
#[macro_export]
macro_rules! node_span {
    ($op:expr, $node_id:expr) => {
        tracing::info_span!(
            "node_operation",
            operation = $op,
            node_id = %$node_id
        )
    };
}

/// Create a span for scheduling operations.
#[macro_export]
macro_rules! scheduler_span {
    ($op:expr) => {
        tracing::info_span!("scheduler_operation", operation = $op)
    };
    ($op:expr, $workload_id:expr) => {
        tracing::info_span!(
            "scheduler_operation",
            operation = $op,
            workload_id = %$workload_id
        )
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tracing_config_default() {
        let config = TracingConfig::default();
        assert_eq!(config.service_name, "orchestrator");
        assert_eq!(config.log_level, Level::INFO);
        assert!(!config.json_output);
    }

    #[test]
    fn test_tracing_config_builder() {
        let config = TracingConfig::new("test-service")
            .with_level(Level::DEBUG)
            .with_json(true)
            .with_span_events(true);

        assert_eq!(config.service_name, "test-service");
        assert_eq!(config.log_level, Level::DEBUG);
        assert!(config.json_output);
        assert!(config.include_span_events);
    }

    #[test]
    fn test_env_filter_building() {
        let config = TracingConfig::default();
        let filter = config.build_filter();
        // Filter should be created successfully
        assert!(format!("{:?}", filter).contains("EnvFilter"));
    }
}
