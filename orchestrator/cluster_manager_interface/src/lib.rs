use async_trait::async_trait;
use downcast_rs::{impl_downcast, Downcast};
use orchestrator_shared_types::{Node, NodeId, OrchestrationError, Result};
use tokio::sync::broadcast;

/// Represents an event related to cluster membership or node status.
#[derive(Debug, Clone, PartialEq)]
pub enum ClusterEvent {
    NodeAdded(Node),
    NodeRemoved(NodeId),
    NodeUpdated(Node), // e.g., status change, resource update
}

#[async_trait]
pub trait ClusterManager: Downcast + Send + Sync {
    async fn initialize(&self) -> Result<()>;
    async fn get_node(&self, node_id: &NodeId) -> Result<Option<Node>>;
    async fn list_nodes(&self) -> Result<Vec<Node>>;
    /// Subscribe to cluster events. Uses broadcast channel to ensure all events are delivered.
    async fn subscribe_to_events(&self) -> Result<broadcast::Receiver<ClusterEvent>>;

    // Methods for leader election might go here if the manager handles it.
    // async fn is_leader(&self) -> Result<bool>;

    // Health checking logic would be invoked by this manager.
    // For example, the manager might periodically ping nodes.
}

impl_downcast!(ClusterManager);

// Example of a specific error for this interface
#[derive(Debug, thiserror::Error)]
pub enum ClusterManagerError {
    #[error("Node discovery failed: {0}")]
    DiscoveryFailed(String),
    #[error("Node health check failed for {0}: {1}")]
    HealthCheckFailed(NodeId, String),
    #[error("Communication error with peer: {0}")]
    PeerCommunicationError(String),
    #[error("Subscription failed: {0}")]
    SubscriptionFailed(String),
}

impl From<ClusterManagerError> for OrchestrationError {
    fn from(err: ClusterManagerError) -> Self {
        OrchestrationError::ClusterError(err.to_string())
    }
}
