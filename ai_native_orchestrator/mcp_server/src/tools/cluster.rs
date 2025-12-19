//! Cluster-wide MCP tools.
//!
//! These tools allow AI agents to query and manage the cluster as a whole.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Input for getting cluster status
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct GetClusterStatusInput {
    /// Include detailed node information
    #[serde(default)]
    pub include_nodes: bool,
    /// Include workload summary
    #[serde(default)]
    pub include_workloads: bool,
}

/// Cluster status output
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GetClusterStatusOutput {
    /// Total number of nodes
    pub total_nodes: usize,
    /// Nodes by status
    pub nodes_by_status: HashMap<String, usize>,
    /// Total workloads
    pub total_workloads: usize,
    /// Total running instances
    pub total_instances: usize,
    /// Total CPU capacity
    pub total_cpu_capacity: f32,
    /// Total memory capacity in MB
    pub total_memory_capacity_mb: u64,
    /// Used CPU
    pub used_cpu: f32,
    /// Used memory in MB
    pub used_memory_mb: u64,
    /// Cluster health status
    pub health: String,
}

/// Input for getting cluster events
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GetClusterEventsInput {
    /// Maximum number of events to return
    #[serde(default = "default_event_limit")]
    pub limit: usize,
    /// Filter by event type (node_joined, node_left, workload_created, etc.)
    #[serde(default)]
    pub event_type: Option<String>,
    /// Filter by resource ID (node or workload ID)
    #[serde(default)]
    pub resource_id: Option<String>,
}

fn default_event_limit() -> usize {
    50
}

/// Cluster event
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ClusterEventOutput {
    /// Event type
    pub event_type: String,
    /// Timestamp (RFC3339)
    pub timestamp: String,
    /// Resource type (node, workload, instance)
    pub resource_type: String,
    /// Resource ID
    pub resource_id: String,
    /// Event message
    pub message: String,
    /// Additional details
    pub details: HashMap<String, String>,
}

/// Output from getting cluster events
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GetClusterEventsOutput {
    /// List of events
    pub events: Vec<ClusterEventOutput>,
    /// Total count (may be more than returned)
    pub total: usize,
}

/// Input for cluster-wide scheduling analysis
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AnalyzeSchedulingInput {
    /// Analyze specific workload (if not provided, analyzes all)
    #[serde(default)]
    pub workload_id: Option<String>,
    /// Include what-if analysis for potential replicas
    #[serde(default)]
    pub include_what_if: bool,
}

/// Scheduling analysis output
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AnalyzeSchedulingOutput {
    /// Current scheduling status
    pub current_status: SchedulingStatus,
    /// Recommendations
    pub recommendations: Vec<SchedulingRecommendation>,
    /// What-if analysis results
    pub what_if: Option<WhatIfAnalysis>,
}

/// Current scheduling status
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SchedulingStatus {
    /// Number of pending instances
    pub pending_instances: usize,
    /// Number of running instances
    pub running_instances: usize,
    /// Number of unschedulable instances (no suitable nodes)
    pub unschedulable_instances: usize,
    /// Resource utilization percentage
    pub resource_utilization_percent: f32,
}

/// Scheduling recommendation
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SchedulingRecommendation {
    /// Recommendation type
    pub recommendation_type: String,
    /// Priority (high, medium, low)
    pub priority: String,
    /// Description
    pub description: String,
    /// Affected resource IDs
    pub affected_resources: Vec<String>,
}

/// What-if analysis
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WhatIfAnalysis {
    /// Maximum additional replicas that could be scheduled
    pub max_additional_replicas: u32,
    /// Nodes with available capacity
    pub nodes_with_capacity: Vec<NodeCapacityInfo>,
}

/// Node capacity info for what-if analysis
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct NodeCapacityInfo {
    /// Node ID
    pub node_id: String,
    /// Available CPU
    pub available_cpu: f32,
    /// Available memory MB
    pub available_memory_mb: u64,
    /// Estimated additional instances this node could handle
    pub estimated_capacity: u32,
}

/// Input for getting resource quotas
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GetResourceQuotasInput {
    /// Namespace to get quotas for (if namespaces are supported)
    #[serde(default)]
    pub namespace: Option<String>,
}

/// Resource quota output
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GetResourceQuotasOutput {
    /// CPU quota (total allowed)
    pub cpu_quota: f32,
    /// CPU used
    pub cpu_used: f32,
    /// Memory quota in MB
    pub memory_quota_mb: u64,
    /// Memory used in MB
    pub memory_used_mb: u64,
    /// Maximum workloads allowed
    pub max_workloads: u32,
    /// Current workload count
    pub current_workloads: u32,
}
