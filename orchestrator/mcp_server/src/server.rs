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
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use cluster_manager_interface::ClusterManager;
use orchestrator_shared_types::{NodeId, WorkloadId};
use scheduler_interface::Scheduler;
use state_store_interface::StateStore;

use crate::resources::{self, ResourcePath};
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
    #[allow(dead_code)] // Reserved for future scheduling operations
    scheduler: Arc<RwLock<Sch>>,
}

impl<S, C, Sch> OrchestratorMcpServer<S, C, Sch>
where
    S: StateStore + Send + Sync + Clone + 'static,
    C: ClusterManager + Send + Sync + Clone + 'static,
    Sch: Scheduler + Send + Sync + Clone + 'static,
{
    /// Create a new MCP server with the given orchestrator components.
    pub fn new(state_store: S, cluster_manager: C, scheduler: Sch) -> Self {
        Self {
            state_store: Arc::new(RwLock::new(state_store)),
            cluster_manager: Arc::new(RwLock::new(cluster_manager)),
            scheduler: Arc::new(RwLock::new(scheduler)),
        }
    }
}

impl<S, C, Sch> OrchestratorMcpServer<S, C, Sch>
where
    S: StateStore + Send + Sync + 'static,
    C: ClusterManager + Send + Sync + 'static,
    Sch: Scheduler + Send + Sync + 'static,
{
    /// Create a new MCP server with pre-wrapped Arc components.
    /// Use this when integrating with existing Arc-wrapped orchestrator components.
    pub fn with_shared(
        state_store: Arc<RwLock<S>>,
        cluster_manager: Arc<RwLock<C>>,
        scheduler: Arc<RwLock<Sch>>,
    ) -> Self {
        Self {
            state_store,
            cluster_manager,
            scheduler,
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
    async fn handle_list_workloads(
        &self,
        input: ListWorkloadsInput,
    ) -> Result<ListWorkloadsOutput, String> {
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
    async fn handle_create_workload(
        &self,
        input: CreateWorkloadInput,
    ) -> Result<CreateWorkloadOutput, String> {
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
    async fn handle_get_workload(
        &self,
        input: GetWorkloadInput,
    ) -> Result<GetWorkloadOutput, String> {
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
    async fn handle_scale_workload(
        &self,
        input: ScaleWorkloadInput,
    ) -> Result<ScaleWorkloadOutput, String> {
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
    async fn handle_delete_workload(
        &self,
        input: DeleteWorkloadInput,
    ) -> Result<DeleteWorkloadOutput, String> {
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
    async fn handle_get_cluster_status(
        &self,
        _input: GetClusterStatusInput,
    ) -> Result<GetClusterStatusOutput, String> {
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

// ============================================================================
// JSON-RPC Types
// ============================================================================

/// JSON-RPC 2.0 Request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    /// JSON-RPC version (always "2.0")
    pub jsonrpc: String,
    /// Request ID
    pub id: Option<Value>,
    /// Method name
    pub method: String,
    /// Parameters (optional)
    #[serde(default)]
    pub params: Value,
}

/// JSON-RPC 2.0 Response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    /// JSON-RPC version (always "2.0")
    pub jsonrpc: String,
    /// Request ID (null for notifications)
    pub id: Option<Value>,
    /// Result (on success)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    /// Error (on failure)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

/// JSON-RPC 2.0 Error
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    /// Error code
    pub code: i32,
    /// Error message
    pub message: String,
    /// Additional data
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl JsonRpcResponse {
    /// Create a success response.
    pub fn success(id: Option<Value>, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    /// Create an error response.
    pub fn error(id: Option<Value>, code: i32, message: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(JsonRpcError {
                code,
                message: message.into(),
                data: None,
            }),
        }
    }

    /// Create an error response with data.
    pub fn error_with_data(
        id: Option<Value>,
        code: i32,
        message: impl Into<String>,
        data: Value,
    ) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(JsonRpcError {
                code,
                message: message.into(),
                data: Some(data),
            }),
        }
    }
}

// Standard JSON-RPC error codes
pub const PARSE_ERROR: i32 = -32700;
pub const INVALID_REQUEST: i32 = -32600;
pub const METHOD_NOT_FOUND: i32 = -32601;
pub const INVALID_PARAMS: i32 = -32602;
pub const INTERNAL_ERROR: i32 = -32603;

// ============================================================================
// MCP Protocol Implementation
// ============================================================================

impl<S, C, Sch> OrchestratorMcpServer<S, C, Sch>
where
    S: StateStore + Send + Sync + Clone + 'static,
    C: ClusterManager + Send + Sync + Clone + 'static,
    Sch: Scheduler + Send + Sync + Clone + 'static,
{
    /// Handle an incoming JSON-RPC request.
    pub async fn handle_request(&self, request: JsonRpcRequest) -> JsonRpcResponse {
        debug!(method = %request.method, "Handling MCP request");

        match request.method.as_str() {
            // MCP Protocol Methods
            "initialize" => self.handle_initialize(request.id, request.params).await,
            "initialized" => self.handle_initialized(request.id).await,
            "ping" => self.handle_ping(request.id).await,

            // Tool Methods
            "tools/list" => self.handle_tools_list(request.id).await,
            "tools/call" => self.handle_tools_call(request.id, request.params).await,

            // Resource Methods
            "resources/list" => self.handle_resources_list(request.id).await,
            "resources/read" => self.handle_resources_read(request.id, request.params).await,

            // Unknown method
            _ => {
                warn!(method = %request.method, "Unknown method");
                JsonRpcResponse::error(
                    request.id,
                    METHOD_NOT_FOUND,
                    format!("Method not found: {}", request.method),
                )
            }
        }
    }

    /// Handle initialize request.
    async fn handle_initialize(&self, id: Option<Value>, params: Value) -> JsonRpcResponse {
        #[derive(Debug, Deserialize)]
        #[allow(dead_code)] // Fields reserved for future capability negotiation
        struct InitializeParams {
            #[serde(default)]
            protocol_version: Option<String>,
            #[serde(default)]
            capabilities: Option<Value>,
            #[serde(default)]
            client_info: Option<Value>,
        }

        let _params: InitializeParams = match serde_json::from_value(params) {
            Ok(p) => p,
            Err(e) => {
                return JsonRpcResponse::error(
                    id,
                    INVALID_PARAMS,
                    format!("Invalid params: {}", e),
                );
            }
        };

        let server_info = Self::server_info();
        let result = serde_json::json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "tools": {
                    "listChanged": false
                },
                "resources": {
                    "subscribe": false,
                    "listChanged": false
                }
            },
            "serverInfo": {
                "name": server_info.name,
                "version": server_info.version
            }
        });

        info!("MCP server initialized");
        JsonRpcResponse::success(id, result)
    }

    /// Handle initialized notification.
    async fn handle_initialized(&self, id: Option<Value>) -> JsonRpcResponse {
        debug!("Client sent initialized notification");
        JsonRpcResponse::success(id, serde_json::json!({}))
    }

    /// Handle ping request.
    async fn handle_ping(&self, id: Option<Value>) -> JsonRpcResponse {
        JsonRpcResponse::success(id, serde_json::json!({}))
    }

    /// Handle tools/list request.
    async fn handle_tools_list(&self, id: Option<Value>) -> JsonRpcResponse {
        let tool_defs = ToolDefinitions::all();
        let tools: Vec<Value> = tool_defs
            .tools
            .iter()
            .map(|t| {
                serde_json::json!({
                    "name": t.name,
                    "description": t.description,
                    "inputSchema": t.input_schema
                })
            })
            .collect();

        JsonRpcResponse::success(id, serde_json::json!({ "tools": tools }))
    }

    /// Handle tools/call request.
    async fn handle_tools_call(&self, id: Option<Value>, params: Value) -> JsonRpcResponse {
        #[derive(Debug, Deserialize)]
        struct ToolCallParams {
            name: String,
            #[serde(default)]
            arguments: Value,
        }

        let params: ToolCallParams = match serde_json::from_value(params) {
            Ok(p) => p,
            Err(e) => {
                return JsonRpcResponse::error(
                    id,
                    INVALID_PARAMS,
                    format!("Invalid params: {}", e),
                );
            }
        };

        debug!(tool = %params.name, "Calling tool");

        let result = match params.name.as_str() {
            "list_workloads" => {
                let input: ListWorkloadsInput =
                    serde_json::from_value(params.arguments).unwrap_or_default();
                match self.handle_list_workloads(input).await {
                    Ok(output) => serde_json::to_value(output).unwrap_or_default(),
                    Err(e) => return JsonRpcResponse::error(id, INTERNAL_ERROR, e),
                }
            }
            "create_workload" => {
                let input: CreateWorkloadInput = match serde_json::from_value(params.arguments) {
                    Ok(i) => i,
                    Err(e) => return JsonRpcResponse::error(id, INVALID_PARAMS, e.to_string()),
                };
                match self.handle_create_workload(input).await {
                    Ok(output) => serde_json::to_value(output).unwrap_or_default(),
                    Err(e) => return JsonRpcResponse::error(id, INTERNAL_ERROR, e),
                }
            }
            "get_workload" => {
                let input: GetWorkloadInput = match serde_json::from_value(params.arguments) {
                    Ok(i) => i,
                    Err(e) => return JsonRpcResponse::error(id, INVALID_PARAMS, e.to_string()),
                };
                match self.handle_get_workload(input).await {
                    Ok(output) => serde_json::to_value(output).unwrap_or_default(),
                    Err(e) => return JsonRpcResponse::error(id, INTERNAL_ERROR, e),
                }
            }
            "scale_workload" => {
                let input: ScaleWorkloadInput = match serde_json::from_value(params.arguments) {
                    Ok(i) => i,
                    Err(e) => return JsonRpcResponse::error(id, INVALID_PARAMS, e.to_string()),
                };
                match self.handle_scale_workload(input).await {
                    Ok(output) => serde_json::to_value(output).unwrap_or_default(),
                    Err(e) => return JsonRpcResponse::error(id, INTERNAL_ERROR, e),
                }
            }
            "delete_workload" => {
                let input: DeleteWorkloadInput = match serde_json::from_value(params.arguments) {
                    Ok(i) => i,
                    Err(e) => return JsonRpcResponse::error(id, INVALID_PARAMS, e.to_string()),
                };
                match self.handle_delete_workload(input).await {
                    Ok(output) => serde_json::to_value(output).unwrap_or_default(),
                    Err(e) => return JsonRpcResponse::error(id, INTERNAL_ERROR, e),
                }
            }
            "list_nodes" => {
                let input: ListNodesInput =
                    serde_json::from_value(params.arguments).unwrap_or_default();
                match self.handle_list_nodes(input).await {
                    Ok(output) => serde_json::to_value(output).unwrap_or_default(),
                    Err(e) => return JsonRpcResponse::error(id, INTERNAL_ERROR, e),
                }
            }
            "get_node" => {
                let input: GetNodeInput = match serde_json::from_value(params.arguments) {
                    Ok(i) => i,
                    Err(e) => return JsonRpcResponse::error(id, INVALID_PARAMS, e.to_string()),
                };
                match self.handle_get_node(input).await {
                    Ok(output) => serde_json::to_value(output).unwrap_or_default(),
                    Err(e) => return JsonRpcResponse::error(id, INTERNAL_ERROR, e),
                }
            }
            "get_cluster_status" => {
                let input: GetClusterStatusInput =
                    serde_json::from_value(params.arguments).unwrap_or_default();
                match self.handle_get_cluster_status(input).await {
                    Ok(output) => serde_json::to_value(output).unwrap_or_default(),
                    Err(e) => return JsonRpcResponse::error(id, INTERNAL_ERROR, e),
                }
            }
            _ => {
                return JsonRpcResponse::error(
                    id,
                    METHOD_NOT_FOUND,
                    format!("Unknown tool: {}", params.name),
                );
            }
        };

        // Format result as MCP tool result
        let content = serde_json::json!([{
            "type": "text",
            "text": serde_json::to_string_pretty(&result).unwrap_or_default()
        }]);

        JsonRpcResponse::success(id, serde_json::json!({ "content": content }))
    }

    /// Handle resources/list request.
    async fn handle_resources_list(&self, id: Option<Value>) -> JsonRpcResponse {
        let resources = resources::list_available_resources();
        let resource_list: Vec<Value> = resources
            .iter()
            .map(|r| {
                serde_json::json!({
                    "uri": r.uri,
                    "name": r.name,
                    "description": r.description,
                    "mimeType": r.mime_type
                })
            })
            .collect();

        JsonRpcResponse::success(id, serde_json::json!({ "resources": resource_list }))
    }

    /// Handle resources/read request.
    async fn handle_resources_read(&self, id: Option<Value>, params: Value) -> JsonRpcResponse {
        #[derive(Debug, Deserialize)]
        struct ReadParams {
            uri: String,
        }

        let params: ReadParams = match serde_json::from_value(params) {
            Ok(p) => p,
            Err(e) => {
                return JsonRpcResponse::error(
                    id,
                    INVALID_PARAMS,
                    format!("Invalid params: {}", e),
                );
            }
        };

        let resource_path = match resources::parse_resource_uri(&params.uri) {
            Some(p) => p,
            None => {
                return JsonRpcResponse::error(
                    id,
                    INVALID_PARAMS,
                    format!("Invalid resource URI: {}", params.uri),
                );
            }
        };

        let content = match resource_path {
            ResourcePath::NodeList => {
                let cluster = self.cluster_manager.read().await;
                match cluster.list_nodes().await {
                    Ok(nodes) => {
                        let node_list: Vec<Value> = nodes
                            .iter()
                            .map(|n| {
                                serde_json::json!({
                                    "id": n.id.to_string(),
                                    "address": n.address,
                                    "status": format!("{:?}", n.status),
                                    "cpu_capacity": n.resources_capacity.cpu_cores,
                                    "memory_capacity_mb": n.resources_capacity.memory_mb
                                })
                            })
                            .collect();
                        serde_json::to_string_pretty(&node_list).unwrap_or_default()
                    }
                    Err(e) => return JsonRpcResponse::error(id, INTERNAL_ERROR, e.to_string()),
                }
            }
            ResourcePath::Node(node_id) => {
                let parsed_id: NodeId = match node_id.parse() {
                    Ok(id) => id,
                    Err(_) => return JsonRpcResponse::error(id, INVALID_PARAMS, "Invalid node ID"),
                };
                let cluster = self.cluster_manager.read().await;
                match cluster.get_node(&parsed_id).await {
                    Ok(Some(node)) => {
                        let node_data = serde_json::json!({
                            "id": node.id.to_string(),
                            "address": node.address,
                            "status": format!("{:?}", node.status),
                            "labels": node.labels,
                            "resources_capacity": {
                                "cpu_cores": node.resources_capacity.cpu_cores,
                                "memory_mb": node.resources_capacity.memory_mb,
                                "disk_mb": node.resources_capacity.disk_mb
                            },
                            "resources_allocatable": {
                                "cpu_cores": node.resources_allocatable.cpu_cores,
                                "memory_mb": node.resources_allocatable.memory_mb,
                                "disk_mb": node.resources_allocatable.disk_mb
                            }
                        });
                        serde_json::to_string_pretty(&node_data).unwrap_or_default()
                    }
                    Ok(None) => {
                        return JsonRpcResponse::error(id, INVALID_PARAMS, "Node not found")
                    }
                    Err(e) => return JsonRpcResponse::error(id, INTERNAL_ERROR, e.to_string()),
                }
            }
            ResourcePath::WorkloadList => {
                let store = self.state_store.read().await;
                match store.list_workloads().await {
                    Ok(workloads) => {
                        let workload_list: Vec<Value> = workloads
                            .iter()
                            .map(|w| {
                                serde_json::json!({
                                    "id": w.id.to_string(),
                                    "name": w.name,
                                    "replicas": w.replicas,
                                    "containers": w.containers.len()
                                })
                            })
                            .collect();
                        serde_json::to_string_pretty(&workload_list).unwrap_or_default()
                    }
                    Err(e) => return JsonRpcResponse::error(id, INTERNAL_ERROR, e.to_string()),
                }
            }
            ResourcePath::Workload(workload_id) => {
                let parsed_id: WorkloadId = match workload_id.parse() {
                    Ok(id) => id,
                    Err(_) => {
                        return JsonRpcResponse::error(id, INVALID_PARAMS, "Invalid workload ID")
                    }
                };
                let store = self.state_store.read().await;
                match store.get_workload(&parsed_id).await {
                    Ok(Some(workload)) => {
                        let workload_data = serde_json::json!({
                            "id": workload.id.to_string(),
                            "name": workload.name,
                            "replicas": workload.replicas,
                            "labels": workload.labels,
                            "containers": workload.containers.iter().map(|c| {
                                serde_json::json!({
                                    "name": c.name,
                                    "image": c.image,
                                    "ports": c.ports
                                })
                            }).collect::<Vec<_>>()
                        });
                        serde_json::to_string_pretty(&workload_data).unwrap_or_default()
                    }
                    Ok(None) => {
                        return JsonRpcResponse::error(id, INVALID_PARAMS, "Workload not found")
                    }
                    Err(e) => return JsonRpcResponse::error(id, INTERNAL_ERROR, e.to_string()),
                }
            }
            ResourcePath::ClusterStatus => {
                match self
                    .handle_get_cluster_status(GetClusterStatusInput::default())
                    .await
                {
                    Ok(status) => serde_json::to_string_pretty(&status).unwrap_or_default(),
                    Err(e) => return JsonRpcResponse::error(id, INTERNAL_ERROR, e),
                }
            }
            ResourcePath::ClusterMetrics => {
                match self
                    .handle_get_cluster_status(GetClusterStatusInput::default())
                    .await
                {
                    Ok(status) => {
                        let metrics = serde_json::json!({
                            "total_cpu_capacity": status.total_cpu_capacity,
                            "total_memory_capacity_mb": status.total_memory_capacity_mb,
                            "used_cpu": status.used_cpu,
                            "used_memory_mb": status.used_memory_mb,
                            "cpu_utilization_percent": if status.total_cpu_capacity > 0.0 {
                                status.used_cpu / status.total_cpu_capacity * 100.0
                            } else { 0.0 },
                            "memory_utilization_percent": if status.total_memory_capacity_mb > 0 {
                                status.used_memory_mb as f64 / status.total_memory_capacity_mb as f64 * 100.0
                            } else { 0.0 }
                        });
                        serde_json::to_string_pretty(&metrics).unwrap_or_default()
                    }
                    Err(e) => return JsonRpcResponse::error(id, INTERNAL_ERROR, e),
                }
            }
        };

        let result = serde_json::json!({
            "contents": [{
                "uri": params.uri,
                "mimeType": "application/json",
                "text": content
            }]
        });

        JsonRpcResponse::success(id, result)
    }

    /// Run the MCP server over stdio.
    pub async fn serve_stdio(self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let stdin = tokio::io::stdin();
        let mut stdout = tokio::io::stdout();
        let reader = BufReader::new(stdin);
        let mut lines = reader.lines();

        info!("MCP server listening on stdio");

        while let Some(line) = lines.next_line().await? {
            if line.is_empty() {
                continue;
            }

            debug!(request = %line, "Received request");

            let request: JsonRpcRequest = match serde_json::from_str(&line) {
                Ok(r) => r,
                Err(e) => {
                    error!(error = %e, "Failed to parse request");
                    let response =
                        JsonRpcResponse::error(None, PARSE_ERROR, format!("Parse error: {}", e));
                    let response_json = serde_json::to_string(&response)?;
                    stdout.write_all(response_json.as_bytes()).await?;
                    stdout.write_all(b"\n").await?;
                    stdout.flush().await?;
                    continue;
                }
            };

            let response = self.handle_request(request).await;
            let response_json = serde_json::to_string(&response)?;

            debug!(response = %response_json, "Sending response");

            stdout.write_all(response_json.as_bytes()).await?;
            stdout.write_all(b"\n").await?;
            stdout.flush().await?;
        }

        info!("MCP server shutdown");
        Ok(())
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
        async fn initialize(&self) -> orchestrator_shared_types::Result<()> {
            Ok(())
        }
        async fn health_check(&self) -> orchestrator_shared_types::Result<bool> {
            Ok(true)
        }
        async fn put_node(
            &self,
            _: orchestrator_shared_types::Node,
        ) -> orchestrator_shared_types::Result<()> {
            Ok(())
        }
        async fn get_node(
            &self,
            _: &orchestrator_shared_types::NodeId,
        ) -> orchestrator_shared_types::Result<Option<orchestrator_shared_types::Node>> {
            Ok(None)
        }
        async fn list_nodes(
            &self,
        ) -> orchestrator_shared_types::Result<Vec<orchestrator_shared_types::Node>> {
            Ok(vec![])
        }
        async fn delete_node(
            &self,
            _: &orchestrator_shared_types::NodeId,
        ) -> orchestrator_shared_types::Result<()> {
            Ok(())
        }
        async fn put_workload(
            &self,
            _: orchestrator_shared_types::WorkloadDefinition,
        ) -> orchestrator_shared_types::Result<()> {
            Ok(())
        }
        async fn get_workload(
            &self,
            _: &orchestrator_shared_types::WorkloadId,
        ) -> orchestrator_shared_types::Result<Option<orchestrator_shared_types::WorkloadDefinition>>
        {
            Ok(None)
        }
        async fn list_workloads(
            &self,
        ) -> orchestrator_shared_types::Result<Vec<orchestrator_shared_types::WorkloadDefinition>>
        {
            Ok(vec![])
        }
        async fn delete_workload(
            &self,
            _: &orchestrator_shared_types::WorkloadId,
        ) -> orchestrator_shared_types::Result<()> {
            Ok(())
        }
        async fn put_instance(
            &self,
            _: orchestrator_shared_types::WorkloadInstance,
        ) -> orchestrator_shared_types::Result<()> {
            Ok(())
        }
        async fn get_instance(
            &self,
            _: &str,
        ) -> orchestrator_shared_types::Result<Option<orchestrator_shared_types::WorkloadInstance>>
        {
            Ok(None)
        }
        async fn list_instances_for_workload(
            &self,
            _: &orchestrator_shared_types::WorkloadId,
        ) -> orchestrator_shared_types::Result<Vec<orchestrator_shared_types::WorkloadInstance>>
        {
            Ok(vec![])
        }
        async fn list_all_instances(
            &self,
        ) -> orchestrator_shared_types::Result<Vec<orchestrator_shared_types::WorkloadInstance>>
        {
            Ok(vec![])
        }
        async fn delete_instance(&self, _: &str) -> orchestrator_shared_types::Result<()> {
            Ok(())
        }
        async fn delete_instances_for_workload(
            &self,
            _: &orchestrator_shared_types::WorkloadId,
        ) -> orchestrator_shared_types::Result<()> {
            Ok(())
        }
    }

    #[derive(Clone)]
    struct DummyClusterManager;

    #[async_trait::async_trait]
    impl ClusterManager for DummyClusterManager {
        async fn initialize(&self) -> orchestrator_shared_types::Result<()> {
            Ok(())
        }
        async fn get_node(
            &self,
            _: &orchestrator_shared_types::NodeId,
        ) -> orchestrator_shared_types::Result<Option<orchestrator_shared_types::Node>> {
            Ok(None)
        }
        async fn list_nodes(
            &self,
        ) -> orchestrator_shared_types::Result<Vec<orchestrator_shared_types::Node>> {
            Ok(vec![])
        }
        async fn subscribe_to_events(
            &self,
        ) -> orchestrator_shared_types::Result<
            tokio::sync::broadcast::Receiver<cluster_manager_interface::ClusterEvent>,
        > {
            let (tx, rx) = tokio::sync::broadcast::channel(16);
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
