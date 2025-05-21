use std::sync::Arc;
use tokio;

// Assuming you have mock implementations in these modules or crates
// For example:
// mod mock_runtime; use mock_runtime::MockRuntime;
// mod mock_cluster_manager; use mock_cluster_manager::MockClusterManager;

use orchestrator_core::start_orchestrator_service; // if in lib.rs
use orchestrator_shared_types::{NodeId, WorkloadDefinition, ContainerConfig, NodeResources, PortMapping};
use scheduler_interface::SimpleScheduler;
use uuid::Uuid;
use std::collections::HashMap;

// --- Mock Implementations (simplified, put these in their respective interface crates or a mock crate) ---
use async_trait::async_trait;
use orchestrator_shared_types::{Node, Result, OrchestrationError, ContainerId};
use container_runtime_interface::{ContainerRuntime, CreateContainerOptions, ContainerStatus};
use cluster_manager_interface::{ClusterManager, ClusterEvent};
use tokio::sync::watch;

#[derive(Default, Clone)]
struct MockRuntime {
    containers: Arc<tokio::sync::Mutex<HashMap<ContainerId, (ContainerConfig, ContainerStatus)>>>,
}

#[async_trait]
impl ContainerRuntime for MockRuntime {
    async fn init_node(&self, _node_id: NodeId) -> Result<()> { Ok(()) }
    async fn create_container(&self, config: &ContainerConfig, _options: &CreateContainerOptions) -> Result<ContainerId> {
        let id = Uuid::new_v4().to_string();
        let status = ContainerStatus { id: id.clone(), state: "Pending".to_string(), exit_code: None, error_message: None };
        self.containers.lock().await.insert(id.clone(), (config.clone(), status));
        tracing::info!("[MockRuntime] Created container {}", id);
        // Simulate it starting after a bit
        let containers_clone = self.containers.clone();
        let id_clone = id.clone();
        tokio::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
            let mut locked_containers = containers_clone.lock().await;
            if let Some((_cfg, status)) = locked_containers.get_mut(&id_clone) {
                status.state = "Running".to_string();
                tracing::info!("[MockRuntime] Container {} is now Running", id_clone);
            }
        });
        Ok(id)
    }
    async fn stop_container(&self, container_id: &ContainerId) -> Result<()> {
        if let Some((_cfg, status)) = self.containers.lock().await.get_mut(container_id) {
            status.state = "Stopped".to_string();
            status.exit_code = Some(0);
            tracing::info!("[MockRuntime] Stopped container {}", container_id);
            Ok(())
        } else {
            Err(OrchestrationError::RuntimeError(format!("Container {} not found", container_id)))
        }
    }
    async fn remove_container(&self, container_id: &ContainerId) -> Result<()> {
        if self.containers.lock().await.remove(container_id).is_some() {
            tracing::info!("[MockRuntime] Removed container {}", container_id);
            Ok(())
        } else {
            Err(OrchestrationError::RuntimeError(format!("Container {} not found for removal", container_id)))
        }
    }
    async fn get_container_status(&self, container_id: &ContainerId) -> Result<ContainerStatus> {
        self.containers.lock().await.get(container_id)
            .map(|(_cfg, status)| status.clone())
            .ok_or_else(|| OrchestrationError::RuntimeError(format!("Container {} status not found", container_id)))
    }
    async fn list_containers(&self, _node_id: NodeId) -> Result<Vec<ContainerStatus>> {
        Ok(self.containers.lock().await.values().map(|(_cfg, status)| status.clone()).collect())
    }
}

struct MockClusterManager {
    event_tx: watch::Sender<Option<ClusterEvent>>,
    nodes: Arc<tokio::sync::Mutex<HashMap<NodeId, Node>>>,
}
impl MockClusterManager {
    fn new() -> (Self, watch::Receiver<Option<ClusterEvent>>) {
        let (tx, rx) = watch::channel(None);
        (MockClusterManager { event_tx: tx, nodes: Arc::new(tokio::sync::Mutex::new(HashMap::new())) }, rx)
    }
    async fn add_node(&self, node: Node) {
        self.nodes.lock().await.insert(node.id, node.clone());
        self.event_tx.send(Some(ClusterEvent::NodeAdded(node))).ok();
    }
}

#[async_trait]
impl ClusterManager for MockClusterManager {
    async fn initialize(&self) -> Result<()> { Ok(()) }
    async fn get_node(&self, node_id: &NodeId) -> Result<Option<Node>> {
        Ok(self.nodes.lock().await.get(node_id).cloned())
    }
    async fn list_nodes(&self) -> Result<Vec<Node>> {
        Ok(self.nodes.lock().await.values().cloned().collect())
    }
    async fn subscribe_to_events(&self) -> Result<watch::Receiver<Option<ClusterEvent>>> {
        Ok(self.event_tx.subscribe())
    }
}
// --- End Mock Implementations ---

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let runtime = Arc::new(MockRuntime::default());
    let (cluster_manager_impl, _cluster_event_rx_for_test) = MockClusterManager::new();
    let cluster_manager: Arc<dyn ClusterManager> = Arc::new(cluster_manager_impl);
    let scheduler = Arc::new(SimpleScheduler);

    // Start the orchestrator service (it runs in a spawned task)
    let workload_tx = start_orchestrator_service(runtime.clone(), cluster_manager.clone(), scheduler.clone()).await?;
    tracing::info!("Orchestrator service started in background.");

    // Simulate adding a node to the cluster after a delay
    let mock_cm_accessor = cluster_manager.clone(); // To access add_node if it were public on the trait or specific type
    tokio::spawn(async move {
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        let node1_id = Uuid::new_v4();
        let node1 = Node {
            id: node1_id,
            address: "10.0.0.1:1234".to_string(),
            status: orchestrator_shared_types::NodeStatus::Ready,
            labels: Default::default(),
            resources_capacity: NodeResources { cpu_cores: 4.0, memory_mb: 8192, disk_mb: 100000 },
            resources_allocatable: NodeResources { cpu_cores: 3.8, memory_mb: 7000, disk_mb: 90000 },
        };
        tracing::info!("Simulating add_node: {}", node1_id);
        // This downcast is a bit of a hack for testing, ideally you'd have a way to interact with the mock
        if let Some(mock_cm) = mock_cm_accessor.downcast_arc::<MockClusterManager>().ok() {
             mock_cm.add_node(node1).await;
        } else {
            tracing::error!("Failed to downcast to MockClusterManager to add node");
        }
    });


    // Simulate submitting a workload after a few seconds (to allow node to be added)
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
    let workload_def = WorkloadDefinition {
        id: Uuid::new_v4(),
        name: "my-nginx-service".to_string(),
        containers: vec![ContainerConfig {
            name: "nginx".to_string(),
            image: "nginx:latest".to_string(),
            command: None,
            args: None,
            env_vars: Default::default(),
            ports: vec![PortMapping { container_port: 80, host_port: Some(8080), protocol: "tcp".to_string() }],
            resource_requests: NodeResources { cpu_cores: 0.5, memory_mb: 256, disk_mb: 0 },
        }],
        replicas: 2,
        labels: Default::default(),
    };
    tracing::info!("Submitting workload: {}", workload_def.name);
    workload_tx.send(workload_def.clone()).await?;


    // Keep main alive for a bit to see logs, or until Ctrl+C
    tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
    tracing::info!("Test period finished.");

    Ok(())
}