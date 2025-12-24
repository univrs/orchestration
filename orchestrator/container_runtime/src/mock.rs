//! Mock container runtime for testing and development.
//!
//! This provides an in-memory implementation that simulates container operations
//! without actually creating containers.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::RwLock;
use tracing::{debug, info};
use uuid::Uuid;

use container_runtime_interface::{
    ContainerRuntime, ContainerStatus, CreateContainerOptions,
};
use orchestrator_shared_types::{ContainerConfig, ContainerId, NodeId, Result};

/// Mock container state
#[derive(Debug, Clone)]
struct MockContainer {
    id: ContainerId,
    _config: ContainerConfig,
    node_id: NodeId,
    state: String,
    exit_code: Option<i32>,
}

/// Mock runtime that simulates container operations in-memory.
#[derive(Debug, Default)]
pub struct MockRuntime {
    /// All containers by ID
    containers: Arc<RwLock<HashMap<ContainerId, MockContainer>>>,
    /// Containers grouped by node
    containers_by_node: Arc<RwLock<HashMap<NodeId, Vec<ContainerId>>>>,
    /// Initialized nodes
    initialized_nodes: Arc<RwLock<Vec<NodeId>>>,
}

impl MockRuntime {
    /// Create a new mock runtime instance.
    pub fn new() -> Self {
        Self::default()
    }

    /// Get the count of containers (for testing).
    pub async fn container_count(&self) -> usize {
        self.containers.read().await.len()
    }

    /// Check if a node is initialized (for testing).
    pub async fn is_node_initialized(&self, node_id: &NodeId) -> bool {
        self.initialized_nodes.read().await.contains(node_id)
    }
}

#[async_trait]
impl ContainerRuntime for MockRuntime {
    async fn init_node(&self, node_id: NodeId) -> Result<()> {
        info!("MockRuntime: Initializing node {}", node_id);

        let mut nodes = self.initialized_nodes.write().await;
        if !nodes.contains(&node_id) {
            nodes.push(node_id);
        }

        // Initialize container list for this node
        let mut by_node = self.containers_by_node.write().await;
        by_node.entry(node_id).or_insert_with(Vec::new);

        Ok(())
    }

    async fn create_container(
        &self,
        config: &ContainerConfig,
        options: &CreateContainerOptions,
    ) -> Result<ContainerId> {
        let container_id = format!("mock-container-{}", Uuid::new_v4());

        info!(
            "MockRuntime: Creating container {} for workload {} on node {}",
            container_id, options.workload_id, options.node_id
        );
        debug!("Container config: {:?}", config);

        let container = MockContainer {
            id: container_id.clone(),
            _config: config.clone(),
            node_id: options.node_id,
            state: "running".to_string(),
            exit_code: None,
        };

        // Store container
        self.containers.write().await.insert(container_id.clone(), container);

        // Track by node
        self.containers_by_node
            .write()
            .await
            .entry(options.node_id)
            .or_insert_with(Vec::new)
            .push(container_id.clone());

        Ok(container_id)
    }

    async fn stop_container(&self, container_id: &ContainerId) -> Result<()> {
        info!("MockRuntime: Stopping container {}", container_id);

        let mut containers = self.containers.write().await;
        if let Some(container) = containers.get_mut(container_id) {
            container.state = "stopped".to_string();
            container.exit_code = Some(0);
            Ok(())
        } else {
            Err(orchestrator_shared_types::OrchestrationError::RuntimeError(
                format!("Container not found: {}", container_id),
            ))
        }
    }

    async fn remove_container(&self, container_id: &ContainerId) -> Result<()> {
        info!("MockRuntime: Removing container {}", container_id);

        let mut containers = self.containers.write().await;
        if let Some(container) = containers.remove(container_id) {
            // Remove from node tracking
            let mut by_node = self.containers_by_node.write().await;
            if let Some(node_containers) = by_node.get_mut(&container.node_id) {
                node_containers.retain(|id| id != container_id);
            }
            Ok(())
        } else {
            Err(orchestrator_shared_types::OrchestrationError::RuntimeError(
                format!("Container not found: {}", container_id),
            ))
        }
    }

    async fn get_container_status(&self, container_id: &ContainerId) -> Result<ContainerStatus> {
        debug!("MockRuntime: Getting status for container {}", container_id);

        let containers = self.containers.read().await;
        if let Some(container) = containers.get(container_id) {
            Ok(ContainerStatus {
                id: container.id.clone(),
                state: container.state.clone(),
                exit_code: container.exit_code,
                error_message: None,
            })
        } else {
            Err(orchestrator_shared_types::OrchestrationError::RuntimeError(
                format!("Container not found: {}", container_id),
            ))
        }
    }

    async fn list_containers(&self, node_id: NodeId) -> Result<Vec<ContainerStatus>> {
        debug!("MockRuntime: Listing containers for node {}", node_id);

        let containers = self.containers.read().await;
        let by_node = self.containers_by_node.read().await;

        let container_ids = by_node.get(&node_id).cloned().unwrap_or_default();

        let statuses: Vec<ContainerStatus> = container_ids
            .iter()
            .filter_map(|id| {
                containers.get(id).map(|c| ContainerStatus {
                    id: c.id.clone(),
                    state: c.state.clone(),
                    exit_code: c.exit_code,
                    error_message: None,
                })
            })
            .collect();

        Ok(statuses)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use orchestrator_shared_types::{NodeResources, PortMapping};
    use std::collections::HashMap;

    fn create_test_config() -> ContainerConfig {
        ContainerConfig {
            name: "test-container".to_string(),
            image: "nginx:latest".to_string(),
            command: None,
            args: None,
            env_vars: HashMap::new(),
            ports: vec![PortMapping {
                container_port: 80,
                host_port: Some(8080),
                protocol: "tcp".to_string(),
            }],
            resource_requests: NodeResources::default(),
        }
    }

    fn generate_node_id() -> NodeId {
        orchestrator_shared_types::Keypair::generate().public_key()
    }

    #[tokio::test]
    async fn test_init_node() {
        let runtime = MockRuntime::new();
        let node_id = generate_node_id();

        assert!(!runtime.is_node_initialized(&node_id).await);
        runtime.init_node(node_id).await.unwrap();
        assert!(runtime.is_node_initialized(&node_id).await);
    }

    #[tokio::test]
    async fn test_create_container() {
        let runtime = MockRuntime::new();
        let node_id = generate_node_id();
        let workload_id = Uuid::new_v4();

        runtime.init_node(node_id).await.unwrap();

        let config = create_test_config();
        let options = CreateContainerOptions {
            workload_id,
            node_id,
        };

        let container_id = runtime.create_container(&config, &options).await.unwrap();
        assert!(container_id.starts_with("mock-container-"));
        assert_eq!(runtime.container_count().await, 1);
    }

    #[tokio::test]
    async fn test_stop_and_remove_container() {
        let runtime = MockRuntime::new();
        let node_id = generate_node_id();
        let workload_id = Uuid::new_v4();

        runtime.init_node(node_id).await.unwrap();

        let config = create_test_config();
        let options = CreateContainerOptions {
            workload_id,
            node_id,
        };

        let container_id = runtime.create_container(&config, &options).await.unwrap();

        // Stop
        runtime.stop_container(&container_id).await.unwrap();
        let status = runtime.get_container_status(&container_id).await.unwrap();
        assert_eq!(status.state, "stopped");
        assert_eq!(status.exit_code, Some(0));

        // Remove
        runtime.remove_container(&container_id).await.unwrap();
        assert_eq!(runtime.container_count().await, 0);
    }

    #[tokio::test]
    async fn test_list_containers() {
        let runtime = MockRuntime::new();
        let node_id = generate_node_id();
        let workload_id = Uuid::new_v4();

        runtime.init_node(node_id).await.unwrap();

        let config = create_test_config();
        let options = CreateContainerOptions {
            workload_id,
            node_id,
        };

        // Create 3 containers
        for _ in 0..3 {
            runtime.create_container(&config, &options).await.unwrap();
        }

        let containers = runtime.list_containers(node_id).await.unwrap();
        assert_eq!(containers.len(), 3);
    }

    #[tokio::test]
    async fn test_container_not_found() {
        let runtime = MockRuntime::new();

        let result = runtime.get_container_status(&"nonexistent".to_string()).await;
        assert!(result.is_err());
    }
}
