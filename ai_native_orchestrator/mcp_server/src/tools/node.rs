//! Node management MCP tools.
//!
//! These tools allow AI agents to view, manage, and operate on cluster nodes.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use orchestrator_shared_types::Node;

/// Input for listing nodes
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ListNodesInput {
    /// Optional filter by status
    #[serde(default)]
    pub status_filter: Option<String>,
    /// Optional filter by label key
    #[serde(default)]
    pub label_key: Option<String>,
    /// Optional filter by label value (requires label_key)
    #[serde(default)]
    pub label_value: Option<String>,
}

/// Node summary for listing
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct NodeSummary {
    /// Node ID
    pub id: String,
    /// Node address
    pub address: String,
    /// Node status
    pub status: String,
    /// Labels
    pub labels: HashMap<String, String>,
    /// Total CPU cores
    pub cpu_capacity: f32,
    /// Total memory in MB
    pub memory_capacity_mb: u64,
    /// Available CPU cores
    pub cpu_allocatable: f32,
    /// Available memory in MB
    pub memory_allocatable_mb: u64,
}

/// Output from listing nodes
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ListNodesOutput {
    /// List of node summaries
    pub nodes: Vec<NodeSummary>,
    /// Total count
    pub total: usize,
    /// Count by status
    pub status_counts: HashMap<String, usize>,
}

/// Input for getting node details
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GetNodeInput {
    /// Node ID to get
    pub node_id: String,
}

/// Detailed node output
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GetNodeOutput {
    /// Node ID
    pub id: String,
    /// Node address
    pub address: String,
    /// Node status
    pub status: String,
    /// Labels
    pub labels: HashMap<String, String>,
    /// Resource capacity
    pub capacity: NodeResourcesOutput,
    /// Allocatable resources
    pub allocatable: NodeResourcesOutput,
    /// Running workload instance IDs on this node
    pub running_instances: Vec<String>,
}

/// Node resources output
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct NodeResourcesOutput {
    pub cpu_cores: f32,
    pub memory_mb: u64,
    pub disk_mb: u64,
}

/// Input for cordoning a node (preventing new workloads)
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CordonNodeInput {
    /// Node ID to cordon
    pub node_id: String,
}

/// Output from cordoning a node
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CordonNodeOutput {
    /// Node ID
    pub node_id: String,
    /// Previous status
    pub previous_status: String,
    /// New status
    pub new_status: String,
    /// Message
    pub message: String,
}

/// Input for uncordoning a node (allowing new workloads)
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct UncordonNodeInput {
    /// Node ID to uncordon
    pub node_id: String,
}

/// Output from uncordoning a node
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct UncordonNodeOutput {
    /// Node ID
    pub node_id: String,
    /// Previous status
    pub previous_status: String,
    /// New status
    pub new_status: String,
    /// Message
    pub message: String,
}

/// Input for draining a node (evicting all workloads)
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DrainNodeInput {
    /// Node ID to drain
    pub node_id: String,
    /// Grace period in seconds for workload termination
    #[serde(default = "default_grace_period")]
    pub grace_period_seconds: u32,
    /// Whether to force drain even if workloads can't be rescheduled
    #[serde(default)]
    pub force: bool,
}

fn default_grace_period() -> u32 {
    30
}

/// Output from draining a node
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DrainNodeOutput {
    /// Node ID
    pub node_id: String,
    /// Number of workload instances evicted
    pub evicted_instances: u32,
    /// Whether the drain is complete
    pub completed: bool,
    /// Message
    pub message: String,
}

/// Input for adding a label to a node
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LabelNodeInput {
    /// Node ID to label
    pub node_id: String,
    /// Label key
    pub key: String,
    /// Label value
    pub value: String,
}

/// Output from labeling a node
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LabelNodeOutput {
    /// Node ID
    pub node_id: String,
    /// Updated labels
    pub labels: HashMap<String, String>,
    /// Message
    pub message: String,
}

impl From<&Node> for NodeSummary {
    fn from(n: &Node) -> Self {
        Self {
            id: n.id.to_string(),
            address: n.address.clone(),
            status: format!("{:?}", n.status),
            labels: n.labels.clone(),
            cpu_capacity: n.resources_capacity.cpu_cores,
            memory_capacity_mb: n.resources_capacity.memory_mb,
            cpu_allocatable: n.resources_allocatable.cpu_cores,
            memory_allocatable_mb: n.resources_allocatable.memory_mb,
        }
    }
}

impl From<&Node> for GetNodeOutput {
    fn from(n: &Node) -> Self {
        Self {
            id: n.id.to_string(),
            address: n.address.clone(),
            status: format!("{:?}", n.status),
            labels: n.labels.clone(),
            capacity: NodeResourcesOutput {
                cpu_cores: n.resources_capacity.cpu_cores,
                memory_mb: n.resources_capacity.memory_mb,
                disk_mb: n.resources_capacity.disk_mb,
            },
            allocatable: NodeResourcesOutput {
                cpu_cores: n.resources_allocatable.cpu_cores,
                memory_mb: n.resources_allocatable.memory_mb,
                disk_mb: n.resources_allocatable.disk_mb,
            },
            running_instances: vec![], // Will be filled in by caller
        }
    }
}
