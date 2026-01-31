//! API request handlers.

use std::collections::HashMap;

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, Query, State,
    },
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use container_runtime_interface::LogOptions as RuntimeLogOptions;

use orchestrator_shared_types::{
    ContainerConfig, Node, NodeId, NodeResources, NodeStatus, PortMapping, WorkloadDefinition,
    WorkloadInstance, WorkloadInstanceStatus,
};

use super::error::{ApiError, ApiResult};
use super::state::ApiState;

// ============================================================================
// Request/Response Types
// ============================================================================

/// Request to create a new workload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateWorkloadRequest {
    /// User-friendly name for the workload.
    pub name: String,
    /// Container configurations.
    pub containers: Vec<ContainerConfigRequest>,
    /// Desired number of replicas.
    pub replicas: u32,
    /// Optional labels for scheduling and selection.
    #[serde(default)]
    pub labels: HashMap<String, String>,
}

/// Container configuration in API request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerConfigRequest {
    pub name: String,
    pub image: String,
    #[serde(default)]
    pub command: Option<Vec<String>>,
    #[serde(default)]
    pub args: Option<Vec<String>>,
    #[serde(default)]
    pub env_vars: HashMap<String, String>,
    #[serde(default)]
    pub ports: Vec<PortMappingRequest>,
    #[serde(default)]
    pub resource_requests: ResourceRequestsRequest,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortMappingRequest {
    pub container_port: u16,
    pub host_port: Option<u16>,
    #[serde(default = "default_protocol")]
    pub protocol: String,
}

fn default_protocol() -> String {
    "tcp".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ResourceRequestsRequest {
    #[serde(default)]
    pub cpu_cores: f32,
    #[serde(default)]
    pub memory_mb: u64,
    #[serde(default)]
    pub disk_mb: u64,
}

/// Response for workload operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkloadResponse {
    pub id: Uuid,
    pub name: String,
    pub replicas: u32,
    pub labels: HashMap<String, String>,
    pub containers: Vec<ContainerConfigResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerConfigResponse {
    pub name: String,
    pub image: String,
    pub command: Option<Vec<String>>,
    pub args: Option<Vec<String>>,
    pub env_vars: HashMap<String, String>,
    pub ports: Vec<PortMappingResponse>,
    pub resource_requests: ResourceRequestsResponse,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortMappingResponse {
    pub container_port: u16,
    pub host_port: Option<u16>,
    pub protocol: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceRequestsResponse {
    pub cpu_cores: f32,
    pub memory_mb: u64,
    pub disk_mb: u64,
}

/// Response for list operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListResponse<T> {
    pub items: Vec<T>,
    pub count: usize,
}

/// Node response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeResponse {
    pub id: String,
    pub address: String,
    pub status: String,
    pub labels: HashMap<String, String>,
    pub resources_capacity: ResourceRequestsResponse,
    pub resources_allocatable: ResourceRequestsResponse,
}

/// Workload instance response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstanceResponse {
    pub id: Uuid,
    pub workload_id: Uuid,
    pub node_id: String,
    pub container_ids: Vec<String>,
    pub status: String,
}

/// Cluster status response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterStatusResponse {
    pub total_nodes: usize,
    pub ready_nodes: usize,
    pub not_ready_nodes: usize,
    pub total_workloads: usize,
    pub total_instances: usize,
    pub running_instances: usize,
    pub pending_instances: usize,
    pub failed_instances: usize,
    pub total_cpu_capacity: f32,
    pub total_memory_mb: u64,
    pub total_cpu_allocatable: f32,
    pub total_memory_allocatable_mb: u64,
}

/// Query parameters for log requests.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct LogsQuery {
    /// Return only the last N lines.
    pub tail: Option<usize>,
    /// Include timestamps in output.
    #[serde(default)]
    pub timestamps: bool,
    /// Only return logs since this timestamp (RFC3339).
    pub since: Option<String>,
    /// Only return logs until this timestamp (RFC3339).
    pub until: Option<String>,
}

/// Response for workload logs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogsResponse {
    /// Workload ID.
    pub workload_id: Uuid,
    /// Instance ID (if specific instance).
    pub instance_id: Option<Uuid>,
    /// Container ID (if specific container).
    pub container_id: Option<String>,
    /// The log content.
    pub logs: String,
    /// Number of lines returned.
    pub lines: usize,
}

// ============================================================================
// Conversion Helpers
// ============================================================================

impl From<CreateWorkloadRequest> for WorkloadDefinition {
    fn from(req: CreateWorkloadRequest) -> Self {
        WorkloadDefinition {
            id: Uuid::new_v4(),
            name: req.name,
            containers: req.containers.into_iter().map(Into::into).collect(),
            replicas: req.replicas,
            labels: req.labels,
        }
    }
}

impl From<ContainerConfigRequest> for ContainerConfig {
    fn from(req: ContainerConfigRequest) -> Self {
        ContainerConfig {
            name: req.name,
            image: req.image,
            command: req.command,
            args: req.args,
            env_vars: req.env_vars,
            ports: req.ports.into_iter().map(Into::into).collect(),
            resource_requests: req.resource_requests.into(),
        }
    }
}

impl From<PortMappingRequest> for PortMapping {
    fn from(req: PortMappingRequest) -> Self {
        PortMapping {
            container_port: req.container_port,
            host_port: req.host_port,
            protocol: req.protocol,
        }
    }
}

impl From<ResourceRequestsRequest> for NodeResources {
    fn from(req: ResourceRequestsRequest) -> Self {
        NodeResources {
            cpu_cores: req.cpu_cores,
            memory_mb: req.memory_mb,
            disk_mb: req.disk_mb,
        }
    }
}

impl From<WorkloadDefinition> for WorkloadResponse {
    fn from(def: WorkloadDefinition) -> Self {
        WorkloadResponse {
            id: def.id,
            name: def.name,
            replicas: def.replicas,
            labels: def.labels,
            containers: def.containers.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<ContainerConfig> for ContainerConfigResponse {
    fn from(cfg: ContainerConfig) -> Self {
        ContainerConfigResponse {
            name: cfg.name,
            image: cfg.image,
            command: cfg.command,
            args: cfg.args,
            env_vars: cfg.env_vars,
            ports: cfg.ports.into_iter().map(Into::into).collect(),
            resource_requests: cfg.resource_requests.into(),
        }
    }
}

impl From<PortMapping> for PortMappingResponse {
    fn from(pm: PortMapping) -> Self {
        PortMappingResponse {
            container_port: pm.container_port,
            host_port: pm.host_port,
            protocol: pm.protocol,
        }
    }
}

impl From<NodeResources> for ResourceRequestsResponse {
    fn from(res: NodeResources) -> Self {
        ResourceRequestsResponse {
            cpu_cores: res.cpu_cores,
            memory_mb: res.memory_mb,
            disk_mb: res.disk_mb,
        }
    }
}

impl From<Node> for NodeResponse {
    fn from(node: Node) -> Self {
        NodeResponse {
            id: node.id.to_string(),
            address: node.address,
            status: format!("{:?}", node.status),
            labels: node.labels,
            resources_capacity: node.resources_capacity.into(),
            resources_allocatable: node.resources_allocatable.into(),
        }
    }
}

impl From<WorkloadInstance> for InstanceResponse {
    fn from(inst: WorkloadInstance) -> Self {
        InstanceResponse {
            id: inst.id,
            workload_id: inst.workload_id,
            node_id: inst.node_id.to_string(),
            container_ids: inst.container_ids,
            status: format!("{:?}", inst.status),
        }
    }
}

// ============================================================================
// Workload Handlers
// ============================================================================

/// Create a new workload.
pub async fn create_workload(
    State(state): State<ApiState>,
    Json(request): Json<CreateWorkloadRequest>,
) -> ApiResult<impl IntoResponse> {
    // Validate request
    if request.name.is_empty() {
        return Err(ApiError::validation_error("Workload name cannot be empty"));
    }

    if request.containers.is_empty() {
        return Err(ApiError::validation_error(
            "Workload must have at least one container",
        ));
    }

    if request.replicas == 0 {
        return Err(ApiError::validation_error("Replicas must be at least 1"));
    }

    // Convert to workload definition
    let workload: WorkloadDefinition = request.into();

    // Store workload
    state
        .state_store
        .put_workload(workload.clone())
        .await
        .map_err(ApiError::from)?;

    // Send to orchestrator for scheduling
    state
        .workload_tx
        .send(workload.clone())
        .await
        .map_err(|_| ApiError::internal_error("Failed to submit workload to orchestrator"))?;

    let response: WorkloadResponse = workload.into();
    Ok((StatusCode::CREATED, Json(response)))
}

/// List all workloads.
pub async fn list_workloads(State(state): State<ApiState>) -> ApiResult<impl IntoResponse> {
    let workloads = state
        .state_store
        .list_workloads()
        .await
        .map_err(ApiError::from)?;

    let items: Vec<WorkloadResponse> = workloads.into_iter().map(Into::into).collect();
    let count = items.len();

    Ok(Json(ListResponse { items, count }))
}

/// Get a workload by ID.
pub async fn get_workload(
    State(state): State<ApiState>,
    Path(workload_id): Path<Uuid>,
) -> ApiResult<impl IntoResponse> {
    let workload = state
        .state_store
        .get_workload(&workload_id)
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::not_found("Workload", &workload_id.to_string()))?;

    let response: WorkloadResponse = workload.into();
    Ok(Json(response))
}

/// Update a workload.
pub async fn update_workload(
    State(state): State<ApiState>,
    Path(workload_id): Path<Uuid>,
    Json(request): Json<CreateWorkloadRequest>,
) -> ApiResult<impl IntoResponse> {
    // Check workload exists
    let _ = state
        .state_store
        .get_workload(&workload_id)
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::not_found("Workload", &workload_id.to_string()))?;

    // Validate request
    if request.name.is_empty() {
        return Err(ApiError::validation_error("Workload name cannot be empty"));
    }

    if request.containers.is_empty() {
        return Err(ApiError::validation_error(
            "Workload must have at least one container",
        ));
    }

    // Create updated workload with same ID
    let workload = WorkloadDefinition {
        id: workload_id,
        name: request.name,
        containers: request.containers.into_iter().map(Into::into).collect(),
        replicas: request.replicas,
        labels: request.labels,
    };

    // Store updated workload
    state
        .state_store
        .put_workload(workload.clone())
        .await
        .map_err(ApiError::from)?;

    // Send to orchestrator for re-reconciliation
    state
        .workload_tx
        .send(workload.clone())
        .await
        .map_err(|_| {
            ApiError::internal_error("Failed to submit workload update to orchestrator")
        })?;

    let response: WorkloadResponse = workload.into();
    Ok(Json(response))
}

/// Delete a workload.
pub async fn delete_workload(
    State(state): State<ApiState>,
    Path(workload_id): Path<Uuid>,
) -> ApiResult<impl IntoResponse> {
    // Check workload exists
    let _ = state
        .state_store
        .get_workload(&workload_id)
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::not_found("Workload", &workload_id.to_string()))?;

    // Delete instances first
    state
        .state_store
        .delete_instances_for_workload(&workload_id)
        .await
        .map_err(ApiError::from)?;

    // Delete workload
    state
        .state_store
        .delete_workload(&workload_id)
        .await
        .map_err(ApiError::from)?;

    Ok(StatusCode::NO_CONTENT)
}

/// List instances for a workload.
pub async fn list_workload_instances(
    State(state): State<ApiState>,
    Path(workload_id): Path<Uuid>,
) -> ApiResult<impl IntoResponse> {
    // Check workload exists
    let _ = state
        .state_store
        .get_workload(&workload_id)
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::not_found("Workload", &workload_id.to_string()))?;

    let instances = state
        .state_store
        .list_instances_for_workload(&workload_id)
        .await
        .map_err(ApiError::from)?;

    let items: Vec<InstanceResponse> = instances.into_iter().map(Into::into).collect();
    let count = items.len();

    Ok(Json(ListResponse { items, count }))
}

// ============================================================================
// Node Handlers
// ============================================================================

/// List all nodes.
pub async fn list_nodes(State(state): State<ApiState>) -> ApiResult<impl IntoResponse> {
    let nodes = state
        .state_store
        .list_nodes()
        .await
        .map_err(ApiError::from)?;

    let items: Vec<NodeResponse> = nodes.into_iter().map(Into::into).collect();
    let count = items.len();

    Ok(Json(ListResponse { items, count }))
}

/// Get a node by ID.
pub async fn get_node(
    State(state): State<ApiState>,
    Path(node_id_str): Path<String>,
) -> ApiResult<impl IntoResponse> {
    let node_id: NodeId = node_id_str
        .parse()
        .map_err(|_| ApiError::validation_error(&format!("Invalid node ID: {}", node_id_str)))?;

    let node = state
        .state_store
        .get_node(&node_id)
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::not_found("Node", &node_id_str))?;

    let response: NodeResponse = node.into();
    Ok(Json(response))
}

// ============================================================================
// Cluster Handlers
// ============================================================================

/// Get cluster status.
pub async fn get_cluster_status(State(state): State<ApiState>) -> ApiResult<impl IntoResponse> {
    // Get nodes
    let nodes = state
        .state_store
        .list_nodes()
        .await
        .map_err(ApiError::from)?;

    let total_nodes = nodes.len();
    let ready_nodes = nodes
        .iter()
        .filter(|n| n.status == NodeStatus::Ready)
        .count();
    let not_ready_nodes = total_nodes - ready_nodes;

    // Calculate resource totals
    let (total_cpu_capacity, total_memory_mb) = nodes.iter().fold((0.0f32, 0u64), |acc, n| {
        (
            acc.0 + n.resources_capacity.cpu_cores,
            acc.1 + n.resources_capacity.memory_mb,
        )
    });

    let (total_cpu_allocatable, total_memory_allocatable_mb) =
        nodes.iter().fold((0.0f32, 0u64), |acc, n| {
            (
                acc.0 + n.resources_allocatable.cpu_cores,
                acc.1 + n.resources_allocatable.memory_mb,
            )
        });

    // Get workloads
    let workloads = state
        .state_store
        .list_workloads()
        .await
        .map_err(ApiError::from)?;

    let total_workloads = workloads.len();

    // Get all instances
    let instances = state
        .state_store
        .list_all_instances()
        .await
        .map_err(ApiError::from)?;

    let total_instances = instances.len();
    let running_instances = instances
        .iter()
        .filter(|i| i.status == WorkloadInstanceStatus::Running)
        .count();
    let pending_instances = instances
        .iter()
        .filter(|i| i.status == WorkloadInstanceStatus::Pending)
        .count();
    let failed_instances = instances
        .iter()
        .filter(|i| i.status == WorkloadInstanceStatus::Failed)
        .count();

    Ok(Json(ClusterStatusResponse {
        total_nodes,
        ready_nodes,
        not_ready_nodes,
        total_workloads,
        total_instances,
        running_instances,
        pending_instances,
        failed_instances,
        total_cpu_capacity,
        total_memory_mb,
        total_cpu_allocatable,
        total_memory_allocatable_mb,
    }))
}

// ============================================================================
// Log Handlers
// ============================================================================

/// Get logs for a workload.
/// Returns logs from all instances/containers of the workload.
pub async fn get_workload_logs(
    State(state): State<ApiState>,
    Path(workload_id): Path<Uuid>,
    Query(query): Query<LogsQuery>,
) -> ApiResult<impl IntoResponse> {
    // Check runtime is available
    let runtime = state.container_runtime.as_ref().ok_or_else(|| {
        ApiError::internal_error("Container runtime not configured for log access")
    })?;

    // Check workload exists
    let _ = state
        .state_store
        .get_workload(&workload_id)
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::not_found("Workload", &workload_id.to_string()))?;

    // Get instances for workload
    let instances = state
        .state_store
        .list_instances_for_workload(&workload_id)
        .await
        .map_err(ApiError::from)?;

    if instances.is_empty() {
        return Ok(Json(LogsResponse {
            workload_id,
            instance_id: None,
            container_id: None,
            logs: String::new(),
            lines: 0,
        }));
    }

    // Build log options
    let log_options = RuntimeLogOptions {
        tail: query.tail,
        timestamps: query.timestamps,
        since: query.since,
        until: query.until,
    };

    // Collect logs from all containers
    let mut all_logs = Vec::new();

    for instance in &instances {
        for container_id in &instance.container_ids {
            match runtime.get_container_logs(container_id, &log_options).await {
                Ok(logs) if !logs.is_empty() => {
                    all_logs.push(format!("=== Container: {} ===\n{}", container_id, logs));
                }
                Ok(_) => {} // Empty logs
                Err(e) => {
                    tracing::warn!("Failed to get logs for container {}: {}", container_id, e);
                }
            }
        }
    }

    let combined_logs = all_logs.join("\n\n");
    let line_count = combined_logs.lines().count();

    Ok(Json(LogsResponse {
        workload_id,
        instance_id: None,
        container_id: None,
        logs: combined_logs,
        lines: line_count,
    }))
}

/// Get logs for a specific workload instance.
pub async fn get_instance_logs(
    State(state): State<ApiState>,
    Path((workload_id, instance_id)): Path<(Uuid, Uuid)>,
    Query(query): Query<LogsQuery>,
) -> ApiResult<impl IntoResponse> {
    // Check runtime is available
    let runtime = state.container_runtime.as_ref().ok_or_else(|| {
        ApiError::internal_error("Container runtime not configured for log access")
    })?;

    // Check workload exists
    let _ = state
        .state_store
        .get_workload(&workload_id)
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::not_found("Workload", &workload_id.to_string()))?;

    // Get the specific instance
    let instances = state
        .state_store
        .list_instances_for_workload(&workload_id)
        .await
        .map_err(ApiError::from)?;

    let instance = instances
        .into_iter()
        .find(|i| i.id == instance_id)
        .ok_or_else(|| ApiError::not_found("Instance", &instance_id.to_string()))?;

    // Build log options
    let log_options = RuntimeLogOptions {
        tail: query.tail,
        timestamps: query.timestamps,
        since: query.since,
        until: query.until,
    };

    // Collect logs from all containers in this instance
    let mut all_logs = Vec::new();

    for container_id in &instance.container_ids {
        match runtime.get_container_logs(container_id, &log_options).await {
            Ok(logs) if !logs.is_empty() => {
                all_logs.push(format!("=== Container: {} ===\n{}", container_id, logs));
            }
            Ok(_) => {} // Empty logs
            Err(e) => {
                tracing::warn!("Failed to get logs for container {}: {}", container_id, e);
            }
        }
    }

    let combined_logs = all_logs.join("\n\n");
    let line_count = combined_logs.lines().count();

    Ok(Json(LogsResponse {
        workload_id,
        instance_id: Some(instance_id),
        container_id: None,
        logs: combined_logs,
        lines: line_count,
    }))
}

/// WebSocket endpoint for streaming logs in real-time.
/// Connects to the container runtime's log stream and forwards messages.
pub async fn stream_workload_logs(
    State(state): State<ApiState>,
    Path(workload_id): Path<Uuid>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_log_stream(socket, state, workload_id))
}

/// Handle WebSocket connection for log streaming.
async fn handle_log_stream(mut socket: WebSocket, state: ApiState, workload_id: Uuid) {
    // Check runtime is available
    let runtime = match state.container_runtime.as_ref() {
        Some(rt) => rt,
        None => {
            let _ = socket
                .send(Message::Text(
                    r#"{"error":"Container runtime not configured for log streaming"}"#.into(),
                ))
                .await;
            let _ = socket.close().await;
            return;
        }
    };

    // Check workload exists
    match state.state_store.get_workload(&workload_id).await {
        Ok(Some(_)) => {}
        Ok(None) => {
            let _ = socket
                .send(Message::Text(format!(
                    r#"{{"error":"Workload {} not found"}}"#,
                    workload_id
                )))
                .await;
            let _ = socket.close().await;
            return;
        }
        Err(e) => {
            let _ = socket
                .send(Message::Text(format!(
                    r#"{{"error":"Failed to check workload: {}"}}"#,
                    e
                )))
                .await;
            let _ = socket.close().await;
            return;
        }
    }

    // Get instances for workload
    let instances = match state
        .state_store
        .list_instances_for_workload(&workload_id)
        .await
    {
        Ok(instances) => instances,
        Err(e) => {
            let _ = socket
                .send(Message::Text(format!(
                    r#"{{"error":"Failed to list instances: {}"}}"#,
                    e
                )))
                .await;
            let _ = socket.close().await;
            return;
        }
    };

    if instances.is_empty() {
        let _ = socket
            .send(Message::Text(
                r#"{"info":"No instances for this workload"}"#.into(),
            ))
            .await;
        let _ = socket.close().await;
        return;
    }

    // Send initial message
    let _ = socket
        .send(Message::Text(format!(
            r#"{{"status":"connected","workload_id":"{}","instances":{}}}"#,
            workload_id,
            instances.len()
        )))
        .await;

    // Build log options for follow mode
    let log_options = RuntimeLogOptions {
        tail: Some(100), // Start with last 100 lines
        timestamps: true,
        since: None,
        until: None,
    };

    // Stream logs from all containers
    // For now, poll periodically since the runtime interface doesn't expose streaming
    let mut last_line_counts: HashMap<String, usize> = HashMap::new();
    let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(500));

    loop {
        tokio::select! {
            _ = interval.tick() => {
                for instance in &instances {
                    for container_id in &instance.container_ids {
                        match runtime.get_container_logs(container_id, &log_options).await {
                            Ok(logs) if !logs.is_empty() => {
                                let lines: Vec<&str> = logs.lines().collect();
                                let line_count = lines.len();
                                let last_count = *last_line_counts.get(container_id).unwrap_or(&0);

                                if line_count > last_count {
                                    // Send new lines
                                    for line in lines.iter().skip(last_count) {
                                        let msg = serde_json::json!({
                                            "container_id": container_id,
                                            "instance_id": instance.id.to_string(),
                                            "log": line
                                        });
                                        if socket.send(Message::Text(msg.to_string())).await.is_err() {
                                            return; // Client disconnected
                                        }
                                    }
                                    last_line_counts.insert(container_id.clone(), line_count);
                                }
                            }
                            Ok(_) => {} // Empty logs
                            Err(e) => {
                                tracing::debug!("Log fetch error for {}: {}", container_id, e);
                            }
                        }
                    }
                }
            }
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Close(_))) | None => {
                        tracing::debug!("WebSocket closed for workload {}", workload_id);
                        return;
                    }
                    Some(Ok(Message::Ping(data))) => {
                        let _ = socket.send(Message::Pong(data)).await;
                    }
                    Some(Err(e)) => {
                        tracing::debug!("WebSocket error: {}", e);
                        return;
                    }
                    _ => {} // Ignore other messages
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use orchestrator_shared_types::Keypair;

    fn generate_node_id() -> NodeId {
        Keypair::generate().public_key()
    }

    #[test]
    fn test_create_workload_request_conversion() {
        let request = CreateWorkloadRequest {
            name: "test-workload".to_string(),
            containers: vec![ContainerConfigRequest {
                name: "nginx".to_string(),
                image: "nginx:latest".to_string(),
                command: None,
                args: None,
                env_vars: HashMap::new(),
                ports: vec![PortMappingRequest {
                    container_port: 80,
                    host_port: Some(8080),
                    protocol: "tcp".to_string(),
                }],
                resource_requests: ResourceRequestsRequest {
                    cpu_cores: 0.5,
                    memory_mb: 512,
                    disk_mb: 1024,
                },
            }],
            replicas: 3,
            labels: HashMap::new(),
        };

        let workload: WorkloadDefinition = request.into();
        assert_eq!(workload.name, "test-workload");
        assert_eq!(workload.replicas, 3);
        assert_eq!(workload.containers.len(), 1);
        assert_eq!(workload.containers[0].image, "nginx:latest");
    }

    #[test]
    fn test_node_response_conversion() {
        let node_id = generate_node_id();
        let node = Node {
            id: node_id,
            address: "10.0.0.1:8080".to_string(),
            status: NodeStatus::Ready,
            labels: HashMap::new(),
            resources_capacity: NodeResources {
                cpu_cores: 4.0,
                memory_mb: 8192,
                disk_mb: 102400,
            },
            resources_allocatable: NodeResources {
                cpu_cores: 3.6,
                memory_mb: 7372,
                disk_mb: 92160,
            },
        };

        let response: NodeResponse = node.clone().into();
        assert_eq!(response.id, node.id.to_string());
        assert_eq!(response.status, "Ready");
        assert_eq!(response.resources_capacity.cpu_cores, 4.0);
    }

    #[test]
    fn test_instance_response_conversion() {
        let instance = WorkloadInstance {
            id: Uuid::new_v4(),
            workload_id: Uuid::new_v4(),
            node_id: generate_node_id(),
            container_ids: vec!["container-1".to_string()],
            status: WorkloadInstanceStatus::Running,
        };

        let response: InstanceResponse = instance.clone().into();
        assert_eq!(response.id, instance.id);
        assert_eq!(response.status, "Running");
    }
}
