//! MCP Resources for orchestrator state access.
//!
//! Resources provide read-only access to orchestrator state, allowing AI agents
//! to query cluster information without executing actions.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Resource URI scheme: orchestrator://
///
/// Examples:
/// - orchestrator://nodes - List of all nodes
/// - orchestrator://nodes/{id} - Specific node details
/// - orchestrator://workloads - List of all workloads
/// - orchestrator://workloads/{id} - Specific workload details
/// - orchestrator://cluster/status - Cluster status summary
/// - orchestrator://cluster/metrics - Cluster metrics

/// Node resource representation
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct NodeResource {
    /// Resource URI
    pub uri: String,
    /// Node ID
    pub id: String,
    /// Node address
    pub address: String,
    /// Status
    pub status: String,
    /// Labels
    pub labels: HashMap<String, String>,
    /// CPU capacity
    pub cpu_capacity: f32,
    /// Memory capacity MB
    pub memory_capacity_mb: u64,
    /// CPU available
    pub cpu_available: f32,
    /// Memory available MB
    pub memory_available_mb: u64,
}

/// Workload resource representation
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WorkloadResource {
    /// Resource URI
    pub uri: String,
    /// Workload ID
    pub id: String,
    /// Workload name
    pub name: String,
    /// Desired replicas
    pub replicas: u32,
    /// Running instances
    pub running_instances: u32,
    /// Image
    pub image: String,
    /// Status
    pub status: String,
}

/// Cluster status resource
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ClusterStatusResource {
    /// Resource URI
    pub uri: String,
    /// Total nodes
    pub total_nodes: usize,
    /// Ready nodes
    pub ready_nodes: usize,
    /// Total workloads
    pub total_workloads: usize,
    /// Total running instances
    pub total_instances: usize,
    /// Cluster health
    pub health: String,
    /// Last updated timestamp
    pub last_updated: String,
}

/// Cluster metrics resource
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ClusterMetricsResource {
    /// Resource URI
    pub uri: String,
    /// CPU utilization percentage
    pub cpu_utilization_percent: f32,
    /// Memory utilization percentage
    pub memory_utilization_percent: f32,
    /// Total CPU cores
    pub total_cpu_cores: f32,
    /// Total memory MB
    pub total_memory_mb: u64,
    /// Used CPU cores
    pub used_cpu_cores: f32,
    /// Used memory MB
    pub used_memory_mb: u64,
    /// Instance count by status
    pub instances_by_status: HashMap<String, usize>,
}

/// Resource list response
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ResourceListResponse {
    /// List of resource URIs
    pub resources: Vec<ResourceInfo>,
}

/// Resource info for listing
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ResourceInfo {
    /// Resource URI
    pub uri: String,
    /// Resource name
    pub name: String,
    /// Resource description
    pub description: String,
    /// MIME type
    pub mime_type: String,
}

/// All available resources
pub fn list_available_resources() -> Vec<ResourceInfo> {
    vec![
        ResourceInfo {
            uri: "orchestrator://nodes".to_string(),
            name: "Nodes".to_string(),
            description: "List of all cluster nodes".to_string(),
            mime_type: "application/json".to_string(),
        },
        ResourceInfo {
            uri: "orchestrator://workloads".to_string(),
            name: "Workloads".to_string(),
            description: "List of all workloads".to_string(),
            mime_type: "application/json".to_string(),
        },
        ResourceInfo {
            uri: "orchestrator://cluster/status".to_string(),
            name: "Cluster Status".to_string(),
            description: "Current cluster status summary".to_string(),
            mime_type: "application/json".to_string(),
        },
        ResourceInfo {
            uri: "orchestrator://cluster/metrics".to_string(),
            name: "Cluster Metrics".to_string(),
            description: "Cluster resource metrics".to_string(),
            mime_type: "application/json".to_string(),
        },
    ]
}

/// Parse a resource URI into its components
pub fn parse_resource_uri(uri: &str) -> Option<ResourcePath> {
    let stripped = uri.strip_prefix("orchestrator://")?;
    let parts: Vec<&str> = stripped.split('/').collect();

    match parts.as_slice() {
        ["nodes"] => Some(ResourcePath::NodeList),
        ["nodes", id] => Some(ResourcePath::Node(id.to_string())),
        ["workloads"] => Some(ResourcePath::WorkloadList),
        ["workloads", id] => Some(ResourcePath::Workload(id.to_string())),
        ["cluster", "status"] => Some(ResourcePath::ClusterStatus),
        ["cluster", "metrics"] => Some(ResourcePath::ClusterMetrics),
        _ => None,
    }
}

/// Parsed resource path
#[derive(Debug, Clone)]
pub enum ResourcePath {
    NodeList,
    Node(String),
    WorkloadList,
    Workload(String),
    ClusterStatus,
    ClusterMetrics,
}
