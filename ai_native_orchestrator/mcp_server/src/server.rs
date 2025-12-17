//! MCP Server implementation for the orchestrator.
//!
//! This module provides the main MCP server that exposes orchestrator
//! functionality to AI agents via the Model Context Protocol.

use std::collections::HashMap;
use std::sync::Arc;

use rmcp::model::*;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::RwLock;

use state_store_interface::StateStore;
use cluster_manager_interface::ClusterManager;
use scheduler_interface::Scheduler;
use orchestrator_shared_types::{NodeId, WorkloadId};

use crate::tools::*;

/// MCP Server for the AI-native container orchestrator.
///
/// This server exposes the orchestrator's functionality through MCP tools
/// and resources, allowing AI agents to manage workloads, nodes, and
/// query cluster state.
#[derive(Clone)]
pub struct OrchestratorMcpServer<S, C, Sch>
where
    S: StateStore + Send + Sync + 'static,
    C: ClusterManager + Send + Sync + 'static,
    Sch: Scheduler + Send + Sync + 'static,
{
    state_store: Arc<RwLock<S>>,
    cluster_manager: Arc<RwLock<C>>,
    scheduler: Arc<RwLock<Sch>>,
}

impl<S, C, Sch> OrchestratorMcpServer<S, C, Sch>
where
    S: StateStore + Send + Sync + Clone + 'static,
    C: ClusterManager + Send + Sync + Clone + 'static,
    Sch: Scheduler + Send + Sync + Clone + 'static,
{
    /// Create a new MCP server with the given orchestrator components.
    pub fn new(
        state_store: S,
        cluster_manager: C,
        scheduler: Sch,
    ) -> Self {
        Self {
            state_store: Arc::new(RwLock::new(state_store)),
            cluster_manager: Arc::new(RwLock::new(cluster_manager)),
            scheduler: Arc::new(RwLock::new(scheduler)),
        }
    }

    /// Get server info for MCP initialization.
    pub fn server_info() -> Implementation {
        Implementation {
            name: "orchestrator-mcp".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            title: Some("AI-Native Container Orchestrator".to_string()),
            website_url: None,
            icons: None,
        }
    }

    // === Tool Implementations ===

    /// List all workloads.
    async fn handle_list_workloads(&self, input: ListWorkloadsInput) -> Result<ListWorkloadsOutput, String> {
        let store = self.state_store.read().await;
        let workloads = store.list_workloads().await.map_err(|e| e.to_string())?;

        let summaries: Vec<WorkloadSummary> = workloads
            .iter()
            .filter(|w| {
                if let Some(ref filter) = input.name_filter {
                    w.name.contains(filter)
                } else {
                    true
                }
            })
            .map(|w| WorkloadSummary::from(w))
            .collect();

        let total = summaries.len();
        Ok(ListWorkloadsOutput {
            workloads: summaries,
            total,
        })
    }

    /// Create a new workload.
    async fn handle_create_workload(&self, input: CreateWorkloadInput) -> Result<CreateWorkloadOutput, String> {
        let workload_id = WorkloadId::new_v4();
        let workload = input.to_workload_definition(workload_id);

        let store = self.state_store.read().await;
        store
            .put_workload(workload.clone())
            .await
            .map_err(|e| e.to_string())?;

        Ok(CreateWorkloadOutput {
            workload_id: workload_id.to_string(),
            name: input.name,
            replicas: input.replicas,
            message: "Workload created successfully".to_string(),
        })
    }

    /// Get workload details.
    async fn handle_get_workload(&self, input: GetWorkloadInput) -> Result<GetWorkloadOutput, String> {
        let workload_id: WorkloadId = input
            .workload_id
            .parse()
            .map_err(|_| "Invalid workload ID".to_string())?;

        let store = self.state_store.read().await;
        let workload = store
            .get_workload(&workload_id)
            .await
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "Workload not found".to_string())?;

        Ok(GetWorkloadOutput::from(&workload))
    }

    /// Scale a workload.
    async fn handle_scale_workload(&self, input: ScaleWorkloadInput) -> Result<ScaleWorkloadOutput, String> {
        let workload_id: WorkloadId = input
            .workload_id
            .parse()
            .map_err(|_| "Invalid workload ID".to_string())?;

        let store = self.state_store.read().await;
        let mut workload = store
            .get_workload(&workload_id)
            .await
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "Workload not found".to_string())?;

        let previous = workload.replicas;
        workload.replicas = input.replicas;

        store
            .put_workload(workload)
            .await
            .map_err(|e| e.to_string())?;

        Ok(ScaleWorkloadOutput {
            workload_id: workload_id.to_string(),
            previous_replicas: previous,
            new_replicas: input.replicas,
            message: format!("Scaled from {} to {} replicas", previous, input.replicas),
        })
    }

    /// Delete a workload.
    async fn handle_delete_workload(&self, input: DeleteWorkloadInput) -> Result<DeleteWorkloadOutput, String> {
        let workload_id: WorkloadId = input
            .workload_id
            .parse()
            .map_err(|_| "Invalid workload ID".to_string())?;

        let store = self.state_store.read().await;
        store
            .delete_workload(&workload_id)
            .await
            .map_err(|e| e.to_string())?;

        Ok(DeleteWorkloadOutput {
            workload_id: workload_id.to_string(),
            message: "Workload deleted successfully".to_string(),
        })
    }

    /// List all nodes.
    async fn handle_list_nodes(&self, input: ListNodesInput) -> Result<ListNodesOutput, String> {
        let cluster = self.cluster_manager.read().await;
        let nodes = cluster.list_nodes().await.map_err(|e| e.to_string())?;

        let mut status_counts: HashMap<String, usize> = HashMap::new();
        let mut summaries = Vec::new();

        for node in nodes.iter() {
            // Apply status filter
            if let Some(ref status_filter) = input.status_filter {
                let status_str = format!("{:?}", node.status);
                if !status_str.eq_ignore_ascii_case(status_filter) {
                    continue;
                }
            }

            // Apply label filter
            if let Some(ref label_key) = input.label_key {
                match node.labels.get(label_key) {
                    Some(value) => {
                        if let Some(ref label_value) = input.label_value {
                            if value != label_value {
                                continue;
                            }
                        }
                    }
                    None => continue,
                }
            }

            let status_str = format!("{:?}", node.status);
            *status_counts.entry(status_str).or_insert(0) += 1;
            summaries.push(NodeSummary::from(node));
        }

        let total = summaries.len();
        Ok(ListNodesOutput {
            nodes: summaries,
            total,
            status_counts,
        })
    }

    /// Get node details.
    async fn handle_get_node(&self, input: GetNodeInput) -> Result<GetNodeOutput, String> {
        let node_id: NodeId = input
            .node_id
            .parse()
            .map_err(|_| "Invalid node ID".to_string())?;

        let cluster = self.cluster_manager.read().await;
        let node = cluster
            .get_node(&node_id)
            .await
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "Node not found".to_string())?;

        Ok(GetNodeOutput::from(&node))
    }

    /// Get cluster status.
    async fn handle_get_cluster_status(&self, _input: GetClusterStatusInput) -> Result<GetClusterStatusOutput, String> {
        let cluster = self.cluster_manager.read().await;
        let nodes = cluster.list_nodes().await.map_err(|e| e.to_string())?;

        let store = self.state_store.read().await;
        let workloads = store.list_workloads().await.map_err(|e| e.to_string())?;

        let mut nodes_by_status: HashMap<String, usize> = HashMap::new();
        let mut total_cpu = 0.0f32;
        let mut total_mem = 0u64;
        let mut used_cpu = 0.0f32;
        let mut used_mem = 0u64;

        for node in nodes.iter() {
            let status_str = format!("{:?}", node.status);
            *nodes_by_status.entry(status_str).or_insert(0) += 1;
            total_cpu += node.resources_capacity.cpu_cores;
            total_mem += node.resources_capacity.memory_mb;
            used_cpu += node.resources_capacity.cpu_cores - node.resources_allocatable.cpu_cores;
            used_mem += node.resources_capacity.memory_mb - node.resources_allocatable.memory_mb;
        }

        let total_instances: u32 = workloads.iter().map(|w| w.replicas).sum();

        let health = if nodes.is_empty() {
            "Unhealthy"
        } else if nodes_by_status.get("Ready").copied().unwrap_or(0) == nodes.len() {
            "Healthy"
        } else {
            "Degraded"
        };

        Ok(GetClusterStatusOutput {
            total_nodes: nodes.len(),
            nodes_by_status,
            total_workloads: workloads.len(),
            total_instances: total_instances as usize,
            total_cpu_capacity: total_cpu,
            total_memory_capacity_mb: total_mem,
            used_cpu,
            used_memory_mb: used_mem,
            health: health.to_string(),
        })
    }
}

/// Tool definitions for the MCP server.
///
/// This struct holds the tool schemas and dispatch logic.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ToolDefinitions {
    /// Available tools
    pub tools: Vec<ToolInfo>,
}

/// Information about a single tool.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ToolInfo {
    /// Tool name
    pub name: String,
    /// Tool description
    pub description: String,
    /// Input schema (JSON Schema)
    pub input_schema: Value,
}

impl ToolDefinitions {
    /// Get all available tool definitions.
    pub fn all() -> Self {
        Self {
            tools: vec![
                ToolInfo {
                    name: "list_workloads".to_string(),
                    description: "List all workloads in the cluster, optionally filtered by name".to_string(),
                    input_schema: serde_json::to_value(schemars::schema_for!(ListWorkloadsInput)).unwrap_or_default(),
                },
                ToolInfo {
                    name: "create_workload".to_string(),
                    description: "Create a new workload with the specified configuration".to_string(),
                    input_schema: serde_json::to_value(schemars::schema_for!(CreateWorkloadInput)).unwrap_or_default(),
                },
                ToolInfo {
                    name: "get_workload".to_string(),
                    description: "Get detailed information about a specific workload".to_string(),
                    input_schema: serde_json::to_value(schemars::schema_for!(GetWorkloadInput)).unwrap_or_default(),
                },
                ToolInfo {
                    name: "scale_workload".to_string(),
                    description: "Scale a workload to a new replica count".to_string(),
                    input_schema: serde_json::to_value(schemars::schema_for!(ScaleWorkloadInput)).unwrap_or_default(),
                },
                ToolInfo {
                    name: "delete_workload".to_string(),
                    description: "Delete a workload and all its instances".to_string(),
                    input_schema: serde_json::to_value(schemars::schema_for!(DeleteWorkloadInput)).unwrap_or_default(),
                },
                ToolInfo {
                    name: "list_nodes".to_string(),
                    description: "List all nodes in the cluster, optionally filtered by status or labels".to_string(),
                    input_schema: serde_json::to_value(schemars::schema_for!(ListNodesInput)).unwrap_or_default(),
                },
                ToolInfo {
                    name: "get_node".to_string(),
                    description: "Get detailed information about a specific node".to_string(),
                    input_schema: serde_json::to_value(schemars::schema_for!(GetNodeInput)).unwrap_or_default(),
                },
                ToolInfo {
                    name: "get_cluster_status".to_string(),
                    description: "Get overall cluster status including node count, resource usage, and health".to_string(),
                    input_schema: serde_json::to_value(schemars::schema_for!(GetClusterStatusInput)).unwrap_or_default(),
                },
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper trait implementations for testing
    #[derive(Clone)]
    struct DummyStateStore;

    #[async_trait::async_trait]
    impl StateStore for DummyStateStore {
        async fn initialize(&self) -> orchestrator_shared_types::Result<()> { Ok(()) }
        async fn health_check(&self) -> orchestrator_shared_types::Result<bool> { Ok(true) }
        async fn put_node(&self, _: orchestrator_shared_types::Node) -> orchestrator_shared_types::Result<()> { Ok(()) }
        async fn get_node(&self, _: &orchestrator_shared_types::NodeId) -> orchestrator_shared_types::Result<Option<orchestrator_shared_types::Node>> { Ok(None) }
        async fn list_nodes(&self) -> orchestrator_shared_types::Result<Vec<orchestrator_shared_types::Node>> { Ok(vec![]) }
        async fn delete_node(&self, _: &orchestrator_shared_types::NodeId) -> orchestrator_shared_types::Result<()> { Ok(()) }
        async fn put_workload(&self, _: orchestrator_shared_types::WorkloadDefinition) -> orchestrator_shared_types::Result<()> { Ok(()) }
        async fn get_workload(&self, _: &orchestrator_shared_types::WorkloadId) -> orchestrator_shared_types::Result<Option<orchestrator_shared_types::WorkloadDefinition>> { Ok(None) }
        async fn list_workloads(&self) -> orchestrator_shared_types::Result<Vec<orchestrator_shared_types::WorkloadDefinition>> { Ok(vec![]) }
        async fn delete_workload(&self, _: &orchestrator_shared_types::WorkloadId) -> orchestrator_shared_types::Result<()> { Ok(()) }
        async fn put_instance(&self, _: orchestrator_shared_types::WorkloadInstance) -> orchestrator_shared_types::Result<()> { Ok(()) }
        async fn get_instance(&self, _: &str) -> orchestrator_shared_types::Result<Option<orchestrator_shared_types::WorkloadInstance>> { Ok(None) }
        async fn list_instances_for_workload(&self, _: &orchestrator_shared_types::WorkloadId) -> orchestrator_shared_types::Result<Vec<orchestrator_shared_types::WorkloadInstance>> { Ok(vec![]) }
        async fn list_all_instances(&self) -> orchestrator_shared_types::Result<Vec<orchestrator_shared_types::WorkloadInstance>> { Ok(vec![]) }
        async fn delete_instance(&self, _: &str) -> orchestrator_shared_types::Result<()> { Ok(()) }
        async fn delete_instances_for_workload(&self, _: &orchestrator_shared_types::WorkloadId) -> orchestrator_shared_types::Result<()> { Ok(()) }
    }

    #[derive(Clone)]
    struct DummyClusterManager;

    #[async_trait::async_trait]
    impl ClusterManager for DummyClusterManager {
        async fn initialize(&self) -> orchestrator_shared_types::Result<()> { Ok(()) }
        async fn get_node(&self, _: &orchestrator_shared_types::NodeId) -> orchestrator_shared_types::Result<Option<orchestrator_shared_types::Node>> { Ok(None) }
        async fn list_nodes(&self) -> orchestrator_shared_types::Result<Vec<orchestrator_shared_types::Node>> { Ok(vec![]) }
        async fn subscribe_to_events(&self) -> orchestrator_shared_types::Result<tokio::sync::watch::Receiver<Option<cluster_manager_interface::ClusterEvent>>> {
            let (tx, rx) = tokio::sync::watch::channel(None);
            let _ = tx;
            Ok(rx)
        }
    }

    #[derive(Clone)]
    struct DummyScheduler;

    #[async_trait::async_trait]
    impl Scheduler for DummyScheduler {
        async fn schedule(
            &self,
            _request: &scheduler_interface::ScheduleRequest,
            _available_nodes: &[orchestrator_shared_types::Node],
        ) -> orchestrator_shared_types::Result<Vec<scheduler_interface::ScheduleDecision>> {
            Ok(vec![])
        }
    }

    #[test]
    fn test_server_info() {
        let info = OrchestratorMcpServer::<DummyStateStore, DummyClusterManager, DummyScheduler>::server_info();
        assert_eq!(info.name, "orchestrator-mcp");
    }

    #[test]
    fn test_tool_definitions() {
        let tools = ToolDefinitions::all();
        assert!(!tools.tools.is_empty());
        assert!(tools.tools.iter().any(|t| t.name == "list_workloads"));
        assert!(tools.tools.iter().any(|t| t.name == "create_workload"));
        assert!(tools.tools.iter().any(|t| t.name == "list_nodes"));
    }

    #[test]
    fn test_server_creation() {
        let store = DummyStateStore;
        let cluster = DummyClusterManager;
        let scheduler = DummyScheduler;
        let _server = OrchestratorMcpServer::new(store, cluster, scheduler);
    }
}
