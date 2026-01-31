//! Mock cluster manager for testing and development.
//!
//! This provides an in-memory implementation that simulates cluster operations
//! without requiring an actual gossip network.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::{broadcast, RwLock};
use tracing::{debug, info};

use cluster_manager_interface::{ClusterEvent, ClusterManager};
use orchestrator_shared_types::{Node, NodeId, NodeResources, NodeStatus, Result};

/// Buffer size for the broadcast channel
const EVENT_CHANNEL_CAPACITY: usize = 64;

/// Mock cluster manager that simulates cluster operations in-memory.
pub struct MockClusterManager {
    /// All nodes by ID
    nodes: Arc<RwLock<HashMap<NodeId, Node>>>,
    /// Event broadcaster - uses broadcast channel to ensure all events are delivered
    event_tx: Arc<broadcast::Sender<ClusterEvent>>,
    /// Whether the cluster is initialized
    initialized: Arc<RwLock<bool>>,
}

impl std::fmt::Debug for MockClusterManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MockClusterManager")
            .field("nodes", &self.nodes)
            .field("initialized", &self.initialized)
            .finish()
    }
}

impl Default for MockClusterManager {
    fn default() -> Self {
        Self::new()
    }
}

impl MockClusterManager {
    /// Create a new mock cluster manager instance.
    pub fn new() -> Self {
        let (event_tx, _) = broadcast::channel(EVENT_CHANNEL_CAPACITY);
        Self {
            nodes: Arc::new(RwLock::new(HashMap::new())),
            event_tx: Arc::new(event_tx),
            initialized: Arc::new(RwLock::new(false)),
        }
    }

    /// Add a node to the cluster (for testing).
    pub async fn add_node(&self, node: Node) -> Result<()> {
        let node_id = node.id.clone();
        info!("MockClusterManager: Adding node {}", node_id);

        self.nodes.write().await.insert(node_id, node.clone());

        // Broadcast the event - all subscribers receive this
        let _ = self.event_tx.send(ClusterEvent::NodeAdded(node));

        Ok(())
    }

    /// Remove a node from the cluster (for testing).
    pub async fn remove_node(&self, node_id: &NodeId) -> Result<()> {
        info!("MockClusterManager: Removing node {}", node_id);

        if self.nodes.write().await.remove(node_id).is_some() {
            // Broadcast the event
            let _ = self.event_tx.send(ClusterEvent::NodeRemoved(node_id.clone()));
        }

        Ok(())
    }

    /// Update a node's status (for testing).
    pub async fn update_node_status(&self, node_id: &NodeId, status: NodeStatus) -> Result<()> {
        debug!(
            "MockClusterManager: Updating node {} status to {:?}",
            node_id, status
        );

        let mut nodes = self.nodes.write().await;
        if let Some(node) = nodes.get_mut(node_id) {
            node.status = status;
            let updated_node = node.clone();
            drop(nodes);

            // Broadcast the event
            let _ = self.event_tx.send(ClusterEvent::NodeUpdated(updated_node));
        }

        Ok(())
    }

    /// Get the count of nodes (for testing).
    pub async fn node_count(&self) -> usize {
        self.nodes.read().await.len()
    }

    /// Check if cluster is initialized (for testing).
    pub async fn is_initialized(&self) -> bool {
        *self.initialized.read().await
    }

    /// Create a test node with default resources.
    pub fn create_test_node(node_id: NodeId, address: &str) -> Node {
        Node {
            id: node_id,
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
}

#[async_trait]
impl ClusterManager for MockClusterManager {
    async fn initialize(&self) -> Result<()> {
        info!("MockClusterManager: Initializing cluster manager");
        *self.initialized.write().await = true;
        Ok(())
    }

    async fn get_node(&self, node_id: &NodeId) -> Result<Option<Node>> {
        debug!("MockClusterManager: Getting node {}", node_id);
        Ok(self.nodes.read().await.get(node_id).cloned())
    }

    async fn list_nodes(&self) -> Result<Vec<Node>> {
        debug!("MockClusterManager: Listing all nodes");
        Ok(self.nodes.read().await.values().cloned().collect())
    }

    async fn subscribe_to_events(&self) -> Result<broadcast::Receiver<ClusterEvent>> {
        debug!("MockClusterManager: Creating event subscription");
        Ok(self.event_tx.subscribe())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use orchestrator_shared_types::Keypair;

    #[tokio::test]
    async fn test_initialize() {
        let manager = MockClusterManager::new();
        assert!(!manager.is_initialized().await);

        manager.initialize().await.unwrap();
        assert!(manager.is_initialized().await);
    }

    fn generate_node_id() -> NodeId {
        Keypair::generate().public_key()
    }

    #[tokio::test]
    async fn test_add_node() {
        let manager = MockClusterManager::new();
        manager.initialize().await.unwrap();

        let node_id = generate_node_id();
        let node = MockClusterManager::create_test_node(node_id, "10.0.0.1:8080");

        manager.add_node(node.clone()).await.unwrap();
        assert_eq!(manager.node_count().await, 1);

        let retrieved = manager.get_node(&node_id).await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().address, "10.0.0.1:8080");
    }

    #[tokio::test]
    async fn test_remove_node() {
        let manager = MockClusterManager::new();
        manager.initialize().await.unwrap();

        let node_id = generate_node_id();
        let node = MockClusterManager::create_test_node(node_id, "10.0.0.1:8080");

        manager.add_node(node).await.unwrap();
        assert_eq!(manager.node_count().await, 1);

        manager.remove_node(&node_id).await.unwrap();
        assert_eq!(manager.node_count().await, 0);

        let retrieved = manager.get_node(&node_id).await.unwrap();
        assert!(retrieved.is_none());
    }

    #[tokio::test]
    async fn test_list_nodes() {
        let manager = MockClusterManager::new();
        manager.initialize().await.unwrap();

        // Add 3 nodes
        for i in 0..3 {
            let node_id = generate_node_id();
            let node =
                MockClusterManager::create_test_node(node_id, &format!("10.0.0.{}:8080", i + 1));
            manager.add_node(node).await.unwrap();
        }

        let nodes = manager.list_nodes().await.unwrap();
        assert_eq!(nodes.len(), 3);
    }

    #[tokio::test]
    async fn test_update_node_status() {
        let manager = MockClusterManager::new();
        manager.initialize().await.unwrap();

        let node_id = generate_node_id();
        let node = MockClusterManager::create_test_node(node_id, "10.0.0.1:8080");

        manager.add_node(node).await.unwrap();

        // Update status
        manager
            .update_node_status(&node_id, NodeStatus::NotReady)
            .await
            .unwrap();

        let retrieved = manager.get_node(&node_id).await.unwrap().unwrap();
        assert_eq!(retrieved.status, NodeStatus::NotReady);
    }

    #[tokio::test]
    async fn test_event_subscription() {
        let manager = MockClusterManager::new();
        manager.initialize().await.unwrap();

        let mut event_rx = manager.subscribe_to_events().await.unwrap();

        let node_id = generate_node_id();
        let node = MockClusterManager::create_test_node(node_id, "10.0.0.1:8080");

        // Add node
        manager.add_node(node.clone()).await.unwrap();

        // Check for event - broadcast channel receives the actual event
        let event = event_rx.recv().await.unwrap();
        assert!(matches!(event, ClusterEvent::NodeAdded(_)));

        // Remove node
        manager.remove_node(&node_id).await.unwrap();

        let event = event_rx.recv().await.unwrap();
        assert!(matches!(event, ClusterEvent::NodeRemoved(_)));
    }

    #[tokio::test]
    async fn test_get_nonexistent_node() {
        let manager = MockClusterManager::new();
        manager.initialize().await.unwrap();

        let nonexistent_id = generate_node_id();
        let result = manager.get_node(&nonexistent_id).await.unwrap();
        assert!(result.is_none());
    }
}
