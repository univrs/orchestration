//! Status command - show cluster and workload status.

use clap::Args;
use serde::{Deserialize, Serialize};
use tabled::Tabled;

use crate::client::ApiClient;
use crate::output::{self, print_data, section};
use crate::OutputFormat;

/// Arguments for the status command.
#[derive(Args)]
pub struct StatusArgs {
    /// Show detailed status
    #[arg(short, long)]
    detailed: bool,

    /// Filter by workload name
    #[arg(short, long)]
    workload: Option<String>,

    /// Show only nodes
    #[arg(long)]
    nodes_only: bool,

    /// Show only workloads
    #[arg(long)]
    workloads_only: bool,
}

/// Generic list response wrapper from API.
#[derive(Debug, Deserialize)]
struct ListResponse<T> {
    items: Vec<T>,
    #[allow(dead_code)]
    count: usize,
}

/// Cluster status response from API.
#[derive(Debug, Deserialize)]
struct ClusterStatusResponse {
    total_nodes: usize,
    ready_nodes: usize,
    #[allow(dead_code)]
    not_ready_nodes: usize,
    total_workloads: usize,
    #[allow(dead_code)]
    total_instances: usize,
    running_instances: usize,
    pending_instances: usize,
    failed_instances: usize,
    total_cpu_capacity: f32,
    total_memory_mb: u64,
    total_cpu_allocatable: f32,
    total_memory_allocatable_mb: u64,
}

/// Resource information from API.
#[derive(Debug, Serialize, Deserialize)]
struct ResourcesResponse {
    cpu_cores: f32,
    memory_mb: u64,
    #[allow(dead_code)]
    disk_mb: u64,
}

/// Node response from API.
#[derive(Debug, Serialize, Deserialize)]
struct NodeResponse {
    id: String,
    status: String,
    address: String,
    #[allow(dead_code)]
    labels: std::collections::HashMap<String, String>,
    resources_capacity: ResourcesResponse,
    resources_allocatable: ResourcesResponse,
}

/// Display-friendly node for table output.
#[derive(Debug, Serialize, Tabled)]
struct NodeDisplay {
    #[tabled(rename = "ID")]
    id: String,
    #[tabled(rename = "Status")]
    status: String,
    #[tabled(rename = "Address")]
    address: String,
    #[tabled(rename = "CPU (alloc)")]
    cpu_allocatable: String,
    #[tabled(rename = "Memory (alloc)")]
    memory_allocatable: String,
}

impl From<NodeResponse> for NodeDisplay {
    fn from(n: NodeResponse) -> Self {
        NodeDisplay {
            id: n.id[..8.min(n.id.len())].to_string(),
            status: n.status,
            address: n.address,
            cpu_allocatable: format!(
                "{:.1}/{:.1}",
                n.resources_allocatable.cpu_cores, n.resources_capacity.cpu_cores
            ),
            memory_allocatable: format!(
                "{}/{} MB",
                n.resources_allocatable.memory_mb, n.resources_capacity.memory_mb
            ),
        }
    }
}

/// Workload response from API.
#[derive(Debug, Serialize, Deserialize)]
struct WorkloadResponse {
    id: String,
    name: String,
    replicas: u32,
    #[allow(dead_code)]
    labels: std::collections::HashMap<String, String>,
    containers: Vec<ContainerConfigResponse>,
}

/// Container config response from API.
#[derive(Debug, Serialize, Deserialize)]
struct ContainerConfigResponse {
    name: String,
    image: String,
    #[allow(dead_code)]
    command: Option<Vec<String>>,
    #[allow(dead_code)]
    args: Option<Vec<String>>,
    #[allow(dead_code)]
    env_vars: std::collections::HashMap<String, String>,
    #[allow(dead_code)]
    ports: Vec<PortMappingResponse>,
    #[allow(dead_code)]
    resource_requests: ResourcesResponse,
}

#[derive(Debug, Serialize, Deserialize)]
struct PortMappingResponse {
    #[allow(dead_code)]
    container_port: u16,
    #[allow(dead_code)]
    host_port: Option<u16>,
    #[allow(dead_code)]
    protocol: String,
}

/// Display-friendly workload for table output.
#[derive(Debug, Serialize, Tabled)]
struct WorkloadDisplay {
    #[tabled(rename = "ID")]
    id: String,
    #[tabled(rename = "Name")]
    name: String,
    #[tabled(rename = "Replicas")]
    replicas: u32,
    #[tabled(rename = "Image")]
    image: String,
}

impl From<&WorkloadResponse> for WorkloadDisplay {
    fn from(w: &WorkloadResponse) -> Self {
        WorkloadDisplay {
            id: w.id[..8.min(w.id.len())].to_string(),
            name: w.name.clone(),
            replicas: w.replicas,
            image: w
                .containers
                .first()
                .map(|c| c.image.clone())
                .unwrap_or_else(|| "-".to_string()),
        }
    }
}

/// Workload instance response from API.
#[derive(Debug, Serialize, Deserialize)]
struct InstanceResponse {
    id: String,
    workload_id: String,
    node_id: String,
    status: String,
    container_ids: Vec<String>,
}

/// Display-friendly instance for table output.
#[derive(Debug, Serialize, Tabled)]
struct InstanceDisplay {
    #[tabled(rename = "ID")]
    id: String,
    #[tabled(rename = "Workload")]
    workload_id: String,
    #[tabled(rename = "Node")]
    node_id: String,
    #[tabled(rename = "Status")]
    status: String,
    #[tabled(rename = "Containers")]
    containers: usize,
}

impl From<InstanceResponse> for InstanceDisplay {
    fn from(i: InstanceResponse) -> Self {
        InstanceDisplay {
            id: i.id[..8.min(i.id.len())].to_string(),
            workload_id: i.workload_id[..8.min(i.workload_id.len())].to_string(),
            node_id: i.node_id[..8.min(i.node_id.len())].to_string(),
            status: i.status,
            containers: i.container_ids.len(),
        }
    }
}

/// Execute the status command.
pub async fn execute(args: StatusArgs, api_url: &str, format: OutputFormat) -> anyhow::Result<()> {
    let client = match ApiClient::authenticated(api_url).await {
        Ok(c) => c,
        Err(e) => {
            output::warn(&format!(
                "Could not load identity for authentication: {}",
                e
            ));
            output::info("Using unauthenticated client (some endpoints may fail)");
            ApiClient::new(api_url)
        }
    };

    // Show cluster status first (unless filtering)
    if !args.nodes_only && !args.workloads_only {
        match client
            .get::<ClusterStatusResponse>("/api/v1/cluster/status")
            .await
        {
            Ok(status) => {
                section("Cluster Status");
                println!(
                    "  Nodes:       {}/{} ready",
                    status.ready_nodes, status.total_nodes
                );
                println!("  Workloads:   {}", status.total_workloads);
                println!(
                    "  Instances:   {} running, {} pending, {} failed",
                    status.running_instances, status.pending_instances, status.failed_instances
                );
                println!(
                    "  CPU:         {:.1}/{:.1} cores allocatable",
                    status.total_cpu_allocatable, status.total_cpu_capacity
                );
                println!(
                    "  Memory:      {}/{} MB allocatable",
                    status.total_memory_allocatable_mb, status.total_memory_mb
                );
            }
            Err(e) => {
                output::error(&format!("Failed to get cluster status: {}", e));
            }
        }
    }

    // Show nodes
    if !args.workloads_only {
        section("Nodes");
        match client
            .get::<ListResponse<NodeResponse>>("/api/v1/nodes")
            .await
        {
            Ok(response) => {
                let displays: Vec<NodeDisplay> =
                    response.items.into_iter().map(Into::into).collect();
                print_data(&displays, format)?;
            }
            Err(e) => {
                output::error(&format!("Failed to get nodes: {}", e));
            }
        }
    }

    // Show workloads
    if !args.nodes_only {
        section("Workloads");
        match client
            .get::<ListResponse<WorkloadResponse>>("/api/v1/workloads")
            .await
        {
            Ok(response) => {
                let workloads = response.items;
                let filtered: Vec<_> = if let Some(ref name) = args.workload {
                    workloads
                        .into_iter()
                        .filter(|w| w.name.contains(name))
                        .collect()
                } else {
                    workloads
                };

                let displays: Vec<WorkloadDisplay> = filtered.iter().map(Into::into).collect();
                print_data(&displays, format)?;

                // Show instances if detailed
                if args.detailed && !filtered.is_empty() {
                    section("Instances");
                    for workload in &filtered {
                        let path = format!("/api/v1/workloads/{}/instances", workload.id);
                        match client.get::<ListResponse<InstanceResponse>>(&path).await {
                            Ok(inst_response) => {
                                if !inst_response.items.is_empty() {
                                    println!(
                                        "\n  Workload: {} ({})",
                                        workload.name,
                                        &workload.id[..8]
                                    );
                                    let displays: Vec<InstanceDisplay> =
                                        inst_response.items.into_iter().map(Into::into).collect();
                                    print_data(&displays, format)?;
                                }
                            }
                            Err(e) => {
                                output::error(&format!(
                                    "Failed to get instances for {}: {}",
                                    workload.name, e
                                ));
                            }
                        }
                    }
                }
            }
            Err(e) => {
                output::error(&format!("Failed to get workloads: {}", e));
            }
        }
    }

    Ok(())
}
