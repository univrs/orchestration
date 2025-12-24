use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;
use uuid::Uuid;

// Re-export univrs-identity types
pub use univrs_identity::{Keypair, PublicKey, Signature};

/// NodeId is an Ed25519 public key providing cryptographic identity.
/// This enables:
/// - Secure node authentication
/// - Message signing for cluster communication
/// - Self-sovereign identity (no central authority)
pub type NodeId = PublicKey;

pub type WorkloadId = Uuid;
pub type ContainerId = String; // Typically a hash provided by the runtime

#[derive(Debug, Error)]
pub enum OrchestrationError {
    #[error("Node not found: {0}")]
    NodeNotFound(NodeId),
    #[error("Workload not found: {0}")]
    WorkloadNotFound(WorkloadId),
    #[error("Container runtime error: {0}")]
    RuntimeError(String),
    #[error("Scheduling error: {0}")]
    SchedulingError(String),
    #[error("Cluster management error: {0}")]
    ClusterError(String),
    #[error("State persistence error: {0}")]
    StateError(String),
    #[error("Configuration error: {0}")]
    ConfigError(String),
    #[error("Network error: {0}")]
    NetworkError(String),
    #[error("Internal error: {0}")]
    InternalError(String),
    #[error("Feature not implemented: {0}")]
    NotImplemented(String),
}

// Represents a node in the cluster
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Node {
    pub id: NodeId,
    pub address: String, // e.g., "10.0.0.1:8080"
    pub status: NodeStatus,
    pub labels: HashMap<String, String>,
    pub resources_capacity: NodeResources,
    pub resources_allocatable: NodeResources, // Capacity - system overhead
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum NodeStatus {
    Ready,
    NotReady,
    Unknown,
    Down,
}

// Represents available/requested resources on a node or for a workload
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct NodeResources {
    pub cpu_cores: f32,    // e.g., 2.0 for 2 cores, 0.5 for half a core
    pub memory_mb: u64,    // Memory in Megabytes
    pub disk_mb: u64,      // Disk space in Megabytes
    // Potentially GPU resources, custom resources, etc.
}

// Configuration for a single container within a workload
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContainerConfig {
    pub name: String,
    pub image: String, // e.g., "nginx:latest"
    pub command: Option<Vec<String>>,
    pub args: Option<Vec<String>>,
    pub env_vars: HashMap<String, String>,
    pub ports: Vec<PortMapping>,
    pub resource_requests: NodeResources,
    // Volume mounts, health checks, etc. would go here
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PortMapping {
    pub container_port: u16,
    pub host_port: Option<u16>, // If None, runtime chooses an ephemeral port
    pub protocol: String,       // "tcp" or "udp"
}

// Defines a workload to be run on the cluster
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkloadDefinition {
    pub id: WorkloadId,
    pub name: String, // User-friendly name
    pub containers: Vec<ContainerConfig>,
    pub replicas: u32,
    pub labels: HashMap<String, String>, // For scheduling, selection
    // Placement constraints, update strategy, etc.
}

// Represents an instance of a workload running on a specific node
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkloadInstance {
    pub id: Uuid, // Unique ID for this instance
    pub workload_id: WorkloadId,
    pub node_id: NodeId,
    pub container_ids: Vec<ContainerId>, // IDs of containers run by the runtime for this instance
    pub status: WorkloadInstanceStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum WorkloadInstanceStatus {
    Pending,
    Running,
    Succeeded,
    Failed,
    Unknown,
    Terminating,
}

// Generic result type for orchestration operations
pub type Result<T> = std::result::Result<T, OrchestrationError>;