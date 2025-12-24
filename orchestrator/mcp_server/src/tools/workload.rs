//! Workload management MCP tools.
//!
//! These tools allow AI agents to create, update, scale, and delete workloads.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use orchestrator_shared_types::{
    ContainerConfig, NodeResources, PortMapping, WorkloadDefinition, WorkloadId,
};

/// Input for creating a new workload
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CreateWorkloadInput {
    /// Human-readable name for the workload
    pub name: String,
    /// Number of replicas to run
    pub replicas: u32,
    /// Container image to use (e.g., "nginx:latest")
    pub image: String,
    /// Optional command to run
    #[serde(default)]
    pub command: Option<Vec<String>>,
    /// Optional arguments for the command
    #[serde(default)]
    pub args: Option<Vec<String>>,
    /// Environment variables
    #[serde(default)]
    pub env_vars: HashMap<String, String>,
    /// Port mappings
    #[serde(default)]
    pub ports: Vec<PortMappingInput>,
    /// CPU cores requested
    #[serde(default = "default_cpu")]
    pub cpu_cores: f32,
    /// Memory in MB requested
    #[serde(default = "default_memory")]
    pub memory_mb: u64,
}

fn default_cpu() -> f32 {
    0.5
}
fn default_memory() -> u64 {
    256
}

/// Port mapping input
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PortMappingInput {
    /// Container port
    pub container_port: u16,
    /// Optional host port (auto-assigned if not specified)
    pub host_port: Option<u16>,
    /// Protocol (tcp or udp)
    #[serde(default = "default_protocol")]
    pub protocol: String,
}

fn default_protocol() -> String {
    "tcp".to_string()
}

/// Output from creating a workload
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CreateWorkloadOutput {
    /// The ID of the created workload
    pub workload_id: String,
    /// Human-readable name
    pub name: String,
    /// Number of replicas requested
    pub replicas: u32,
    /// Status message
    pub message: String,
}

/// Input for scaling a workload
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ScaleWorkloadInput {
    /// Workload ID to scale
    pub workload_id: String,
    /// New number of replicas
    pub replicas: u32,
}

/// Output from scaling a workload
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ScaleWorkloadOutput {
    /// The workload ID
    pub workload_id: String,
    /// Previous replica count
    pub previous_replicas: u32,
    /// New replica count
    pub new_replicas: u32,
    /// Status message
    pub message: String,
}

/// Input for deleting a workload
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DeleteWorkloadInput {
    /// Workload ID to delete
    pub workload_id: String,
}

/// Output from deleting a workload
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DeleteWorkloadOutput {
    /// The workload ID that was deleted
    pub workload_id: String,
    /// Status message
    pub message: String,
}

/// Input for listing workloads
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct ListWorkloadsInput {
    /// Optional filter by name pattern
    #[serde(default)]
    pub name_filter: Option<String>,
}

/// Workload summary for listing
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WorkloadSummary {
    /// Workload ID
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Desired replicas
    pub desired_replicas: u32,
    /// Running instances count
    pub running_instances: u32,
    /// Workload status
    pub status: String,
}

/// Output from listing workloads
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ListWorkloadsOutput {
    /// List of workload summaries
    pub workloads: Vec<WorkloadSummary>,
    /// Total count
    pub total: usize,
}

/// Input for getting workload details
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GetWorkloadInput {
    /// Workload ID to get
    pub workload_id: String,
}

/// Detailed workload output
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GetWorkloadOutput {
    /// Workload ID
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Full workload specification
    pub spec: WorkloadSpecOutput,
    /// Current status
    pub status: String,
    /// Running instance IDs
    pub instances: Vec<String>,
}

/// Workload spec output
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WorkloadSpecOutput {
    /// Desired replicas
    pub replicas: u32,
    /// Container configurations
    pub containers: Vec<ContainerConfigOutput>,
}

/// Container config output
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ContainerConfigOutput {
    /// Container name
    pub name: String,
    /// Image
    pub image: String,
    /// Command
    pub command: Option<Vec<String>>,
    /// Args
    pub args: Option<Vec<String>>,
    /// Environment variables
    pub env_vars: HashMap<String, String>,
    /// Ports
    pub ports: Vec<PortMappingInput>,
    /// Resource requests
    pub resources: ResourcesOutput,
}

/// Resources output
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ResourcesOutput {
    pub cpu_cores: f32,
    pub memory_mb: u64,
    pub disk_mb: u64,
}

impl CreateWorkloadInput {
    /// Convert to internal WorkloadDefinition type
    pub fn to_workload_definition(&self, id: WorkloadId) -> WorkloadDefinition {
        let container = ContainerConfig {
            name: self.name.clone(),
            image: self.image.clone(),
            command: self.command.clone(),
            args: self.args.clone(),
            env_vars: self.env_vars.clone(),
            ports: self
                .ports
                .iter()
                .map(|p| PortMapping {
                    container_port: p.container_port,
                    host_port: p.host_port,
                    protocol: p.protocol.clone(),
                })
                .collect(),
            resource_requests: NodeResources {
                cpu_cores: self.cpu_cores,
                memory_mb: self.memory_mb,
                disk_mb: 0,
            },
        };

        WorkloadDefinition {
            id,
            name: self.name.clone(),
            containers: vec![container],
            replicas: self.replicas,
            labels: HashMap::new(),
        }
    }
}

impl From<&WorkloadDefinition> for WorkloadSummary {
    fn from(w: &WorkloadDefinition) -> Self {
        Self {
            id: w.id.to_string(),
            name: w.name.clone(),
            desired_replicas: w.replicas,
            running_instances: 0, // Will be filled in by caller
            status: "Unknown".to_string(),
        }
    }
}

impl From<&WorkloadDefinition> for GetWorkloadOutput {
    fn from(w: &WorkloadDefinition) -> Self {
        Self {
            id: w.id.to_string(),
            name: w.name.clone(),
            spec: WorkloadSpecOutput {
                replicas: w.replicas,
                containers: w
                    .containers
                    .iter()
                    .map(|c| ContainerConfigOutput {
                        name: c.name.clone(),
                        image: c.image.clone(),
                        command: c.command.clone(),
                        args: c.args.clone(),
                        env_vars: c.env_vars.clone(),
                        ports: c
                            .ports
                            .iter()
                            .map(|p| PortMappingInput {
                                container_port: p.container_port,
                                host_port: p.host_port,
                                protocol: p.protocol.clone(),
                            })
                            .collect(),
                        resources: ResourcesOutput {
                            cpu_cores: c.resource_requests.cpu_cores,
                            memory_mb: c.resource_requests.memory_mb,
                            disk_mb: c.resource_requests.disk_mb,
                        },
                    })
                    .collect(),
            },
            status: "Unknown".to_string(),
            instances: vec![],
        }
    }
}
