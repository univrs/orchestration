//! Integration tests for the Orchestrator reconciliation loop
//!
//! These tests verify the end-to-end behavior of:
//! - Workload submission and instance scheduling
//! - Scale up (increasing replicas)
//! - Scale down (decreasing replicas)
//! - Node add/remove events
//! - Reconciliation with no available nodes
//! - Multiple concurrent workloads

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use tokio::sync::{watch, RwLock};
use tokio::time::Duration;
use uuid::Uuid;

use cluster_manager_interface::{ClusterEvent, ClusterManager};
use container_runtime_interface::{ContainerRuntime, ContainerStatus, CreateContainerOptions};
use orchestrator_core::start_orchestrator_service;
use orchestrator_shared_types::{
    ContainerConfig, ContainerId, Node, NodeId, NodeResources, NodeStatus,
    OrchestrationError, Result as OrchResult, WorkloadDefinition, PortMapping, Keypair,
};
use scheduler_interface::SimpleScheduler;
use state_store_interface::in_memory::InMemoryStateStore;
use state_store_interface::StateStore;

// ============================================================================
// Test Mock Implementations
// ============================================================================

/// Mock container runtime that tracks all container operations
#[derive(Debug, Default)]
struct MockContainerRuntime {
    containers: Arc<RwLock<HashMap<ContainerId, MockContainer>>>,
    create_count: Arc<AtomicUsize>,
    stop_count: Arc<AtomicUsize>,
    remove_count: Arc<AtomicUsize>,
    init_node_count: Arc<AtomicUsize>,
    should_fail_create: Arc<RwLock<bool>>,
}

#[derive(Debug, Clone)]
struct MockContainer {
    id: ContainerId,
    config: ContainerConfig,
    state: String,
    node_id: NodeId,
}

impl MockContainerRuntime {
    fn new() -> Self {
        Self::default()
    }

    async fn get_create_count(&self) -> usize {
        self.create_count.load(Ordering::SeqCst)
    }

    async fn get_stop_count(&self) -> usize {
        self.stop_count.load(Ordering::SeqCst)
    }

    async fn get_remove_count(&self) -> usize {
        self.remove_count.load(Ordering::SeqCst)
    }

    async fn get_container_count(&self) -> usize {
        self.containers.read().await.len()
    }

    async fn set_should_fail_create(&self, fail: bool) {
        *self.should_fail_create.write().await = fail;
    }
}

#[async_trait]
impl ContainerRuntime for MockContainerRuntime {
    async fn init_node(&self, _node_id: NodeId) -> OrchResult<()> {
        self.init_node_count.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    async fn create_container(
        &self,
        config: &ContainerConfig,
        options: &CreateContainerOptions,
    ) -> OrchResult<ContainerId> {
        if *self.should_fail_create.read().await {
            return Err(OrchestrationError::RuntimeError(
                "Simulated container creation failure".to_string(),
            ));
        }

        self.create_count.fetch_add(1, Ordering::SeqCst);
        let container_id = format!("container-{}", Uuid::new_v4());

        let container = MockContainer {
            id: container_id.clone(),
            config: config.clone(),
            state: "running".to_string(),
            node_id: options.node_id,
        };

        self.containers.write().await.insert(container_id.clone(), container);
        Ok(container_id)
    }

    async fn stop_container(&self, container_id: &ContainerId) -> OrchResult<()> {
        self.stop_count.fetch_add(1, Ordering::SeqCst);
        if let Some(container) = self.containers.write().await.get_mut(container_id) {
            container.state = "stopped".to_string();
        }
        Ok(())
    }

    async fn remove_container(&self, container_id: &ContainerId) -> OrchResult<()> {
        self.remove_count.fetch_add(1, Ordering::SeqCst);
        self.containers.write().await.remove(container_id);
        Ok(())
    }

    async fn get_container_status(&self, container_id: &ContainerId) -> OrchResult<ContainerStatus> {
        let containers = self.containers.read().await;
        if let Some(container) = containers.get(container_id) {
            Ok(ContainerStatus {
                id: container.id.clone(),
                state: container.state.clone(),
                exit_code: None,
                error_message: None,
            })
        } else {
            Err(OrchestrationError::RuntimeError(format!(
                "Container {} not found",
                container_id
            )))
        }
    }

    async fn list_containers(&self, node_id: NodeId) -> OrchResult<Vec<ContainerStatus>> {
        let containers = self.containers.read().await;
        Ok(containers
            .values()
            .filter(|c| c.node_id == node_id)
            .map(|c| ContainerStatus {
                id: c.id.clone(),
                state: c.state.clone(),
                exit_code: None,
                error_message: None,
            })
            .collect())
    }
}

/// Mock cluster manager that allows programmatic node events
struct MockClusterManager {
    event_tx: watch::Sender<Option<ClusterEvent>>,
    nodes: Arc<RwLock<HashMap<NodeId, Node>>>,
}

impl MockClusterManager {
    fn new() -> Self {
        let (event_tx, _) = watch::channel(None);
        Self {
            event_tx,
            nodes: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    async fn add_node(&self, node: Node) {
        self.nodes.write().await.insert(node.id, node.clone());
        let _ = self.event_tx.send(Some(ClusterEvent::NodeAdded(node)));
    }

    async fn remove_node(&self, node_id: NodeId) {
        self.nodes.write().await.remove(&node_id);
        let _ = self.event_tx.send(Some(ClusterEvent::NodeRemoved(node_id)));
    }

    async fn update_node(&self, node: Node) {
        self.nodes.write().await.insert(node.id, node.clone());
        let _ = self.event_tx.send(Some(ClusterEvent::NodeUpdated(node)));
    }
}

#[async_trait]
impl ClusterManager for MockClusterManager {
    async fn initialize(&self) -> OrchResult<()> {
        Ok(())
    }

    async fn get_node(&self, node_id: &NodeId) -> OrchResult<Option<Node>> {
        Ok(self.nodes.read().await.get(node_id).cloned())
    }

    async fn list_nodes(&self) -> OrchResult<Vec<Node>> {
        Ok(self.nodes.read().await.values().cloned().collect())
    }

    async fn subscribe_to_events(&self) -> OrchResult<watch::Receiver<Option<ClusterEvent>>> {
        Ok(self.event_tx.subscribe())
    }
}

// ============================================================================
// Test Helpers
// ============================================================================

fn generate_node_id() -> NodeId {
    Keypair::generate().public_key()
}

fn create_test_node(id: NodeId, status: NodeStatus) -> Node {
    // Use a hash of the public key bytes to generate a deterministic address suffix
    let key_str = id.to_string();
    let addr_suffix = key_str.as_bytes().iter().fold(0u8, |acc, &b| acc.wrapping_add(b));
    Node {
        id,
        address: format!("10.0.0.{}:8080", addr_suffix),
        status,
        labels: HashMap::new(),
        resources_capacity: NodeResources {
            cpu_cores: 4.0,
            memory_mb: 8192,
            disk_mb: 100000,
        },
        resources_allocatable: NodeResources {
            cpu_cores: 3.8,
            memory_mb: 7000,
            disk_mb: 90000,
        },
    }
}

fn create_test_workload(name: &str, replicas: u32) -> WorkloadDefinition {
    WorkloadDefinition {
        id: Uuid::new_v4(),
        name: name.to_string(),
        containers: vec![ContainerConfig {
            name: "test-container".to_string(),
            image: "test-image:latest".to_string(),
            command: None,
            args: None,
            env_vars: HashMap::new(),
            ports: vec![PortMapping {
                container_port: 8080,
                host_port: None,
                protocol: "tcp".to_string(),
            }],
            resource_requests: NodeResources {
                cpu_cores: 0.5,
                memory_mb: 256,
                disk_mb: 0,
            },
        }],
        replicas,
        labels: HashMap::new(),
    }
}

struct TestHarness {
    state_store: Arc<InMemoryStateStore>,
    runtime: Arc<MockContainerRuntime>,
    cluster_manager: Arc<MockClusterManager>,
    workload_tx: tokio::sync::mpsc::Sender<WorkloadDefinition>,
}

impl TestHarness {
    async fn new() -> Self {
        let state_store = Arc::new(InMemoryStateStore::new());
        let runtime = Arc::new(MockContainerRuntime::new());
        let cluster_manager = Arc::new(MockClusterManager::new());
        let scheduler = Arc::new(SimpleScheduler);

        let workload_tx = start_orchestrator_service(
            state_store.clone() as Arc<dyn StateStore>,
            runtime.clone() as Arc<dyn ContainerRuntime>,
            cluster_manager.clone() as Arc<dyn ClusterManager>,
            scheduler as Arc<dyn scheduler_interface::Scheduler>,
        )
        .await
        .expect("Failed to start orchestrator");

        // Give the orchestrator time to start and subscribe to events
        // This is crucial because watch channel subscriptions made AFTER
        // events are sent will mark those events as "already seen"
        tokio::time::sleep(Duration::from_millis(100)).await;

        Self {
            state_store,
            runtime,
            cluster_manager,
            workload_tx,
        }
    }

    async fn add_ready_node(&self) -> NodeId {
        let node_id = generate_node_id();
        let node = create_test_node(node_id, NodeStatus::Ready);
        self.cluster_manager.add_node(node).await;

        // Wait for the orchestrator to process the event and store the node
        // This ensures the node is fully available before returning
        assert!(
            self.wait_for_node_in_state(node_id).await,
            "Timeout waiting for node {} to be stored after add",
            node_id
        );

        node_id
    }

    async fn submit_workload(&self, workload: WorkloadDefinition) {
        self.workload_tx.send(workload).await.expect("Failed to send workload");
        // Give time for reconciliation
        tokio::time::sleep(Duration::from_millis(200)).await;
    }

    async fn wait_for_instances(&self, workload_id: Uuid, expected_count: usize) -> bool {
        for _ in 0..100 {
            let instances = self
                .state_store
                .list_instances_for_workload(&workload_id)
                .await
                .unwrap_or_default();
            if instances.len() == expected_count {
                return true;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        false
    }

    async fn wait_for_node_in_state(&self, node_id: NodeId) -> bool {
        for _ in 0..100 {
            if let Ok(Some(_)) = self.state_store.get_node(&node_id).await {
                return true;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        false
    }
}

// ============================================================================
// Integration Tests
// ============================================================================

#[tokio::test]
async fn test_workload_submission_creates_instances() {
    let harness = TestHarness::new().await;

    // Add a ready node first
    harness.add_ready_node().await;

    // Submit a workload with 2 replicas
    let workload = create_test_workload("test-app", 2);
    let workload_id = workload.id;
    harness.submit_workload(workload).await;

    // Wait for instances to be created
    assert!(
        harness.wait_for_instances(workload_id, 2).await,
        "Expected 2 instances to be created"
    );

    // Verify containers were created
    assert_eq!(harness.runtime.get_create_count().await, 2);
    assert_eq!(harness.runtime.get_container_count().await, 2);
}

#[tokio::test]
async fn test_workload_with_no_nodes_creates_no_instances() {
    let harness = TestHarness::new().await;

    // Submit workload WITHOUT adding any nodes
    let workload = create_test_workload("orphan-app", 3);
    let workload_id = workload.id;
    harness.submit_workload(workload).await;

    // Give extra time
    tokio::time::sleep(Duration::from_millis(200)).await;

    // No instances should be created
    let instances = harness
        .state_store
        .list_instances_for_workload(&workload_id)
        .await
        .unwrap();
    assert_eq!(instances.len(), 0);
    assert_eq!(harness.runtime.get_create_count().await, 0);
}

#[tokio::test]
async fn test_node_added_triggers_reconciliation() {
    let harness = TestHarness::new().await;

    // Submit workload first (no nodes available)
    let workload = create_test_workload("waiting-app", 1);
    let workload_id = workload.id;
    harness.submit_workload(workload).await;

    // Verify no instances yet
    tokio::time::sleep(Duration::from_millis(100)).await;
    let instances = harness
        .state_store
        .list_instances_for_workload(&workload_id)
        .await
        .unwrap();
    assert_eq!(instances.len(), 0);

    // Now add a node - should trigger reconciliation
    harness.add_ready_node().await;

    // Wait for instance to be created
    assert!(
        harness.wait_for_instances(workload_id, 1).await,
        "Expected instance to be created after node added"
    );
}

#[tokio::test]
async fn test_scale_up_workload() {
    let harness = TestHarness::new().await;

    // Add nodes
    harness.add_ready_node().await;
    harness.add_ready_node().await;

    // Submit workload with 1 replica
    let mut workload = create_test_workload("scale-app", 1);
    let workload_id = workload.id;
    harness.submit_workload(workload.clone()).await;

    // Verify 1 instance created
    assert!(harness.wait_for_instances(workload_id, 1).await);
    assert_eq!(harness.runtime.get_create_count().await, 1);

    // Scale up to 3 replicas
    workload.replicas = 3;
    harness.submit_workload(workload).await;

    // Wait for additional instances
    assert!(
        harness.wait_for_instances(workload_id, 3).await,
        "Expected 3 instances after scale up"
    );
    assert_eq!(harness.runtime.get_create_count().await, 3);
}

#[tokio::test]
async fn test_scale_down_workload() {
    let harness = TestHarness::new().await;

    // Add nodes
    harness.add_ready_node().await;
    harness.add_ready_node().await;

    // Submit workload with 3 replicas
    let mut workload = create_test_workload("scale-down-app", 3);
    let workload_id = workload.id;
    harness.submit_workload(workload.clone()).await;

    // Wait for all instances
    assert!(harness.wait_for_instances(workload_id, 3).await);
    assert_eq!(harness.runtime.get_create_count().await, 3);
    assert_eq!(harness.runtime.get_container_count().await, 3);

    // Scale down to 1 replica
    workload.replicas = 1;
    harness.submit_workload(workload).await;

    // Wait for scale down
    assert!(
        harness.wait_for_instances(workload_id, 1).await,
        "Expected 1 instance after scale down"
    );

    // Verify containers were stopped and removed
    assert!(harness.runtime.get_stop_count().await >= 2);
    assert!(harness.runtime.get_remove_count().await >= 2);
}

#[tokio::test]
async fn test_multiple_workloads() {
    let harness = TestHarness::new().await;

    // Add nodes
    harness.add_ready_node().await;
    harness.add_ready_node().await;

    // Submit multiple workloads
    let workload1 = create_test_workload("app-1", 2);
    let workload2 = create_test_workload("app-2", 1);
    let workload3 = create_test_workload("app-3", 2);

    let id1 = workload1.id;
    let id2 = workload2.id;
    let id3 = workload3.id;

    harness.submit_workload(workload1).await;
    harness.submit_workload(workload2).await;
    harness.submit_workload(workload3).await;

    // Wait for all instances
    assert!(harness.wait_for_instances(id1, 2).await);
    assert!(harness.wait_for_instances(id2, 1).await);
    assert!(harness.wait_for_instances(id3, 2).await);

    // Total should be 5 instances
    let all_instances = harness.state_store.list_all_instances().await.unwrap();
    assert_eq!(all_instances.len(), 5);
    assert_eq!(harness.runtime.get_create_count().await, 5);
}

#[tokio::test]
async fn test_workload_without_containers_definition() {
    let harness = TestHarness::new().await;
    harness.add_ready_node().await;

    // Create workload with no containers
    let workload = WorkloadDefinition {
        id: Uuid::new_v4(),
        name: "empty-containers".to_string(),
        containers: vec![], // No container definitions
        replicas: 2,
        labels: HashMap::new(),
    };
    let workload_id = workload.id;

    harness.submit_workload(workload).await;
    tokio::time::sleep(Duration::from_millis(200)).await;

    // No instances should be created (no container config)
    let instances = harness
        .state_store
        .list_instances_for_workload(&workload_id)
        .await
        .unwrap();
    assert_eq!(instances.len(), 0);
    assert_eq!(harness.runtime.get_create_count().await, 0);
}

#[tokio::test]
async fn test_node_status_not_ready_ignored() {
    let harness = TestHarness::new().await;

    // Add a NotReady node
    let node_id = generate_node_id();
    let node = create_test_node(node_id, NodeStatus::NotReady);
    harness.cluster_manager.add_node(node).await;
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Submit workload
    let workload = create_test_workload("test-app", 1);
    let workload_id = workload.id;
    harness.submit_workload(workload).await;

    // No instances should be created (node not ready)
    tokio::time::sleep(Duration::from_millis(200)).await;
    let instances = harness
        .state_store
        .list_instances_for_workload(&workload_id)
        .await
        .unwrap();
    assert_eq!(instances.len(), 0);
}

#[tokio::test]
async fn test_already_reconciled_workload_no_action() {
    let harness = TestHarness::new().await;
    harness.add_ready_node().await;

    // Submit workload
    let workload = create_test_workload("stable-app", 2);
    let workload_id = workload.id;
    harness.submit_workload(workload.clone()).await;

    // Wait for instances
    assert!(harness.wait_for_instances(workload_id, 2).await);
    let initial_create_count = harness.runtime.get_create_count().await;

    // Submit same workload again (should be no-op)
    harness.submit_workload(workload).await;
    tokio::time::sleep(Duration::from_millis(200)).await;

    // No additional containers should be created
    assert_eq!(harness.runtime.get_create_count().await, initial_create_count);
}

#[tokio::test]
async fn test_state_persistence_across_operations() {
    let harness = TestHarness::new().await;
    harness.add_ready_node().await;

    let workload = create_test_workload("persistent-app", 2);
    let workload_id = workload.id;
    harness.submit_workload(workload.clone()).await;

    // Verify workload is stored
    let stored_workload = harness.state_store.get_workload(&workload_id).await.unwrap();
    assert!(stored_workload.is_some());
    assert_eq!(stored_workload.unwrap().name, "persistent-app");

    // Wait for instances
    assert!(harness.wait_for_instances(workload_id, 2).await);

    // Verify instances are stored
    let instances = harness
        .state_store
        .list_instances_for_workload(&workload_id)
        .await
        .unwrap();
    assert_eq!(instances.len(), 2);
    for instance in &instances {
        assert_eq!(instance.workload_id, workload_id);
    }
}

#[tokio::test]
async fn test_node_stored_in_state_on_add() {
    let harness = TestHarness::new().await;

    // Initially no nodes
    let nodes = harness.state_store.list_nodes().await.unwrap();
    assert_eq!(nodes.len(), 0);

    // Add node via cluster manager
    let node_id = harness.add_ready_node().await;

    // Wait for node to be stored in state store
    assert!(
        harness.wait_for_node_in_state(node_id).await,
        "Node should be stored in state"
    );

    let stored_node = harness.state_store.get_node(&node_id).await.unwrap();
    assert!(stored_node.is_some());
    assert_eq!(stored_node.unwrap().status, NodeStatus::Ready);

    let nodes = harness.state_store.list_nodes().await.unwrap();
    assert_eq!(nodes.len(), 1);
}

#[tokio::test]
async fn test_workload_zero_replicas() {
    let harness = TestHarness::new().await;
    harness.add_ready_node().await;

    // Submit workload with 0 replicas
    let workload = create_test_workload("zero-replicas", 0);
    let workload_id = workload.id;
    harness.submit_workload(workload).await;

    tokio::time::sleep(Duration::from_millis(200)).await;

    // No instances should be created
    let instances = harness
        .state_store
        .list_instances_for_workload(&workload_id)
        .await
        .unwrap();
    assert_eq!(instances.len(), 0);
    assert_eq!(harness.runtime.get_create_count().await, 0);
}

#[tokio::test]
async fn test_rapid_workload_submissions() {
    let harness = TestHarness::new().await;

    // Add multiple nodes
    for _ in 0..3 {
        harness.add_ready_node().await;
    }

    // Rapidly submit workloads
    let mut workload_ids = Vec::new();
    for i in 0..5 {
        let workload = create_test_workload(&format!("rapid-app-{}", i), 1);
        workload_ids.push(workload.id);
        // Don't wait between submissions
        harness.workload_tx.send(workload).await.unwrap();
    }

    // Wait for all to be processed
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Verify all workloads have instances
    for workload_id in workload_ids {
        assert!(
            harness.wait_for_instances(workload_id, 1).await,
            "Workload {} should have 1 instance",
            workload_id
        );
    }

    assert_eq!(harness.runtime.get_create_count().await, 5);
}
