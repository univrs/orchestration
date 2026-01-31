//! Chitchat-based cluster manager implementation.
//!
//! This provides a gossip-based cluster membership system using the chitchat library.
//! Chitchat implements a CRDT-based gossip protocol with phi-accrual failure detection.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use chitchat::transport::UdpTransport;
use chitchat::{spawn_chitchat, ChitchatConfig, ChitchatHandle, ChitchatId, FailureDetectorConfig};
use tokio::sync::{broadcast, Mutex, RwLock};
use tracing::{debug, error, info, warn};

use cluster_manager_interface::{ClusterEvent, ClusterManager, ClusterManagerError};
use orchestrator_shared_types::{
    Keypair, Node, NodeId, NodeResources, NodeStatus, OrchestrationError, Result,
};

/// Buffer size for the broadcast channel - enough to handle burst of node joins
const EVENT_CHANNEL_CAPACITY: usize = 64;

/// Key prefix for node metadata in chitchat's key-value store
const NODE_ADDRESS_KEY: &str = "node:address";
const NODE_STATUS_KEY: &str = "node:status";
const NODE_CPU_CAPACITY_KEY: &str = "node:cpu_capacity";
const NODE_MEMORY_CAPACITY_KEY: &str = "node:memory_mb_capacity";
const NODE_DISK_CAPACITY_KEY: &str = "node:disk_mb_capacity";
const NODE_CPU_ALLOCATABLE_KEY: &str = "node:cpu_allocatable";
const NODE_MEMORY_ALLOCATABLE_KEY: &str = "node:memory_mb_allocatable";
const NODE_DISK_ALLOCATABLE_KEY: &str = "node:disk_mb_allocatable";
const NODE_LABELS_KEY: &str = "node:labels";

/// Configuration for the ChitchatClusterManager.
#[derive(Debug, Clone)]
pub struct ChitchatClusterConfig {
    /// The node ID for this instance (UUID string).
    pub node_id: String,
    /// The address this node will listen on.
    pub listen_addr: SocketAddr,
    /// The address this node will advertise to peers.
    pub public_addr: SocketAddr,
    /// Seed nodes to connect to on startup.
    pub seed_nodes: Vec<String>,
    /// Gossip interval for sending state.
    pub gossip_interval: Duration,
    /// Failure detection configuration.
    pub failure_detection_threshold: f64,
    /// Interval for marking dead nodes.
    pub dead_node_grace_period: Duration,
    /// Cluster ID for namespace isolation.
    pub cluster_id: String,
    /// Generation ID (incremented on restarts).
    pub generation_id: u64,
}

impl Default for ChitchatClusterConfig {
    fn default() -> Self {
        Self {
            node_id: Keypair::generate().public_key().to_string(),
            listen_addr: "0.0.0.0:7280".parse().unwrap(),
            public_addr: "127.0.0.1:7280".parse().unwrap(),
            seed_nodes: Vec::new(),
            gossip_interval: Duration::from_millis(500),
            failure_detection_threshold: 8.0,
            dead_node_grace_period: Duration::from_secs(60),
            cluster_id: "orchestrator-cluster".to_string(),
            generation_id: 0,
        }
    }
}

/// Gossip-based cluster manager using chitchat.
#[derive(Clone)]
pub struct ChitchatClusterManager {
    /// Configuration for this cluster manager.
    config: ChitchatClusterConfig,
    /// Chitchat handle for interacting with the gossip network.
    chitchat_handle: Arc<RwLock<Option<ChitchatHandle>>>,
    /// Local cache of nodes for quick lookups.
    nodes_cache: Arc<RwLock<HashMap<NodeId, Node>>>,
    /// Event broadcaster - uses broadcast channel to ensure all events are delivered.
    event_tx: Arc<broadcast::Sender<ClusterEvent>>,
    /// Whether the cluster manager is initialized.
    initialized: Arc<RwLock<bool>>,
    /// This node's info (set on initialization).
    self_node: Arc<RwLock<Option<Node>>>,
    /// Shared reference to the inner Chitchat for monitoring.
    chitchat_inner: Arc<Mutex<Option<Arc<Mutex<chitchat::Chitchat>>>>>,
}

impl std::fmt::Debug for ChitchatClusterManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChitchatClusterManager")
            .field("config", &self.config)
            .field("initialized", &self.initialized)
            .finish()
    }
}

impl ChitchatClusterManager {
    /// Create a new chitchat cluster manager with the given configuration.
    pub fn new(config: ChitchatClusterConfig) -> Self {
        let (event_tx, _) = broadcast::channel(EVENT_CHANNEL_CAPACITY);

        Self {
            config,
            chitchat_handle: Arc::new(RwLock::new(None)),
            nodes_cache: Arc::new(RwLock::new(HashMap::new())),
            event_tx: Arc::new(event_tx),
            initialized: Arc::new(RwLock::new(false)),
            self_node: Arc::new(RwLock::new(None)),
            chitchat_inner: Arc::new(Mutex::new(None)),
        }
    }

    /// Create a new chitchat cluster manager with default configuration.
    pub fn with_defaults(node_id: NodeId, listen_addr: SocketAddr) -> Self {
        let config = ChitchatClusterConfig {
            node_id: node_id.to_string(),
            listen_addr,
            public_addr: listen_addr,
            ..Default::default()
        };
        Self::new(config)
    }

    /// Set this node's information (resources, labels, etc.).
    pub async fn set_self_node(&self, node: Node) {
        *self.self_node.write().await = Some(node);
    }

    /// Parse a NodeId (Ed25519 PublicKey) from a chitchat node ID string.
    fn parse_node_id(chitchat_id: &str) -> Option<NodeId> {
        chitchat_id.parse().ok()
    }

    /// Parse node status from string.
    fn parse_node_status(status_str: &str) -> NodeStatus {
        match status_str.to_lowercase().as_str() {
            "ready" => NodeStatus::Ready,
            "notready" => NodeStatus::NotReady,
            "down" => NodeStatus::Down,
            _ => NodeStatus::Unknown,
        }
    }

    /// Build a Node from chitchat state.
    fn build_node_from_state(node_id: NodeId, state: &HashMap<String, String>) -> Node {
        let address = state
            .get(NODE_ADDRESS_KEY)
            .cloned()
            .unwrap_or_else(|| "unknown".to_string());

        let status = state
            .get(NODE_STATUS_KEY)
            .map(|s| Self::parse_node_status(s))
            .unwrap_or(NodeStatus::Unknown);

        let labels: HashMap<String, String> = state
            .get(NODE_LABELS_KEY)
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default();

        let resources_capacity = NodeResources {
            cpu_cores: state
                .get(NODE_CPU_CAPACITY_KEY)
                .and_then(|s| s.parse().ok())
                .unwrap_or(0.0),
            memory_mb: state
                .get(NODE_MEMORY_CAPACITY_KEY)
                .and_then(|s| s.parse().ok())
                .unwrap_or(0),
            disk_mb: state
                .get(NODE_DISK_CAPACITY_KEY)
                .and_then(|s| s.parse().ok())
                .unwrap_or(0),
        };

        let resources_allocatable = NodeResources {
            cpu_cores: state
                .get(NODE_CPU_ALLOCATABLE_KEY)
                .and_then(|s| s.parse().ok())
                .unwrap_or(0.0),
            memory_mb: state
                .get(NODE_MEMORY_ALLOCATABLE_KEY)
                .and_then(|s| s.parse().ok())
                .unwrap_or(0),
            disk_mb: state
                .get(NODE_DISK_ALLOCATABLE_KEY)
                .and_then(|s| s.parse().ok())
                .unwrap_or(0),
        };

        Node {
            id: node_id,
            address,
            status,
            labels,
            resources_capacity,
            resources_allocatable,
        }
    }

    /// Publish this node's state to chitchat.
    async fn publish_self_state(&self) -> Result<()> {
        let handle_guard = self.chitchat_handle.read().await;
        let handle = handle_guard
            .as_ref()
            .ok_or_else(|| OrchestrationError::ClusterError("Not initialized".to_string()))?;

        let self_node_guard = self.self_node.read().await;
        if let Some(ref node) = *self_node_guard {
            let chitchat = handle.chitchat();
            let mut chitchat_guard = chitchat.lock().await;

            // Publish node metadata
            chitchat_guard
                .self_node_state()
                .set(NODE_ADDRESS_KEY, node.address.clone());
            chitchat_guard
                .self_node_state()
                .set(NODE_STATUS_KEY, format!("{:?}", node.status));
            chitchat_guard.self_node_state().set(
                NODE_CPU_CAPACITY_KEY,
                node.resources_capacity.cpu_cores.to_string(),
            );
            chitchat_guard.self_node_state().set(
                NODE_MEMORY_CAPACITY_KEY,
                node.resources_capacity.memory_mb.to_string(),
            );
            chitchat_guard.self_node_state().set(
                NODE_DISK_CAPACITY_KEY,
                node.resources_capacity.disk_mb.to_string(),
            );
            chitchat_guard.self_node_state().set(
                NODE_CPU_ALLOCATABLE_KEY,
                node.resources_allocatable.cpu_cores.to_string(),
            );
            chitchat_guard.self_node_state().set(
                NODE_MEMORY_ALLOCATABLE_KEY,
                node.resources_allocatable.memory_mb.to_string(),
            );
            chitchat_guard.self_node_state().set(
                NODE_DISK_ALLOCATABLE_KEY,
                node.resources_allocatable.disk_mb.to_string(),
            );

            // Serialize labels as JSON
            let labels_json =
                serde_json::to_string(&node.labels).unwrap_or_else(|_| "{}".to_string());
            chitchat_guard
                .self_node_state()
                .set(NODE_LABELS_KEY, labels_json);

            debug!("Published self node state for {}", node.id);
        }

        Ok(())
    }

    /// Start the membership change monitoring task.
    fn start_membership_monitor(
        chitchat_arc: Arc<Mutex<chitchat::Chitchat>>,
        nodes_cache: Arc<RwLock<HashMap<NodeId, Node>>>,
        event_tx: Arc<broadcast::Sender<ClusterEvent>>,
    ) {
        tokio::spawn(async move {
            // Get the watcher from the inner Chitchat
            let watcher = {
                let chitchat_guard = chitchat_arc.lock().await;
                chitchat_guard.live_nodes_watcher()
            };
            let mut watcher = watcher;

            loop {
                // Wait for membership changes
                if watcher.changed().await.is_err() {
                    warn!("Membership watcher channel closed");
                    break;
                }

                let live_nodes = watcher.borrow().clone();

                let mut new_nodes: HashMap<NodeId, Node> = HashMap::new();

                // Process each live node (BTreeMap<ChitchatId, NodeState>)
                for (chitchat_id, node_state) in &live_nodes {
                    let id_str = chitchat_id.node_id.as_str();
                    if let Some(node_id) = Self::parse_node_id(id_str) {
                        let state: HashMap<String, String> = node_state
                            .key_values()
                            .map(|(k, v)| (k.to_string(), v.to_string()))
                            .collect();

                        let node = Self::build_node_from_state(node_id, &state);
                        new_nodes.insert(node_id, node);
                    }
                }

                // Compare with cached nodes to detect changes
                let mut cache = nodes_cache.write().await;

                // Find added nodes - broadcast sends to ALL subscribers (no lost events)
                for (node_id, node) in &new_nodes {
                    if !cache.contains_key(node_id) {
                        info!("Node joined cluster: {}", node_id);
                        let _ = event_tx.send(ClusterEvent::NodeAdded(node.clone()));
                    } else if cache.get(node_id) != Some(node) {
                        info!("Node updated: {}", node_id);
                        let _ = event_tx.send(ClusterEvent::NodeUpdated(node.clone()));
                    }
                }

                // Find removed nodes
                for node_id in cache.keys() {
                    if !new_nodes.contains_key(node_id) {
                        info!("Node left cluster: {}", node_id);
                        let _ = event_tx.send(ClusterEvent::NodeRemoved(*node_id));
                    }
                }

                // Update cache
                *cache = new_nodes;
            }
        });
    }

    /// Update this node's status in the cluster.
    pub async fn update_self_status(&self, status: NodeStatus) -> Result<()> {
        {
            let mut self_node = self.self_node.write().await;
            if let Some(ref mut node) = *self_node {
                node.status = status;
            }
        }
        self.publish_self_state().await
    }

    /// Update this node's allocatable resources.
    pub async fn update_self_resources(&self, allocatable: NodeResources) -> Result<()> {
        {
            let mut self_node = self.self_node.write().await;
            if let Some(ref mut node) = *self_node {
                node.resources_allocatable = allocatable;
            }
        }
        self.publish_self_state().await
    }
}

#[async_trait]
impl ClusterManager for ChitchatClusterManager {
    async fn initialize(&self) -> Result<()> {
        info!(
            "ChitchatClusterManager: Initializing with node_id={}, listen_addr={}",
            self.config.node_id, self.config.listen_addr
        );

        // Create chitchat ID with generation_id
        let chitchat_id = ChitchatId::new(
            self.config.node_id.clone(),
            self.config.generation_id,
            self.config.public_addr,
        );

        let failure_detector_config = FailureDetectorConfig {
            phi_threshold: self.config.failure_detection_threshold,
            initial_interval: self.config.gossip_interval,
            ..Default::default()
        };

        let chitchat_config = ChitchatConfig {
            cluster_id: self.config.cluster_id.clone(),
            chitchat_id,
            gossip_interval: self.config.gossip_interval,
            listen_addr: self.config.listen_addr,
            seed_nodes: self.config.seed_nodes.clone(),
            failure_detector_config,
            marked_for_deletion_grace_period: self.config.dead_node_grace_period,
            catchup_callback: None,
            extra_liveness_predicate: None,
        };

        // Create UDP transport
        let transport = UdpTransport;

        // Spawn chitchat
        let handle = spawn_chitchat(chitchat_config, Vec::new(), &transport)
            .await
            .map_err(|e| {
                error!("Failed to spawn chitchat: {}", e);
                ClusterManagerError::DiscoveryFailed(e.to_string())
            })?;

        // Get reference to inner Chitchat for monitoring
        let chitchat_arc = handle.chitchat();

        // Store the inner chitchat reference
        *self.chitchat_inner.lock().await = Some(Arc::clone(&chitchat_arc));

        // Store the handle
        *self.chitchat_handle.write().await = Some(handle);

        // Start membership monitoring
        Self::start_membership_monitor(
            chitchat_arc,
            Arc::clone(&self.nodes_cache),
            self.event_tx.clone(),
        );

        // Publish our initial state
        self.publish_self_state().await?;

        *self.initialized.write().await = true;

        info!("ChitchatClusterManager: Initialization complete");
        Ok(())
    }

    async fn get_node(&self, node_id: &NodeId) -> Result<Option<Node>> {
        debug!("ChitchatClusterManager: Getting node {}", node_id);

        // First check the cache
        let cache = self.nodes_cache.read().await;
        if let Some(node) = cache.get(node_id) {
            return Ok(Some(node.clone()));
        }
        drop(cache);

        // If not in cache, try to get from chitchat directly
        let chitchat_inner_guard = self.chitchat_inner.lock().await;
        if let Some(ref chitchat_arc) = *chitchat_inner_guard {
            let chitchat_guard = chitchat_arc.lock().await;

            // Find the node in chitchat's state (live_nodes returns iterator of &ChitchatId)
            for chitchat_id in chitchat_guard.live_nodes() {
                if let Some(parsed_id) = Self::parse_node_id(chitchat_id.node_id.as_str()) {
                    if parsed_id == *node_id {
                        if let Some(node_state) = chitchat_guard.node_state(chitchat_id) {
                            let state: HashMap<String, String> = node_state
                                .key_values()
                                .map(|(k, v)| (k.to_string(), v.to_string()))
                                .collect();

                            let node = Self::build_node_from_state(parsed_id, &state);
                            return Ok(Some(node));
                        }
                    }
                }
            }
        }

        Ok(None)
    }

    async fn list_nodes(&self) -> Result<Vec<Node>> {
        debug!("ChitchatClusterManager: Listing all nodes");

        // Return cached nodes
        let cache = self.nodes_cache.read().await;
        Ok(cache.values().cloned().collect())
    }

    async fn subscribe_to_events(&self) -> Result<broadcast::Receiver<ClusterEvent>> {
        debug!("ChitchatClusterManager: Creating event subscription");
        Ok(self.event_tx.subscribe())
    }
}

impl Drop for ChitchatClusterManager {
    fn drop(&mut self) {
        info!("ChitchatClusterManager: Shutting down");
        // The ChitchatHandle will be dropped automatically, which shuts down the gossip server
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn generate_node_id() -> NodeId {
        Keypair::generate().public_key()
    }

    #[test]
    fn test_parse_node_id() {
        // Generate a valid public key and convert to string
        let node_id = generate_node_id();
        let valid_key = node_id.to_string();
        assert!(ChitchatClusterManager::parse_node_id(&valid_key).is_some());

        let invalid = "not-a-valid-public-key";
        assert!(ChitchatClusterManager::parse_node_id(invalid).is_none());
    }

    #[test]
    fn test_parse_node_status() {
        assert_eq!(
            ChitchatClusterManager::parse_node_status("Ready"),
            NodeStatus::Ready
        );
        assert_eq!(
            ChitchatClusterManager::parse_node_status("NotReady"),
            NodeStatus::NotReady
        );
        assert_eq!(
            ChitchatClusterManager::parse_node_status("Down"),
            NodeStatus::Down
        );
        assert_eq!(
            ChitchatClusterManager::parse_node_status("unknown_status"),
            NodeStatus::Unknown
        );
    }

    #[test]
    fn test_build_node_from_state() {
        let node_id = generate_node_id();
        let mut state = HashMap::new();

        state.insert(NODE_ADDRESS_KEY.to_string(), "10.0.0.1:8080".to_string());
        state.insert(NODE_STATUS_KEY.to_string(), "Ready".to_string());
        state.insert(NODE_CPU_CAPACITY_KEY.to_string(), "4.0".to_string());
        state.insert(NODE_MEMORY_CAPACITY_KEY.to_string(), "8192".to_string());
        state.insert(NODE_DISK_CAPACITY_KEY.to_string(), "102400".to_string());
        state.insert(NODE_CPU_ALLOCATABLE_KEY.to_string(), "3.5".to_string());
        state.insert(NODE_MEMORY_ALLOCATABLE_KEY.to_string(), "7168".to_string());
        state.insert(NODE_DISK_ALLOCATABLE_KEY.to_string(), "92160".to_string());
        state.insert(
            NODE_LABELS_KEY.to_string(),
            r#"{"zone":"us-east-1"}"#.to_string(),
        );

        let node = ChitchatClusterManager::build_node_from_state(node_id, &state);

        assert_eq!(node.id, node_id);
        assert_eq!(node.address, "10.0.0.1:8080");
        assert_eq!(node.status, NodeStatus::Ready);
        assert_eq!(node.resources_capacity.cpu_cores, 4.0);
        assert_eq!(node.resources_capacity.memory_mb, 8192);
        assert_eq!(node.labels.get("zone"), Some(&"us-east-1".to_string()));
    }

    #[test]
    fn test_default_config() {
        let config = ChitchatClusterConfig::default();
        assert_eq!(config.cluster_id, "orchestrator-cluster");
        assert_eq!(config.gossip_interval, Duration::from_millis(500));
        assert_eq!(config.failure_detection_threshold, 8.0);
        assert_eq!(config.generation_id, 0);
    }

    #[tokio::test]
    async fn test_new_cluster_manager() {
        let config = ChitchatClusterConfig::default();
        let manager = ChitchatClusterManager::new(config);

        assert!(!*manager.initialized.read().await);
        assert!(manager.chitchat_handle.read().await.is_none());
    }

    // Note: Integration tests that require actual network would go in tests/
    // These tests validate the logic without requiring a network.
}
