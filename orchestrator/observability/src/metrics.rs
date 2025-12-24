//! Prometheus-compatible metrics for the orchestrator.
//!
//! Provides counters, gauges, and histograms for monitoring cluster health,
//! workload status, and operational performance.

use metrics::{counter, gauge, histogram, describe_counter, describe_gauge, describe_histogram, Label};
use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Registry for orchestrator metrics with Prometheus exporter.
pub struct MetricsRegistry {
    handle: PrometheusHandle,
}

impl MetricsRegistry {
    /// Create a new metrics registry with Prometheus exporter.
    pub fn new() -> Self {
        let handle = PrometheusBuilder::new()
            .install_recorder()
            .expect("Failed to install Prometheus recorder");

        // Register metric descriptions
        Self::register_descriptions();

        Self { handle }
    }

    /// Register all metric descriptions.
    fn register_descriptions() {
        // Node metrics
        describe_gauge!(
            "orchestrator_nodes_total",
            "Total number of nodes in the cluster"
        );
        describe_gauge!(
            "orchestrator_nodes_ready",
            "Number of ready nodes in the cluster"
        );
        describe_gauge!(
            "orchestrator_node_cpu_capacity",
            "CPU capacity of a node in cores"
        );
        describe_gauge!(
            "orchestrator_node_memory_capacity_bytes",
            "Memory capacity of a node in bytes"
        );
        describe_gauge!(
            "orchestrator_node_cpu_allocatable",
            "Allocatable CPU on a node in cores"
        );
        describe_gauge!(
            "orchestrator_node_memory_allocatable_bytes",
            "Allocatable memory on a node in bytes"
        );

        // Workload metrics
        describe_gauge!(
            "orchestrator_workloads_total",
            "Total number of workloads"
        );
        describe_gauge!(
            "orchestrator_workload_replicas_desired",
            "Desired replicas for a workload"
        );
        describe_gauge!(
            "orchestrator_workload_replicas_running",
            "Running replicas for a workload"
        );
        describe_gauge!(
            "orchestrator_instances_total",
            "Total number of workload instances"
        );
        describe_gauge!(
            "orchestrator_instances_by_status",
            "Number of instances by status"
        );

        // Scheduling metrics
        describe_counter!(
            "orchestrator_scheduling_attempts_total",
            "Total number of scheduling attempts"
        );
        describe_counter!(
            "orchestrator_scheduling_successes_total",
            "Total number of successful scheduling operations"
        );
        describe_counter!(
            "orchestrator_scheduling_failures_total",
            "Total number of failed scheduling operations"
        );
        describe_histogram!(
            "orchestrator_scheduling_duration_seconds",
            "Time taken for scheduling decisions"
        );

        // Reconciliation metrics
        describe_counter!(
            "orchestrator_reconciliation_runs_total",
            "Total number of reconciliation runs"
        );
        describe_histogram!(
            "orchestrator_reconciliation_duration_seconds",
            "Time taken for reconciliation"
        );
        describe_gauge!(
            "orchestrator_reconciliation_queue_depth",
            "Number of items in reconciliation queue"
        );

        // Container runtime metrics
        describe_counter!(
            "orchestrator_container_creates_total",
            "Total number of container create operations"
        );
        describe_counter!(
            "orchestrator_container_starts_total",
            "Total number of container start operations"
        );
        describe_counter!(
            "orchestrator_container_stops_total",
            "Total number of container stop operations"
        );
        describe_counter!(
            "orchestrator_container_errors_total",
            "Total number of container operation errors"
        );
        describe_histogram!(
            "orchestrator_container_operation_duration_seconds",
            "Time taken for container operations"
        );

        // Cluster manager metrics
        describe_counter!(
            "orchestrator_cluster_events_total",
            "Total number of cluster events"
        );
        describe_gauge!(
            "orchestrator_cluster_health",
            "Cluster health status (1=healthy, 0=unhealthy)"
        );

        // State store metrics
        describe_counter!(
            "orchestrator_state_operations_total",
            "Total number of state store operations"
        );
        describe_histogram!(
            "orchestrator_state_operation_duration_seconds",
            "Time taken for state store operations"
        );

        // MCP server metrics
        describe_counter!(
            "orchestrator_mcp_requests_total",
            "Total number of MCP requests"
        );
        describe_counter!(
            "orchestrator_mcp_errors_total",
            "Total number of MCP errors"
        );
        describe_histogram!(
            "orchestrator_mcp_request_duration_seconds",
            "Time taken for MCP requests"
        );
    }

    /// Render metrics in Prometheus text format.
    pub fn render(&self) -> String {
        self.handle.render()
    }
}

impl Default for MetricsRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// High-level interface for recording orchestrator metrics.
#[derive(Clone)]
pub struct OrchestratorMetrics {
    inner: Arc<RwLock<MetricsInner>>,
}

struct MetricsInner {
    // Cached values for aggregate metrics
    nodes_total: u64,
    nodes_ready: u64,
    workloads_total: u64,
    instances_total: u64,
}

impl OrchestratorMetrics {
    /// Create a new metrics interface.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(MetricsInner {
                nodes_total: 0,
                nodes_ready: 0,
                workloads_total: 0,
                instances_total: 0,
            })),
        }
    }

    // === Node Metrics ===

    /// Record total node count.
    pub async fn set_nodes_total(&self, count: u64) {
        gauge!("orchestrator_nodes_total").set(count as f64);
        self.inner.write().await.nodes_total = count;
    }

    /// Record ready node count.
    pub async fn set_nodes_ready(&self, count: u64) {
        gauge!("orchestrator_nodes_ready").set(count as f64);
        self.inner.write().await.nodes_ready = count;
    }

    /// Record node resource capacity.
    pub fn set_node_capacity(&self, node_id: &str, cpu_cores: f64, memory_bytes: u64) {
        let labels = [("node_id", node_id.to_string())];
        gauge!("orchestrator_node_cpu_capacity", &labels).set(cpu_cores);
        gauge!("orchestrator_node_memory_capacity_bytes", &labels).set(memory_bytes as f64);
    }

    /// Record node allocatable resources.
    pub fn set_node_allocatable(&self, node_id: &str, cpu_cores: f64, memory_bytes: u64) {
        let labels = [("node_id", node_id.to_string())];
        gauge!("orchestrator_node_cpu_allocatable", &labels).set(cpu_cores);
        gauge!("orchestrator_node_memory_allocatable_bytes", &labels).set(memory_bytes as f64);
    }

    // === Workload Metrics ===

    /// Record total workload count.
    pub async fn set_workloads_total(&self, count: u64) {
        gauge!("orchestrator_workloads_total").set(count as f64);
        self.inner.write().await.workloads_total = count;
    }

    /// Record workload replica counts.
    pub fn set_workload_replicas(&self, workload_id: &str, desired: u32, running: u32) {
        let labels = [("workload_id", workload_id.to_string())];
        gauge!("orchestrator_workload_replicas_desired", &labels).set(desired as f64);
        gauge!("orchestrator_workload_replicas_running", &labels).set(running as f64);
    }

    /// Record total instance count.
    pub async fn set_instances_total(&self, count: u64) {
        gauge!("orchestrator_instances_total").set(count as f64);
        self.inner.write().await.instances_total = count;
    }

    /// Record instance count by status.
    pub fn set_instances_by_status(&self, status: &str, count: u64) {
        let labels = [("status", status.to_string())];
        gauge!("orchestrator_instances_by_status", &labels).set(count as f64);
    }

    // === Scheduling Metrics ===

    /// Record a scheduling attempt.
    pub fn inc_scheduling_attempts(&self) {
        counter!("orchestrator_scheduling_attempts_total").increment(1);
    }

    /// Record a successful scheduling operation.
    pub fn inc_scheduling_successes(&self) {
        counter!("orchestrator_scheduling_successes_total").increment(1);
    }

    /// Record a failed scheduling operation.
    pub fn inc_scheduling_failures(&self, reason: &str) {
        let labels = [("reason", reason.to_string())];
        counter!("orchestrator_scheduling_failures_total", &labels).increment(1);
    }

    /// Record scheduling duration.
    pub fn record_scheduling_duration(&self, duration_secs: f64) {
        histogram!("orchestrator_scheduling_duration_seconds").record(duration_secs);
    }

    // === Reconciliation Metrics ===

    /// Record a reconciliation run.
    pub fn inc_reconciliation_runs(&self) {
        counter!("orchestrator_reconciliation_runs_total").increment(1);
    }

    /// Record reconciliation duration.
    pub fn record_reconciliation_duration(&self, duration_secs: f64) {
        histogram!("orchestrator_reconciliation_duration_seconds").record(duration_secs);
    }

    /// Set reconciliation queue depth.
    pub fn set_reconciliation_queue_depth(&self, depth: u64) {
        gauge!("orchestrator_reconciliation_queue_depth").set(depth as f64);
    }

    // === Container Runtime Metrics ===

    /// Record a container create operation.
    pub fn inc_container_creates(&self, success: bool) {
        counter!("orchestrator_container_creates_total").increment(1);
        if !success {
            counter!("orchestrator_container_errors_total", "operation" => "create").increment(1);
        }
    }

    /// Record a container start operation.
    pub fn inc_container_starts(&self, success: bool) {
        counter!("orchestrator_container_starts_total").increment(1);
        if !success {
            counter!("orchestrator_container_errors_total", "operation" => "start").increment(1);
        }
    }

    /// Record a container stop operation.
    pub fn inc_container_stops(&self, success: bool) {
        counter!("orchestrator_container_stops_total").increment(1);
        if !success {
            counter!("orchestrator_container_errors_total", "operation" => "stop").increment(1);
        }
    }

    /// Record container operation duration.
    pub fn record_container_operation_duration(&self, operation: &str, duration_secs: f64) {
        let labels = [("operation", operation.to_string())];
        histogram!("orchestrator_container_operation_duration_seconds", &labels).record(duration_secs);
    }

    // === Cluster Manager Metrics ===

    /// Record a cluster event.
    pub fn inc_cluster_events(&self, event_type: &str) {
        let labels = [("event_type", event_type.to_string())];
        counter!("orchestrator_cluster_events_total", &labels).increment(1);
    }

    /// Set cluster health status.
    pub fn set_cluster_health(&self, healthy: bool) {
        gauge!("orchestrator_cluster_health").set(if healthy { 1.0 } else { 0.0 });
    }

    // === State Store Metrics ===

    /// Record a state store operation.
    pub fn inc_state_operations(&self, operation: &str, success: bool) {
        let labels = [
            ("operation", operation.to_string()),
            ("success", success.to_string()),
        ];
        counter!("orchestrator_state_operations_total", &labels).increment(1);
    }

    /// Record state store operation duration.
    pub fn record_state_operation_duration(&self, operation: &str, duration_secs: f64) {
        let labels = [("operation", operation.to_string())];
        histogram!("orchestrator_state_operation_duration_seconds", &labels).record(duration_secs);
    }

    // === MCP Server Metrics ===

    /// Record an MCP request.
    pub fn inc_mcp_requests(&self, tool: &str) {
        let labels = [("tool", tool.to_string())];
        counter!("orchestrator_mcp_requests_total", &labels).increment(1);
    }

    /// Record an MCP error.
    pub fn inc_mcp_errors(&self, tool: &str, error_type: &str) {
        let labels = [
            ("tool", tool.to_string()),
            ("error_type", error_type.to_string()),
        ];
        counter!("orchestrator_mcp_errors_total", &labels).increment(1);
    }

    /// Record MCP request duration.
    pub fn record_mcp_request_duration(&self, tool: &str, duration_secs: f64) {
        let labels = [("tool", tool.to_string())];
        histogram!("orchestrator_mcp_request_duration_seconds", &labels).record(duration_secs);
    }
}

impl Default for OrchestratorMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// Timer guard for automatically recording operation durations.
pub struct MetricTimer {
    start: std::time::Instant,
    histogram_name: &'static str,
    labels: Vec<(&'static str, String)>,
}

impl MetricTimer {
    /// Create a new timer for the given histogram.
    pub fn new(histogram_name: &'static str) -> Self {
        Self {
            start: std::time::Instant::now(),
            histogram_name,
            labels: vec![],
        }
    }

    /// Add a label to the timer.
    pub fn with_label(mut self, key: &'static str, value: impl Into<String>) -> Self {
        self.labels.push((key, value.into()));
        self
    }
}

impl Drop for MetricTimer {
    fn drop(&mut self) {
        let duration = self.start.elapsed().as_secs_f64();
        let labels: Vec<Label> = self.labels.iter()
            .map(|(k, v)| Label::new(*k, v.clone()))
            .collect();

        metrics::histogram!(self.histogram_name, labels).record(duration);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_registry_creation() {
        // Note: Can only create one registry per process
        // This test verifies the structure compiles correctly
    }

    #[tokio::test]
    async fn test_orchestrator_metrics() {
        let metrics = OrchestratorMetrics::new();

        // Test node metrics
        metrics.set_nodes_total(5).await;
        metrics.set_nodes_ready(4).await;
        metrics.set_node_capacity("node-1", 8.0, 16_000_000_000);

        // Test workload metrics
        metrics.set_workloads_total(10).await;
        metrics.set_workload_replicas("workload-1", 3, 3);

        // Test counter increments
        metrics.inc_scheduling_attempts();
        metrics.inc_scheduling_successes();
        metrics.inc_reconciliation_runs();

        // Test histogram recordings
        metrics.record_scheduling_duration(0.05);
        metrics.record_reconciliation_duration(0.1);
    }

    #[test]
    fn test_metric_timer() {
        let _timer = MetricTimer::new("test_duration_seconds")
            .with_label("operation", "test");
        // Timer records duration on drop
    }
}
