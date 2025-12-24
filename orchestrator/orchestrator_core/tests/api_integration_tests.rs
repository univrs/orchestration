//! Integration tests for the REST API.
//!
//! Tests the full API flow including authentication, CRUD operations,
//! and cluster status.

use std::collections::HashMap;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use base64::prelude::*;
use chrono::Utc;
use ed25519_dalek::{SigningKey, Signer};
use rand::rngs::OsRng;
use sha2::{Sha256, Digest};
use tokio::sync::mpsc;
use tower::ServiceExt;
use uuid::Uuid;

use orchestrator_shared_types::{
    Node, NodeResources, NodeStatus, WorkloadDefinition,
};
use state_store_interface::StateStore;

// Import API modules (only available with rest-api feature)
#[cfg(feature = "rest-api")]
use orchestrator_core::api::{
    build_router, ApiState, AuthConfig,
    handlers::{
        ListResponse, NodeResponse, WorkloadResponse, ClusterStatusResponse,
    },
};

// ============================================================================
// Test Helpers
// ============================================================================

#[cfg(feature = "rest-api")]
mod mock {
    use async_trait::async_trait;
    use cluster_manager_interface::{ClusterEvent, ClusterManager};
    use orchestrator_shared_types::{Node, NodeId, Result};
    use tokio::sync::watch;

    #[derive(Default)]
    pub struct MockClusterManager;

    #[async_trait]
    impl ClusterManager for MockClusterManager {
        async fn initialize(&self) -> Result<()> {
            Ok(())
        }

        async fn get_node(&self, _node_id: &NodeId) -> Result<Option<Node>> {
            Ok(None)
        }

        async fn list_nodes(&self) -> Result<Vec<Node>> {
            Ok(vec![])
        }

        async fn subscribe_to_events(&self) -> Result<watch::Receiver<Option<ClusterEvent>>> {
            let (_, rx) = watch::channel(None);
            Ok(rx)
        }
    }
}

#[cfg(feature = "rest-api")]
fn create_test_state() -> (ApiState, mpsc::Receiver<WorkloadDefinition>) {
    use state_store_interface::in_memory::InMemoryStateStore;

    let state_store: Arc<dyn state_store_interface::StateStore> =
        Arc::new(InMemoryStateStore::new());
    let cluster_manager: Arc<dyn cluster_manager_interface::ClusterManager> =
        Arc::new(mock::MockClusterManager);
    let (workload_tx, workload_rx) = mpsc::channel::<WorkloadDefinition>(100);

    let state = ApiState::new_without_auth(state_store, cluster_manager, workload_tx);
    (state, workload_rx)
}

#[cfg(feature = "rest-api")]
fn create_test_state_with_auth(trusted_key: Option<[u8; 32]>) -> (ApiState, mpsc::Receiver<WorkloadDefinition>, SigningKey) {
    use state_store_interface::in_memory::InMemoryStateStore;

    let signing_key = SigningKey::generate(&mut OsRng);
    let public_key = *signing_key.verifying_key().as_bytes();

    let state_store: Arc<dyn state_store_interface::StateStore> =
        Arc::new(InMemoryStateStore::new());
    let cluster_manager: Arc<dyn cluster_manager_interface::ClusterManager> =
        Arc::new(mock::MockClusterManager);
    let (workload_tx, workload_rx) = mpsc::channel::<WorkloadDefinition>(100);

    let mut auth_config = AuthConfig::default();
    if let Some(key) = trusted_key {
        auth_config = auth_config.with_trusted_key(key);
    } else {
        // Trust the generated key
        auth_config = auth_config.with_trusted_key(public_key);
    }

    let state = ApiState::new(state_store, cluster_manager, workload_tx, auth_config);
    (state, workload_rx, signing_key)
}

#[cfg(feature = "rest-api")]
fn sign_request(
    method: &str,
    path: &str,
    body: &[u8],
    signing_key: &SigningKey,
) -> (String, String, String) {
    let timestamp = Utc::now().to_rfc3339();
    let body_hash = hex::encode(Sha256::digest(body));
    let signing_string = format!("{}\n{}\n{}\n{}", method, path, timestamp, body_hash);

    let signature = signing_key.sign(signing_string.as_bytes());
    let public_key = BASE64_STANDARD.encode(signing_key.verifying_key().as_bytes());
    let signature_b64 = BASE64_STANDARD.encode(signature.to_bytes());

    (public_key, timestamp, signature_b64)
}

#[cfg(feature = "rest-api")]
fn create_workload_json() -> String {
    serde_json::json!({
        "name": "test-workload",
        "containers": [{
            "name": "nginx",
            "image": "nginx:latest",
            "ports": [{"container_port": 80, "host_port": 8080, "protocol": "tcp"}],
            "resource_requests": {
                "cpu_cores": 0.5,
                "memory_mb": 512,
                "disk_mb": 1024
            }
        }],
        "replicas": 2,
        "labels": {"app": "test"}
    }).to_string()
}

// ============================================================================
// Tests (only compiled with rest-api feature)
// ============================================================================

#[cfg(feature = "rest-api")]
#[tokio::test]
async fn test_create_workload_without_auth() {
    let (state, mut workload_rx) = create_test_state();
    let router = build_router(state);

    let body = create_workload_json();

    let response = router
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/workloads")
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    // Check workload was sent to orchestrator
    let workload = workload_rx.try_recv().unwrap();
    assert_eq!(workload.name, "test-workload");
    assert_eq!(workload.replicas, 2);
}

#[cfg(feature = "rest-api")]
#[tokio::test]
async fn test_create_workload_with_auth() {
    let (state, mut workload_rx, signing_key) = create_test_state_with_auth(None);
    let router = build_router(state);

    let body = create_workload_json();
    let (pub_key, timestamp, signature) = sign_request("POST", "/api/v1/workloads", body.as_bytes(), &signing_key);

    let response = router
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/workloads")
                .header("content-type", "application/json")
                .header("X-Auth-PublicKey", pub_key)
                .header("X-Auth-Timestamp", timestamp)
                .header("X-Auth-Signature", signature)
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    // Check workload was created
    let workload = workload_rx.try_recv().unwrap();
    assert_eq!(workload.name, "test-workload");
}

#[cfg(feature = "rest-api")]
#[tokio::test]
async fn test_auth_failure_missing_headers() {
    let (state, _workload_rx, _signing_key) = create_test_state_with_auth(None);
    let router = build_router(state);

    let body = create_workload_json();

    // No auth headers
    let response = router
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/workloads")
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[cfg(feature = "rest-api")]
#[tokio::test]
async fn test_auth_failure_invalid_signature() {
    let (state, _workload_rx, signing_key) = create_test_state_with_auth(None);
    let router = build_router(state);

    let body = create_workload_json();
    let (pub_key, timestamp, _signature) = sign_request("POST", "/api/v1/workloads", body.as_bytes(), &signing_key);

    // Use wrong signature
    let wrong_signature = BASE64_STANDARD.encode([0u8; 64]);

    let response = router
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/workloads")
                .header("content-type", "application/json")
                .header("X-Auth-PublicKey", pub_key)
                .header("X-Auth-Timestamp", timestamp)
                .header("X-Auth-Signature", wrong_signature)
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[cfg(feature = "rest-api")]
#[tokio::test]
async fn test_list_workloads_empty() {
    let (state, _workload_rx) = create_test_state();
    let router = build_router(state);

    let response = router
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/workloads")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    let list: ListResponse<WorkloadResponse> = serde_json::from_slice(&body).unwrap();
    assert_eq!(list.count, 0);
    assert!(list.items.is_empty());
}

#[cfg(feature = "rest-api")]
#[tokio::test]
async fn test_list_nodes_empty() {
    let (state, _workload_rx) = create_test_state();
    let router = build_router(state);

    let response = router
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/nodes")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    let list: ListResponse<NodeResponse> = serde_json::from_slice(&body).unwrap();
    assert_eq!(list.count, 0);
}

#[cfg(feature = "rest-api")]
#[tokio::test]
async fn test_cluster_status_empty() {
    let (state, _workload_rx) = create_test_state();
    let router = build_router(state);

    let response = router
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/cluster/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    let status: ClusterStatusResponse = serde_json::from_slice(&body).unwrap();
    assert_eq!(status.total_nodes, 0);
    assert_eq!(status.total_workloads, 0);
}

#[cfg(feature = "rest-api")]
#[tokio::test]
async fn test_get_workload_not_found() {
    let (state, _workload_rx) = create_test_state();
    let router = build_router(state);

    let fake_id = Uuid::new_v4();

    let response = router
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/v1/workloads/{}", fake_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[cfg(feature = "rest-api")]
#[tokio::test]
async fn test_workload_crud_flow() {
    use state_store_interface::in_memory::InMemoryStateStore;

    // Use a real state store to test persistence
    let state_store: Arc<dyn state_store_interface::StateStore> =
        Arc::new(InMemoryStateStore::new());
    let cluster_manager: Arc<dyn cluster_manager_interface::ClusterManager> =
        Arc::new(mock::MockClusterManager);
    let (workload_tx, _workload_rx) = mpsc::channel::<WorkloadDefinition>(100);

    let state = ApiState::new_without_auth(
        state_store.clone(),
        cluster_manager,
        workload_tx,
    );
    let router = build_router(state);

    // 1. Create workload
    let create_body = create_workload_json();
    let create_response = router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/workloads")
                .header("content-type", "application/json")
                .body(Body::from(create_body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(create_response.status(), StatusCode::CREATED);

    let body = axum::body::to_bytes(create_response.into_body(), 1024 * 1024).await.unwrap();
    let created: WorkloadResponse = serde_json::from_slice(&body).unwrap();
    let workload_id = created.id;

    // 2. Get workload
    let get_response = router
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/v1/workloads/{}", workload_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(get_response.status(), StatusCode::OK);

    // 3. Update workload
    let update_body = serde_json::json!({
        "name": "updated-workload",
        "containers": [{
            "name": "nginx",
            "image": "nginx:1.21",
            "ports": [],
            "resource_requests": {"cpu_cores": 1.0, "memory_mb": 1024, "disk_mb": 2048}
        }],
        "replicas": 5,
        "labels": {"app": "updated"}
    }).to_string();

    let update_response = router
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/v1/workloads/{}", workload_id))
                .header("content-type", "application/json")
                .body(Body::from(update_body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(update_response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(update_response.into_body(), 1024 * 1024).await.unwrap();
    let updated: WorkloadResponse = serde_json::from_slice(&body).unwrap();
    assert_eq!(updated.name, "updated-workload");
    assert_eq!(updated.replicas, 5);

    // 4. List workloads
    let list_response = router
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/workloads")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(list_response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(list_response.into_body(), 1024 * 1024).await.unwrap();
    let list: ListResponse<WorkloadResponse> = serde_json::from_slice(&body).unwrap();
    assert_eq!(list.count, 1);

    // 5. Delete workload
    let delete_response = router
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/v1/workloads/{}", workload_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(delete_response.status(), StatusCode::NO_CONTENT);

    // 6. Verify deleted
    let get_deleted_response = router
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/v1/workloads/{}", workload_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(get_deleted_response.status(), StatusCode::NOT_FOUND);
}

#[cfg(feature = "rest-api")]
#[tokio::test]
async fn test_validation_errors() {
    let (state, _workload_rx) = create_test_state();
    let router = build_router(state);

    // Empty name
    let invalid_body = serde_json::json!({
        "name": "",
        "containers": [{"name": "test", "image": "test:latest"}],
        "replicas": 1
    }).to_string();

    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/workloads")
                .header("content-type", "application/json")
                .body(Body::from(invalid_body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    // No containers
    let no_containers = serde_json::json!({
        "name": "test",
        "containers": [],
        "replicas": 1
    }).to_string();

    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/workloads")
                .header("content-type", "application/json")
                .body(Body::from(no_containers))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    // Zero replicas
    let zero_replicas = serde_json::json!({
        "name": "test",
        "containers": [{"name": "test", "image": "test:latest"}],
        "replicas": 0
    }).to_string();

    let response = router
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/workloads")
                .header("content-type", "application/json")
                .body(Body::from(zero_replicas))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[cfg(feature = "rest-api")]
#[tokio::test]
async fn test_cluster_status_with_nodes() {
    use state_store_interface::in_memory::InMemoryStateStore;

    let state_store = Arc::new(InMemoryStateStore::new());

    // Add some nodes to state store
    let node1 = Node {
        id: Uuid::new_v4(),
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

    let node2 = Node {
        id: Uuid::new_v4(),
        address: "10.0.0.2:8080".to_string(),
        status: NodeStatus::NotReady,
        labels: HashMap::new(),
        resources_capacity: NodeResources {
            cpu_cores: 2.0,
            memory_mb: 4096,
            disk_mb: 51200,
        },
        resources_allocatable: NodeResources {
            cpu_cores: 1.8,
            memory_mb: 3686,
            disk_mb: 46080,
        },
    };

    state_store.put_node(node1).await.unwrap();
    state_store.put_node(node2).await.unwrap();

    let cluster_manager: Arc<dyn cluster_manager_interface::ClusterManager> =
        Arc::new(mock::MockClusterManager);
    let (workload_tx, _workload_rx) = mpsc::channel::<WorkloadDefinition>(100);

    let state = ApiState::new_without_auth(
        state_store as Arc<dyn state_store_interface::StateStore>,
        cluster_manager,
        workload_tx,
    );
    let router = build_router(state);

    let response = router
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/cluster/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    let status: ClusterStatusResponse = serde_json::from_slice(&body).unwrap();

    assert_eq!(status.total_nodes, 2);
    assert_eq!(status.ready_nodes, 1);
    assert_eq!(status.not_ready_nodes, 1);
    assert_eq!(status.total_cpu_capacity, 6.0);
    assert_eq!(status.total_memory_mb, 12288);
}
