// orchestrator_core/src/main.rs
// ... (imports) ...
use std::sync::Arc;
use tokio;

use orchestrator_core::start_orchestrator_service;
use orchestrator_shared_types::{NodeId, WorkloadDefinition, ContainerConfig, NodeResources, PortMapping, Node, Result as OrchestrationResult, OrchestrationError, ContainerId};
use scheduler_interface::SimpleScheduler;
use uuid::Uuid;
use std::collections::HashMap;

use async_trait::async_trait;
use container_runtime_interface::{ContainerRuntime, CreateContainerOptions, ContainerStatus};
use cluster_manager_interface::{ClusterManager, ClusterEvent}; // Make sure this is correct path
use tokio::sync::watch;
use downcast_rs::DowncastArc; // Not strictly needed for the main.rs usage side, but good for clarity if trait is modified

// --- Mock Implementations (simplified) ---
#[derive(Default, Clone)]
struct MockRuntime {
    containers: Arc<tokio::sync::Mutex<HashMap<ContainerId, (ContainerConfig, ContainerStatus)>>>,
}

#[async_trait]
impl ContainerRuntime for MockRuntime {
    async fn init_node(&self, _node_id: NodeId) -> OrchestrationResult<()> { Ok(()) }
    async fn create_container(&self, config: &ContainerConfig, _options: &CreateContainerOptions) -> OrchestrationResult<ContainerId> {
        let id = Uuid::new_v4().to_string();
        let status = ContainerStatus { id: id.clone(), state: "Pending".to_string(), exit_code: None, error_message: None };
        self.containers.lock().await.insert(id.clone(), (config.clone(), status));
        tracing::info!("[MockRuntime] Created container {}", id);
        let containers_clone = self.containers.clone();
        let id_clone = id.clone();
        tokio::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await; // Faster for test
            let mut locked_containers = containers_clone.lock().await;
            if let Some((_cfg, status)) = locked_containers.get_mut(&id_clone) {
                status.state = "Running".to_string();
                tracing::info!("[MockRuntime] Container {} is now Running", id_clone);
            }
        });
        Ok(id)
    }
    async fn stop_container(&self, container_id: &ContainerId) -> OrchestrationResult<()> {
        if let Some((_cfg, status)) = self.containers.lock().await.get_mut(container_id) {
            status.state = "Stopped".to_string();
            status.exit_code = Some(0);
            tracing::info!("[MockRuntime] Stopped container {}", container_id);
            Ok(())
        } else {
            Err(OrchestrationError::RuntimeError(format!("Container {} not found", container_id)))
        }
    }
    async fn remove_container(&self, container_id: &ContainerId) -> OrchestrationResult<()> {
        if self.containers.lock().await.remove(container_id).is_some() {
            tracing::info!("[MockRuntime] Removed container {}", container_id);
            Ok(())
        } else {
            Err(OrchestrationError::RuntimeError(format!("Container {} not found for removal", container_id)))
        }
    }
    async fn get_container_status(&self, container_id: &ContainerId) -> OrchestrationResult<ContainerStatus> {
        self.containers.lock().await.get(container_id)
            .map(|(_cfg, status)| status.clone())
            .ok_or_else(|| OrchestrationError::RuntimeError(format!("Container {} status not found", container_id)))
    }
    async fn list_containers(&self, _node_id: NodeId) -> OrchestrationResult<Vec<ContainerStatus>> {
        Ok(self.containers.lock().await.values().map(|(_cfg, status)| status.clone()).collect())
    }
}

// Ensure MockClusterManager is Send + Sync, which it is due to Arc<Mutex<...>> and watch::Sender being Send + Sync
struct MockClusterManager {
    event_tx: watch::Sender<Option<ClusterEvent>>,
    nodes: Arc<tokio::sync::Mutex<HashMap<NodeId, Node>>>,
}
// This impl is important for downcast-rs to work with the trait
// It's implicitly provided by `impl_downcast!(ClusterManager Sync);` in the trait definition file
// if MockClusterManager is Send + Sync + 'static.

impl MockClusterManager {
    fn new() -> Self { // Removed receiver from direct return, orchestrator will subscribe
        let (tx, _rx) = watch::channel(None); // _rx is not used here directly
        MockClusterManager { event_tx: tx, nodes: Arc::new(tokio::sync::Mutex::new(HashMap::new())) }
    }
    // This method is specific to MockClusterManager and is what we want to call via downcasting
    async fn add_node(&self, node: Node) {
        self.nodes.lock().await.insert(node.id, node.clone());
        if self.event_tx.send(Some(ClusterEvent::NodeAdded(node.clone()))).is_err() {
            tracing::warn!("[MockClusterManager] Failed to send NodeAdded event: receiver dropped for node {}", node.id);
        } else {
            tracing::info!("[MockClusterManager] Sent NodeAdded event for node {}", node.id);
        }
    }
}

#[async_trait]
impl ClusterManager for MockClusterManager {
    async fn initialize(&self) -> OrchestrationResult<()> {
        tracing::info!("[MockClusterManager] Initialized");
        Ok(())
    }
    async fn get_node(&self, node_id: &NodeId) -> OrchestrationResult<Option<Node>> {
        Ok(self.nodes.lock().await.get(node_id).cloned())
    }
    async fn list_nodes(&self) -> OrchestrationResult<Vec<Node>> {
        Ok(self.nodes.lock().await.values().cloned().collect())
    }
    async fn subscribe_to_events(&self) -> OrchestrationResult<watch::Receiver<Option<ClusterEvent>>> {
        Ok(self.event_tx.subscribe())
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    //tracing_subscriber::fmt()
    //    .with_env_filter(tracing_subscriber::EnvFilter::from_default_env().add_directive("info".parse()?)) // Default to info
    //    .init();
        // Initialize logging (Ensure tracing_subscriber is a dependency of orchestrator_core)
    use tracing_subscriber::{fmt, EnvFilter}; // Add use statement here if not global
    fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse()?)) // Default to info
        .init();

    let runtime = Arc::new(MockRuntime::default());
    let mock_cluster_manager_concrete = Arc::new(MockClusterManager::new()); // Create concrete Arc<MockClusterManager>
    let cluster_manager_trait_object: Arc<dyn ClusterManager> = mock_cluster_manager_concrete.clone(); // Clone for the trait object
    
    let scheduler = Arc::new(SimpleScheduler);

    let workload_tx = start_orchestrator_service(
        runtime.clone(), 
        cluster_manager_trait_object, // Pass the Arc<dyn ClusterManager>
        scheduler.clone()
    ).await?;
    tracing::info!("Orchestrator service started in background.");

    // Simulate adding a node using the concrete type before downcasting, or use downcasting if needed.
    // Here, we have direct access to mock_cluster_manager_concrete
    tokio::spawn({
        let mock_cm_for_spawn = mock_cluster_manager_concrete.clone(); // Clone the Arc<MockClusterManager>
        async move {
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
            tracing::info!("[main] Simulating add_node: {}", node1_id);
            mock_cm_for_spawn.add_node(node1).await; // Call add_node on the concrete type
        }
    });
    
    // Example of using downcast_arc if you only had the trait object:
    // This is what the original code was trying to do.
    let cm_trait_for_downcast: Arc<dyn ClusterManager> = mock_cluster_manager_concrete.clone();
    tokio::spawn(async move {
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await; // Add another node later
        let node2_id = Uuid::new_v4();
        let node2 = Node {
            id: node2_id,
            address: "10.0.0.2:1234".to_string(),
            status: orchestrator_shared_types::NodeStatus::Ready,
            // ... (fill other fields)
            labels: Default::default(),
            resources_capacity: NodeResources { cpu_cores: 2.0, memory_mb: 4096, disk_mb: 50000 },
            resources_allocatable: NodeResources { cpu_cores: 1.8, memory_mb: 3500, disk_mb: 45000 },
        };
        // The actual downcast
        if let Ok(concrete_mock_cm) = Arc::downcast_arc::<MockClusterManager>(cm_trait_for_downcast) {
            tracing::info!("[main via downcast] Simulating add_node for node2: {}", node2_id);
            concrete_mock_cm.add_node(node2).await;
        } else {
            tracing::error!("[main via downcast] Failed to downcast to MockClusterManager to add node2");
        }
    });


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
        replicas: 1, // Reduced for quicker testing
        labels: Default::default(),
    };
    tracing::info!("[main] Submitting workload: {}", workload_def.name);
    if workload_tx.send(workload_def.clone()).await.is_err() {
        tracing::error!("[main] Failed to send workload: orchestrator likely shut down");
    }

    tokio::time::sleep(tokio::time::Duration::from_secs(10)).await; // Reduced time
    tracing::info!("[main] Test period finished.");

    Ok(())
}