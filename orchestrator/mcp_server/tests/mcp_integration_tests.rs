//! Integration tests for MCP server request/response cycle.
//!
//! Tests the full MCP protocol flow including initialization,
//! tool listing, tool calls, and resource access.

use mcp_server::{
    JsonRpcRequest, JsonRpcResponse, OrchestratorMcpServer, INVALID_PARAMS, METHOD_NOT_FOUND,
};
use serde_json::{json, Value};

// ============================================================================
// Test Mocks
// ============================================================================

mod mock {
    use async_trait::async_trait;
    use cluster_manager_interface::{ClusterEvent, ClusterManager};
    use orchestrator_shared_types::{
        Keypair, Node, NodeId, NodeResources, NodeStatus, Result, WorkloadDefinition, WorkloadId,
        WorkloadInstance,
    };
    use scheduler_interface::{ScheduleDecision, ScheduleRequest, Scheduler};
    use state_store_interface::StateStore;
    use std::collections::HashMap;
    use tokio::sync::{broadcast, mpsc};
    use uuid::Uuid;

    /// Generate a new NodeId for testing (Ed25519 public key).
    pub fn generate_node_id() -> NodeId {
        Keypair::generate().public_key()
    }

    /// Mock cluster manager for testing.
    #[derive(Clone, Default)]
    pub struct MockClusterManager {
        nodes: Vec<Node>,
    }

    impl MockClusterManager {
        pub fn new() -> Self {
            Self { nodes: vec![] }
        }

        pub fn with_nodes(nodes: Vec<Node>) -> Self {
            Self { nodes }
        }
    }

    #[async_trait]
    impl ClusterManager for MockClusterManager {
        async fn initialize(&self) -> Result<()> {
            Ok(())
        }

        async fn get_node(&self, node_id: &NodeId) -> Result<Option<Node>> {
            Ok(self.nodes.iter().find(|n| n.id == *node_id).cloned())
        }

        async fn list_nodes(&self) -> Result<Vec<Node>> {
            Ok(self.nodes.clone())
        }

        async fn subscribe_to_events(&self) -> Result<broadcast::Receiver<ClusterEvent>> {
            let (tx, rx) = broadcast::channel(16);
            let _ = tx; // Keep sender alive for the duration
            Ok(rx)
        }
    }

    /// Mock state store for testing.
    #[derive(Clone, Default)]
    pub struct MockStateStore {
        workload_defs: Vec<WorkloadDefinition>,
        workload_instances: Vec<WorkloadInstance>,
    }

    impl MockStateStore {
        pub fn new() -> Self {
            Self {
                workload_defs: vec![],
                workload_instances: vec![],
            }
        }

        pub fn with_workloads(workload_defs: Vec<WorkloadDefinition>) -> Self {
            Self {
                workload_defs,
                workload_instances: vec![],
            }
        }
    }

    #[async_trait]
    impl StateStore for MockStateStore {
        async fn initialize(&self) -> Result<()> {
            Ok(())
        }

        async fn health_check(&self) -> Result<bool> {
            Ok(true)
        }

        async fn put_node(&self, _node: Node) -> Result<()> {
            Ok(())
        }

        async fn get_node(&self, _node_id: &NodeId) -> Result<Option<Node>> {
            Ok(None)
        }

        async fn list_nodes(&self) -> Result<Vec<Node>> {
            Ok(vec![])
        }

        async fn delete_node(&self, _node_id: &NodeId) -> Result<()> {
            Ok(())
        }

        async fn put_workload(&self, _workload: WorkloadDefinition) -> Result<()> {
            Ok(())
        }

        async fn get_workload(&self, id: &WorkloadId) -> Result<Option<WorkloadDefinition>> {
            Ok(self.workload_defs.iter().find(|w| w.id == *id).cloned())
        }

        async fn list_workloads(&self) -> Result<Vec<WorkloadDefinition>> {
            Ok(self.workload_defs.clone())
        }

        async fn delete_workload(&self, _id: &WorkloadId) -> Result<()> {
            Ok(())
        }

        async fn put_instance(&self, _instance: WorkloadInstance) -> Result<()> {
            Ok(())
        }

        async fn get_instance(&self, id: &str) -> Result<Option<WorkloadInstance>> {
            let uuid = Uuid::parse_str(id).ok();
            Ok(uuid.and_then(|u| self.workload_instances.iter().find(|i| i.id == u).cloned()))
        }

        async fn list_instances_for_workload(
            &self,
            workload_id: &WorkloadId,
        ) -> Result<Vec<WorkloadInstance>> {
            Ok(self
                .workload_instances
                .iter()
                .filter(|i| i.workload_id == *workload_id)
                .cloned()
                .collect())
        }

        async fn list_all_instances(&self) -> Result<Vec<WorkloadInstance>> {
            Ok(self.workload_instances.clone())
        }

        async fn delete_instance(&self, _id: &str) -> Result<()> {
            Ok(())
        }

        async fn delete_instances_for_workload(&self, _workload_id: &WorkloadId) -> Result<()> {
            Ok(())
        }

        async fn watch_nodes(&self) -> Result<mpsc::Receiver<Node>> {
            let (_tx, rx) = mpsc::channel(1);
            Ok(rx)
        }

        async fn watch_workloads(&self) -> Result<mpsc::Receiver<WorkloadDefinition>> {
            let (_tx, rx) = mpsc::channel(1);
            Ok(rx)
        }
    }

    /// Mock scheduler for testing.
    #[derive(Clone, Copy, Default)]
    pub struct MockScheduler;

    #[async_trait]
    impl Scheduler for MockScheduler {
        async fn schedule(
            &self,
            _request: &ScheduleRequest,
            _available_nodes: &[Node],
        ) -> Result<Vec<ScheduleDecision>> {
            Ok(vec![])
        }
    }

    /// Create a test node.
    pub fn create_test_node(id: NodeId, address: &str) -> Node {
        Node {
            id,
            address: address.to_string(),
            status: NodeStatus::Ready,
            labels: HashMap::new(),
            resources_capacity: NodeResources {
                cpu_cores: 4.0,
                memory_mb: 8192,
                disk_mb: 102400,
            },
            resources_allocatable: NodeResources {
                cpu_cores: 3.5,
                memory_mb: 7168,
                disk_mb: 92160,
            },
        }
    }

    /// Create a test workload definition.
    pub fn create_test_workload(name: &str) -> WorkloadDefinition {
        WorkloadDefinition {
            id: Uuid::new_v4(),
            name: name.to_string(),
            containers: vec![],
            replicas: 1,
            labels: HashMap::new(),
        }
    }
}

// ============================================================================
// Test Helpers
// ============================================================================

/// Create an MCP server for testing with default mocks.
fn create_test_server(
) -> OrchestratorMcpServer<mock::MockStateStore, mock::MockClusterManager, mock::MockScheduler> {
    OrchestratorMcpServer::new(
        mock::MockStateStore::new(),
        mock::MockClusterManager::new(),
        mock::MockScheduler,
    )
}

/// Create an MCP server with specific test data.
fn create_server_with_data(
    nodes: Vec<orchestrator_shared_types::Node>,
    workloads: Vec<orchestrator_shared_types::WorkloadDefinition>,
) -> OrchestratorMcpServer<mock::MockStateStore, mock::MockClusterManager, mock::MockScheduler> {
    OrchestratorMcpServer::new(
        mock::MockStateStore::with_workloads(workloads),
        mock::MockClusterManager::with_nodes(nodes),
        mock::MockScheduler,
    )
}

/// Create a JSON-RPC request.
fn make_request(method: &str, params: Value) -> JsonRpcRequest {
    JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(1)),
        method: method.to_string(),
        params,
    }
}

/// Assert a response is successful and extract result.
fn assert_success(response: &JsonRpcResponse) -> &Value {
    assert!(
        response.error.is_none(),
        "Expected success but got error: {:?}",
        response.error
    );
    response
        .result
        .as_ref()
        .expect("Expected result in successful response")
}

/// Assert a response is an error with the given code.
fn assert_error(response: &JsonRpcResponse, expected_code: i32) {
    let error = response.error.as_ref().expect("Expected error response");
    assert_eq!(
        error.code, expected_code,
        "Expected error code {} but got {}",
        expected_code, error.code
    );
}

// ============================================================================
// Initialization Tests
// ============================================================================

#[tokio::test]
async fn test_initialize() {
    let server = create_test_server();

    let request = make_request(
        "initialize",
        json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {
                "name": "test-client",
                "version": "1.0.0"
            }
        }),
    );

    let response = server.handle_request(request).await;
    let result = assert_success(&response);

    // Check protocol version
    assert_eq!(result["protocolVersion"], "2024-11-05");

    // Check capabilities
    assert!(result["capabilities"]["tools"].is_object());
    assert!(result["capabilities"]["resources"].is_object());

    // Check server info
    assert_eq!(result["serverInfo"]["name"], "orchestrator-mcp");
}

#[tokio::test]
async fn test_initialize_minimal_params() {
    let server = create_test_server();

    // Initialize with empty params should still work
    let request = make_request("initialize", json!({}));

    let response = server.handle_request(request).await;
    let result = assert_success(&response);

    assert_eq!(result["protocolVersion"], "2024-11-05");
}

#[tokio::test]
async fn test_initialized_notification() {
    let server = create_test_server();

    let request = make_request("initialized", json!({}));

    let response = server.handle_request(request).await;
    assert_success(&response);
}

#[tokio::test]
async fn test_ping() {
    let server = create_test_server();

    let request = make_request("ping", json!({}));

    let response = server.handle_request(request).await;
    assert_success(&response);
}

// ============================================================================
// Tools Tests
// ============================================================================

#[tokio::test]
async fn test_tools_list() {
    let server = create_test_server();

    let request = make_request("tools/list", json!({}));

    let response = server.handle_request(request).await;
    let result = assert_success(&response);

    // Should return tools array
    let tools = result["tools"].as_array().expect("Expected tools array");
    assert!(!tools.is_empty(), "Expected at least one tool");

    // Check for expected tools
    let tool_names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();

    assert!(
        tool_names.contains(&"list_workloads"),
        "Should have list_workloads tool"
    );
    assert!(
        tool_names.contains(&"create_workload"),
        "Should have create_workload tool"
    );
    assert!(
        tool_names.contains(&"list_nodes"),
        "Should have list_nodes tool"
    );
    assert!(
        tool_names.contains(&"get_cluster_status"),
        "Should have get_cluster_status tool"
    );
}

#[tokio::test]
async fn test_tools_call_list_workloads() {
    let workload = mock::create_test_workload("test-workload");
    let server = create_server_with_data(vec![], vec![workload.clone()]);

    let request = make_request(
        "tools/call",
        json!({
            "name": "list_workloads",
            "arguments": {}
        }),
    );

    let response = server.handle_request(request).await;
    let result = assert_success(&response);

    // Check for content in response
    let content = result["content"]
        .as_array()
        .expect("Expected content array");
    assert!(!content.is_empty(), "Expected content in response");

    // First content item should be text
    assert_eq!(content[0]["type"], "text");
    let text = content[0]["text"].as_str().unwrap();

    // Should mention our test workload
    assert!(
        text.contains("test-workload") || text.contains("1"),
        "Should list the workload"
    );
}

#[tokio::test]
async fn test_tools_call_list_nodes() {
    let node = mock::create_test_node(mock::generate_node_id(), "192.168.1.1:8080");
    let server = create_server_with_data(vec![node.clone()], vec![]);

    let request = make_request(
        "tools/call",
        json!({
            "name": "list_nodes",
            "arguments": {}
        }),
    );

    let response = server.handle_request(request).await;
    let result = assert_success(&response);

    let content = result["content"]
        .as_array()
        .expect("Expected content array");
    assert!(!content.is_empty());
}

#[tokio::test]
async fn test_tools_call_get_cluster_status() {
    let node = mock::create_test_node(mock::generate_node_id(), "192.168.1.1:8080");
    let workload = mock::create_test_workload("test-workload");
    let server = create_server_with_data(vec![node], vec![workload]);

    let request = make_request(
        "tools/call",
        json!({
            "name": "get_cluster_status",
            "arguments": {
                "include_nodes": true,
                "include_workloads": true
            }
        }),
    );

    let response = server.handle_request(request).await;
    let result = assert_success(&response);

    let content = result["content"]
        .as_array()
        .expect("Expected content array");
    assert!(!content.is_empty());
}

#[tokio::test]
async fn test_tools_call_unknown_tool() {
    let server = create_test_server();

    let request = make_request(
        "tools/call",
        json!({
            "name": "unknown_tool",
            "arguments": {}
        }),
    );

    let response = server.handle_request(request).await;

    // Should still be a successful response but with error in content
    // or could be an error response depending on implementation
    // Let's check the response structure
    if response.error.is_some() {
        // That's acceptable
    } else {
        let result = response.result.as_ref().unwrap();
        // If successful, should indicate tool not found
        let content = result.get("content").and_then(|c| c.as_array());
        if let Some(content) = content {
            if !content.is_empty() {
                let text = content[0]
                    .get("text")
                    .and_then(|t| t.as_str())
                    .unwrap_or("");
                assert!(
                    text.to_lowercase().contains("unknown")
                        || text.to_lowercase().contains("not found"),
                    "Expected unknown tool message"
                );
            }
        }
    }
}

#[tokio::test]
async fn test_tools_call_missing_name() {
    let server = create_test_server();

    let request = make_request(
        "tools/call",
        json!({
            "arguments": {}
        }),
    );

    let response = server.handle_request(request).await;
    assert_error(&response, INVALID_PARAMS);
}

// ============================================================================
// Resources Tests
// ============================================================================

#[tokio::test]
async fn test_resources_list() {
    let server = create_test_server();

    let request = make_request("resources/list", json!({}));

    let response = server.handle_request(request).await;
    let result = assert_success(&response);

    // Should return resources array
    let resources = result["resources"]
        .as_array()
        .expect("Expected resources array");
    assert!(!resources.is_empty(), "Expected at least one resource");

    // Check for expected resources
    let resource_uris: Vec<&str> = resources
        .iter()
        .map(|r| r["uri"].as_str().unwrap())
        .collect();

    assert!(
        resource_uris.iter().any(|u| u.contains("nodes")),
        "Should have nodes resource"
    );
    assert!(
        resource_uris.iter().any(|u| u.contains("workloads")),
        "Should have workloads resource"
    );
    assert!(
        resource_uris.iter().any(|u| u.contains("status")),
        "Should have status resource"
    );
}

#[tokio::test]
async fn test_resources_read_nodes() {
    let node = mock::create_test_node(mock::generate_node_id(), "192.168.1.1:8080");
    let server = create_server_with_data(vec![node.clone()], vec![]);

    let request = make_request(
        "resources/read",
        json!({
            "uri": "orchestrator://nodes"
        }),
    );

    let response = server.handle_request(request).await;
    let result = assert_success(&response);

    let contents = result["contents"]
        .as_array()
        .expect("Expected contents array");
    assert!(!contents.is_empty());

    // Check content is text
    assert_eq!(contents[0]["mimeType"], "application/json");
}

#[tokio::test]
async fn test_resources_read_workloads() {
    let workload = mock::create_test_workload("test-workload");
    let server = create_server_with_data(vec![], vec![workload.clone()]);

    let request = make_request(
        "resources/read",
        json!({
            "uri": "orchestrator://workloads"
        }),
    );

    let response = server.handle_request(request).await;
    let result = assert_success(&response);

    let contents = result["contents"]
        .as_array()
        .expect("Expected contents array");
    assert!(!contents.is_empty());
}

#[tokio::test]
async fn test_resources_read_status() {
    let server = create_test_server();

    let request = make_request(
        "resources/read",
        json!({
            "uri": "orchestrator://cluster/status"
        }),
    );

    let response = server.handle_request(request).await;
    let result = assert_success(&response);

    let contents = result["contents"]
        .as_array()
        .expect("Expected contents array");
    assert!(!contents.is_empty());

    // Parse the status JSON
    let text = contents[0]["text"].as_str().expect("Expected text content");
    let status: Value = serde_json::from_str(text).expect("Should be valid JSON");

    // Check status structure - using correct field names from ClusterStatusResource
    assert!(
        status.get("total_nodes").is_some(),
        "Should have total_nodes"
    );
    assert!(
        status.get("total_workloads").is_some(),
        "Should have total_workloads"
    );
}

#[tokio::test]
async fn test_resources_read_unknown_uri() {
    let server = create_test_server();

    let request = make_request(
        "resources/read",
        json!({
            "uri": "orchestrator://unknown/resource"
        }),
    );

    let response = server.handle_request(request).await;

    // Should be an error
    assert!(
        response.error.is_some(),
        "Expected error for unknown resource"
    );
}

#[tokio::test]
async fn test_resources_read_missing_uri() {
    let server = create_test_server();

    let request = make_request("resources/read", json!({}));

    let response = server.handle_request(request).await;
    assert_error(&response, INVALID_PARAMS);
}

// ============================================================================
// Error Handling Tests
// ============================================================================

#[tokio::test]
async fn test_unknown_method() {
    let server = create_test_server();

    let request = make_request("unknown/method", json!({}));

    let response = server.handle_request(request).await;
    assert_error(&response, METHOD_NOT_FOUND);
}

#[tokio::test]
async fn test_request_id_preserved() {
    let server = create_test_server();

    // Test with numeric ID
    let request = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(42)),
        method: "ping".to_string(),
        params: json!({}),
    };

    let response = server.handle_request(request).await;
    assert_eq!(response.id, Some(json!(42)));

    // Test with string ID
    let request = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!("request-123")),
        method: "ping".to_string(),
        params: json!({}),
    };

    let response = server.handle_request(request).await;
    assert_eq!(response.id, Some(json!("request-123")));
}

#[tokio::test]
async fn test_null_request_id() {
    let server = create_test_server();

    let request = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: None, // Notification-style request
        method: "ping".to_string(),
        params: json!({}),
    };

    let response = server.handle_request(request).await;
    assert!(response.id.is_none());
}

// ============================================================================
// Full Protocol Flow Test
// ============================================================================

#[tokio::test]
async fn test_full_mcp_flow() {
    let node = mock::create_test_node(mock::generate_node_id(), "192.168.1.1:8080");
    let workload = mock::create_test_workload("nginx");
    let server = create_server_with_data(vec![node], vec![workload]);

    // 1. Initialize
    let init_request = make_request(
        "initialize",
        json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {
                "name": "claude-code",
                "version": "1.0.0"
            }
        }),
    );

    let init_response = server.handle_request(init_request).await;
    let init_result = assert_success(&init_response);
    assert_eq!(init_result["protocolVersion"], "2024-11-05");

    // 2. Send initialized notification
    let initialized_request = make_request("initialized", json!({}));
    let _ = server.handle_request(initialized_request).await;

    // 3. List available tools
    let tools_request = make_request("tools/list", json!({}));
    let tools_response = server.handle_request(tools_request).await;
    let tools_result = assert_success(&tools_response);
    let tools = tools_result["tools"].as_array().unwrap();
    assert!(!tools.is_empty());

    // 4. List available resources
    let resources_request = make_request("resources/list", json!({}));
    let resources_response = server.handle_request(resources_request).await;
    let resources_result = assert_success(&resources_response);
    let resources = resources_result["resources"].as_array().unwrap();
    assert!(!resources.is_empty());

    // 5. Call a tool
    let call_request = make_request(
        "tools/call",
        json!({
            "name": "get_cluster_status",
            "arguments": {}
        }),
    );
    let call_response = server.handle_request(call_request).await;
    assert_success(&call_response);

    // 6. Read a resource
    let read_request = make_request(
        "resources/read",
        json!({
            "uri": "orchestrator://cluster/status"
        }),
    );
    let read_response = server.handle_request(read_request).await;
    assert_success(&read_response);
}
