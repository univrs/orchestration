//! WebSocket event streaming for real-time cluster updates.
//!
//! Provides a WebSocket endpoint at `/api/v1/events` that allows clients
//! to subscribe to workload, node, and cluster events.
//!
//! # Example
//!
//! ```text
//! // WebSocket connection to ws://host:9090/api/v1/events
//! // Send subscription message:
//! {
//!   "type": "subscribe",
//!   "topics": ["workloads", "nodes", "cluster"]
//! }
//!
//! // Receive events:
//! {
//!   "type": "node_added",
//!   "timestamp": "2024-01-15T10:30:00Z",
//!   "data": { "id": "...", "status": "Ready", ... }
//! }
//! ```

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, RwLock};
use uuid::Uuid;

use cluster_manager_interface::ClusterEvent;
use orchestrator_shared_types::{
    Node, NodeId, WorkloadDefinition, WorkloadId, WorkloadInstance,
    WorkloadInstanceStatus,
};

#[cfg(test)]
use orchestrator_shared_types::{NodeStatus, Keypair};

/// Event topics that clients can subscribe to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventTopic {
    /// Workload lifecycle events (created, updated, deleted, scaled).
    Workloads,
    /// Node membership events (added, removed, status changes).
    Nodes,
    /// Cluster-wide summary events (health changes, capacity updates).
    Cluster,
    /// All events.
    All,
    /// Resource availability gradient signals (P2P).
    Gradient,
    /// Leader election consensus messages (P2P).
    Election,
    /// Economic/credit transactions (P2P).
    Credit,
    /// Coordination barriers and synchronization (P2P).
    Septal,
}

impl EventTopic {
    /// Check if this is a P2P network topic.
    pub fn is_p2p_topic(&self) -> bool {
        matches!(
            self,
            EventTopic::Gradient | EventTopic::Election | EventTopic::Credit | EventTopic::Septal
        )
    }

    /// Convert to gossipsub topic string.
    pub fn to_gossipsub_topic(&self) -> Option<String> {
        match self {
            EventTopic::Gradient => Some("/orchestrator/gradient/1.0.0".to_string()),
            EventTopic::Election => Some("/orchestrator/election/1.0.0".to_string()),
            EventTopic::Credit => Some("/orchestrator/credit/1.0.0".to_string()),
            EventTopic::Septal => Some("/orchestrator/septal/1.0.0".to_string()),
            _ => None, // Non-P2P topics don't map to gossipsub
        }
    }

    /// Create from gossipsub topic string.
    pub fn from_gossipsub_topic(topic: &str) -> Option<Self> {
        match topic {
            "/orchestrator/gradient/1.0.0" => Some(EventTopic::Gradient),
            "/orchestrator/election/1.0.0" => Some(EventTopic::Election),
            "/orchestrator/credit/1.0.0" => Some(EventTopic::Credit),
            "/orchestrator/septal/1.0.0" => Some(EventTopic::Septal),
            _ => None,
        }
    }
}

impl EventTopic {
    /// Check if this topic matches a given event.
    pub fn matches(&self, event: &StreamEvent) -> bool {
        match self {
            EventTopic::All => true,
            EventTopic::Workloads => matches!(
                event.event_type,
                EventType::WorkloadCreated
                    | EventType::WorkloadUpdated
                    | EventType::WorkloadDeleted
                    | EventType::WorkloadScaled
                    | EventType::InstanceStatusChanged
            ),
            EventTopic::Nodes => matches!(
                event.event_type,
                EventType::NodeAdded | EventType::NodeRemoved | EventType::NodeUpdated
            ),
            EventTopic::Cluster => matches!(
                event.event_type,
                EventType::ClusterHealthChanged | EventType::ClusterCapacityChanged
            ),
            // P2P network topics
            EventTopic::Gradient => matches!(
                event.event_type,
                EventType::ResourceGradientUpdate
            ),
            EventTopic::Election => matches!(
                event.event_type,
                EventType::LeaderElectionMessage
            ),
            EventTopic::Credit => matches!(
                event.event_type,
                EventType::CreditTransaction
            ),
            EventTopic::Septal => matches!(
                event.event_type,
                EventType::SeptalCoordination
            ),
        }
    }
}

/// Event types for streaming.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    // Node events
    NodeAdded,
    NodeRemoved,
    NodeUpdated,

    // Workload events
    WorkloadCreated,
    WorkloadUpdated,
    WorkloadDeleted,
    WorkloadScaled,
    InstanceStatusChanged,

    // Cluster events
    ClusterHealthChanged,
    ClusterCapacityChanged,

    // Connection events
    Connected,
    Subscribed,
    Unsubscribed,
    Error,
    Heartbeat,

    // P2P network events (gossipsub)
    /// Resource availability gradient update from network.
    ResourceGradientUpdate,
    /// Leader election message from network.
    LeaderElectionMessage,
    /// Credit/economic transaction from network.
    CreditTransaction,
    /// Septal coordination/barrier message from network.
    SeptalCoordination,

    // Network bridge events
    /// Message received from P2P network.
    NetworkMessageReceived,
    /// Message published to P2P network.
    NetworkMessagePublished,
    /// Peer discovered on network.
    PeerDiscovered,
    /// Peer removed from network.
    PeerRemoved,
}

/// A streamable event sent to WebSocket clients.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamEvent {
    /// Unique event ID.
    pub id: String,
    /// Event type.
    #[serde(rename = "type")]
    pub event_type: EventType,
    /// When the event occurred.
    pub timestamp: DateTime<Utc>,
    /// Event payload (varies by event type).
    pub data: serde_json::Value,
    /// Optional correlation ID for tracing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<String>,
}

impl StreamEvent {
    /// Create a new event.
    pub fn new(event_type: EventType, data: impl Serialize) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            event_type,
            timestamp: Utc::now(),
            data: serde_json::to_value(data).unwrap_or(serde_json::Value::Null),
            correlation_id: None,
        }
    }

    /// Create an event with correlation ID.
    pub fn with_correlation(mut self, correlation_id: impl Into<String>) -> Self {
        self.correlation_id = Some(correlation_id.into());
        self
    }

    /// Create a connected event.
    pub fn connected(client_id: &str) -> Self {
        Self::new(
            EventType::Connected,
            serde_json::json!({
                "client_id": client_id,
                "message": "Connected to event stream"
            }),
        )
    }

    /// Create a subscribed event.
    pub fn subscribed(topics: &[EventTopic]) -> Self {
        Self::new(
            EventType::Subscribed,
            serde_json::json!({
                "topics": topics,
                "message": "Successfully subscribed to topics"
            }),
        )
    }

    /// Create a heartbeat event.
    pub fn heartbeat() -> Self {
        Self::new(EventType::Heartbeat, serde_json::json!({}))
    }

    /// Create an error event.
    pub fn error(message: impl Into<String>) -> Self {
        Self::new(
            EventType::Error,
            serde_json::json!({
                "error": message.into()
            }),
        )
    }

    /// Create from a ClusterEvent.
    pub fn from_cluster_event(event: &ClusterEvent) -> Self {
        match event {
            ClusterEvent::NodeAdded(node) => Self::new(EventType::NodeAdded, NodeEventData::from(node)),
            ClusterEvent::NodeRemoved(node_id) => Self::new(
                EventType::NodeRemoved,
                serde_json::json!({ "node_id": node_id }),
            ),
            ClusterEvent::NodeUpdated(node) => {
                Self::new(EventType::NodeUpdated, NodeEventData::from(node))
            }
        }
    }
}

/// Node event data payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeEventData {
    pub node_id: String,
    pub address: String,
    pub status: String,
    pub labels: HashMap<String, String>,
    pub cpu_capacity: f32,
    pub memory_capacity_mb: u64,
    pub cpu_allocatable: f32,
    pub memory_allocatable_mb: u64,
}

impl From<&Node> for NodeEventData {
    fn from(node: &Node) -> Self {
        Self {
            node_id: node.id.to_string(),
            address: node.address.clone(),
            status: format!("{:?}", node.status),
            labels: node.labels.clone(),
            cpu_capacity: node.resources_capacity.cpu_cores,
            memory_capacity_mb: node.resources_capacity.memory_mb,
            cpu_allocatable: node.resources_allocatable.cpu_cores,
            memory_allocatable_mb: node.resources_allocatable.memory_mb,
        }
    }
}

/// Workload event data payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkloadEventData {
    pub workload_id: String,
    pub name: String,
    pub replicas: u32,
    pub labels: HashMap<String, String>,
    pub container_count: usize,
}

impl From<&WorkloadDefinition> for WorkloadEventData {
    fn from(workload: &WorkloadDefinition) -> Self {
        Self {
            workload_id: workload.id.to_string(),
            name: workload.name.clone(),
            replicas: workload.replicas,
            labels: workload.labels.clone(),
            container_count: workload.containers.len(),
        }
    }
}

/// Instance status change event data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstanceEventData {
    pub instance_id: String,
    pub workload_id: String,
    pub node_id: String,
    pub status: String,
    pub container_ids: Vec<String>,
}

impl From<&WorkloadInstance> for InstanceEventData {
    fn from(instance: &WorkloadInstance) -> Self {
        Self {
            instance_id: instance.id.to_string(),
            workload_id: instance.workload_id.to_string(),
            node_id: instance.node_id.to_string(),
            status: format!("{:?}", instance.status),
            container_ids: instance.container_ids.clone(),
        }
    }
}

/// Cluster health event data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterHealthEventData {
    pub status: String,
    pub total_nodes: usize,
    pub ready_nodes: usize,
    pub total_workloads: usize,
    pub running_instances: usize,
}

/// Cluster capacity event data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterCapacityEventData {
    pub total_cpu_capacity: f32,
    pub total_memory_capacity_mb: u64,
    pub used_cpu: f32,
    pub used_memory_mb: u64,
    pub cpu_utilization_percent: f32,
    pub memory_utilization_percent: f32,
}

/// Messages sent from clients to the server.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    /// Subscribe to event topics.
    Subscribe { topics: Vec<EventTopic> },
    /// Unsubscribe from event topics.
    Unsubscribe { topics: Vec<EventTopic> },
    /// Ping for keep-alive.
    Ping,

    // P2P Network Actions

    /// Publish a gradient message to the network.
    PublishGradient {
        /// CPU availability (0.0-1.0).
        cpu_available: f32,
        /// Memory availability (0.0-1.0).
        memory_available: f32,
        /// Disk availability (0.0-1.0).
        disk_available: f32,
    },

    /// Publish an election vote/message.
    PublishElection {
        /// Election round number.
        round: u64,
        /// Ballot number.
        ballot: u64,
        /// Phase: "prepare", "promise", "accept", "accepted", "elected".
        phase: String,
        /// Candidate/proposer node ID.
        #[serde(skip_serializing_if = "Option::is_none")]
        proposer: Option<String>,
    },

    /// Publish a credit transaction.
    PublishCredit {
        /// Recipient node ID.
        to: String,
        /// Amount to transfer.
        amount: f64,
        /// Optional resource type.
        #[serde(skip_serializing_if = "Option::is_none")]
        resource: Option<String>,
    },

    /// Publish a septal coordination message.
    PublishSeptal {
        /// Barrier type.
        barrier_type: String,
        /// Barrier ID (if joining existing barrier).
        #[serde(skip_serializing_if = "Option::is_none")]
        barrier_id: Option<String>,
        /// Participants.
        #[serde(default)]
        participants: Vec<String>,
        /// Timeout in milliseconds.
        #[serde(default = "default_timeout")]
        timeout_ms: u64,
    },

    /// Request peer list for a topic.
    GetPeers {
        /// Topic to query peers for.
        topic: EventTopic,
    },

    /// Get network bridge statistics.
    GetNetworkStats,
}

fn default_timeout() -> u64 {
    5000
}

/// Subscription state for a connected client.
#[derive(Debug)]
pub struct ClientSubscription {
    pub id: String,
    pub topics: HashSet<EventTopic>,
    pub connected_at: DateTime<Utc>,
    pub last_activity: DateTime<Utc>,
}

impl ClientSubscription {
    pub fn new(id: String) -> Self {
        let now = Utc::now();
        Self {
            id,
            topics: HashSet::new(),
            connected_at: now,
            last_activity: now,
        }
    }

    pub fn subscribe(&mut self, topics: &[EventTopic]) {
        for topic in topics {
            self.topics.insert(*topic);
        }
        self.last_activity = Utc::now();
    }

    pub fn unsubscribe(&mut self, topics: &[EventTopic]) {
        for topic in topics {
            self.topics.remove(topic);
        }
        self.last_activity = Utc::now();
    }

    pub fn should_receive(&self, event: &StreamEvent) -> bool {
        self.topics.iter().any(|topic| topic.matches(event))
    }
}

/// Event hub for managing subscriptions and broadcasting events.
#[derive(Clone)]
pub struct EventHub {
    /// Broadcast channel for events.
    sender: broadcast::Sender<StreamEvent>,
    /// Active subscriptions by client ID.
    subscriptions: Arc<RwLock<HashMap<String, ClientSubscription>>>,
    /// Metrics.
    metrics: Arc<RwLock<EventHubMetrics>>,
}

/// Metrics for the event hub.
#[derive(Debug, Default)]
pub struct EventHubMetrics {
    pub total_events_broadcast: u64,
    pub total_connections: u64,
    pub active_connections: u64,
    pub events_by_type: HashMap<String, u64>,
}

impl EventHub {
    /// Create a new event hub.
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self {
            sender,
            subscriptions: Arc::new(RwLock::new(HashMap::new())),
            metrics: Arc::new(RwLock::new(EventHubMetrics::default())),
        }
    }

    /// Create with default capacity (1024 events).
    pub fn default() -> Self {
        Self::new(1024)
    }

    /// Subscribe to the event stream.
    pub fn subscribe(&self) -> broadcast::Receiver<StreamEvent> {
        self.sender.subscribe()
    }

    /// Broadcast an event to all subscribers.
    pub async fn broadcast(&self, event: StreamEvent) {
        // Update metrics
        {
            let mut metrics = self.metrics.write().await;
            metrics.total_events_broadcast += 1;
            let type_str = format!("{:?}", event.event_type);
            *metrics.events_by_type.entry(type_str).or_insert(0) += 1;
        }

        // Send to broadcast channel
        let _ = self.sender.send(event);
    }

    /// Register a new client connection.
    pub async fn register_client(&self, client_id: String) {
        let subscription = ClientSubscription::new(client_id.clone());

        let mut subs = self.subscriptions.write().await;
        subs.insert(client_id, subscription);

        let mut metrics = self.metrics.write().await;
        metrics.total_connections += 1;
        metrics.active_connections += 1;
    }

    /// Unregister a client connection.
    pub async fn unregister_client(&self, client_id: &str) {
        let mut subs = self.subscriptions.write().await;
        subs.remove(client_id);

        let mut metrics = self.metrics.write().await;
        if metrics.active_connections > 0 {
            metrics.active_connections -= 1;
        }
    }

    /// Update client subscription.
    pub async fn update_subscription(&self, client_id: &str, topics: &[EventTopic], subscribe: bool) {
        let mut subs = self.subscriptions.write().await;
        if let Some(sub) = subs.get_mut(client_id) {
            if subscribe {
                sub.subscribe(topics);
            } else {
                sub.unsubscribe(topics);
            }
        }
    }

    /// Get subscription for a client.
    pub async fn get_subscription(&self, client_id: &str) -> Option<HashSet<EventTopic>> {
        let subs = self.subscriptions.read().await;
        subs.get(client_id).map(|s| s.topics.clone())
    }

    /// Get current metrics.
    pub async fn get_metrics(&self) -> EventHubMetrics {
        let metrics = self.metrics.read().await;
        EventHubMetrics {
            total_events_broadcast: metrics.total_events_broadcast,
            total_connections: metrics.total_connections,
            active_connections: metrics.active_connections,
            events_by_type: metrics.events_by_type.clone(),
        }
    }

    /// Get number of active connections.
    pub async fn active_connections(&self) -> usize {
        self.subscriptions.read().await.len()
    }

    // Event broadcasting helpers

    /// Broadcast a node added event.
    pub async fn broadcast_node_added(&self, node: &Node) {
        let event = StreamEvent::new(EventType::NodeAdded, NodeEventData::from(node));
        self.broadcast(event).await;
    }

    /// Broadcast a node removed event.
    pub async fn broadcast_node_removed(&self, node_id: &NodeId) {
        let event = StreamEvent::new(
            EventType::NodeRemoved,
            serde_json::json!({ "node_id": node_id.to_string() }),
        );
        self.broadcast(event).await;
    }

    /// Broadcast a node updated event.
    pub async fn broadcast_node_updated(&self, node: &Node) {
        let event = StreamEvent::new(EventType::NodeUpdated, NodeEventData::from(node));
        self.broadcast(event).await;
    }

    /// Broadcast a workload created event.
    pub async fn broadcast_workload_created(&self, workload: &WorkloadDefinition) {
        let event = StreamEvent::new(EventType::WorkloadCreated, WorkloadEventData::from(workload));
        self.broadcast(event).await;
    }

    /// Broadcast a workload updated event.
    pub async fn broadcast_workload_updated(&self, workload: &WorkloadDefinition) {
        let event = StreamEvent::new(EventType::WorkloadUpdated, WorkloadEventData::from(workload));
        self.broadcast(event).await;
    }

    /// Broadcast a workload deleted event.
    pub async fn broadcast_workload_deleted(&self, workload_id: &WorkloadId, name: &str) {
        let event = StreamEvent::new(
            EventType::WorkloadDeleted,
            serde_json::json!({
                "workload_id": workload_id.to_string(),
                "name": name
            }),
        );
        self.broadcast(event).await;
    }

    /// Broadcast a workload scaled event.
    pub async fn broadcast_workload_scaled(
        &self,
        workload_id: &WorkloadId,
        name: &str,
        old_replicas: u32,
        new_replicas: u32,
    ) {
        let event = StreamEvent::new(
            EventType::WorkloadScaled,
            serde_json::json!({
                "workload_id": workload_id.to_string(),
                "name": name,
                "old_replicas": old_replicas,
                "new_replicas": new_replicas
            }),
        );
        self.broadcast(event).await;
    }

    /// Broadcast an instance status changed event.
    pub async fn broadcast_instance_status_changed(
        &self,
        instance: &WorkloadInstance,
        old_status: &WorkloadInstanceStatus,
    ) {
        let event = StreamEvent::new(
            EventType::InstanceStatusChanged,
            serde_json::json!({
                "instance": InstanceEventData::from(instance),
                "old_status": format!("{:?}", old_status),
                "new_status": format!("{:?}", instance.status)
            }),
        );
        self.broadcast(event).await;
    }

    /// Broadcast a cluster health changed event.
    pub async fn broadcast_cluster_health_changed(&self, data: ClusterHealthEventData) {
        let event = StreamEvent::new(EventType::ClusterHealthChanged, data);
        self.broadcast(event).await;
    }

    /// Broadcast a cluster capacity changed event.
    pub async fn broadcast_cluster_capacity_changed(&self, data: ClusterCapacityEventData) {
        let event = StreamEvent::new(EventType::ClusterCapacityChanged, data);
        self.broadcast(event).await;
    }

    /// Broadcast a ClusterEvent from cluster_manager_interface.
    pub async fn broadcast_cluster_event(&self, event: &ClusterEvent) {
        let stream_event = StreamEvent::from_cluster_event(event);
        self.broadcast(stream_event).await;
    }
}

impl Default for EventHub {
    fn default() -> Self {
        Self::new(1024)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_topic_matches() {
        let node_event = StreamEvent::new(EventType::NodeAdded, serde_json::json!({}));
        let workload_event = StreamEvent::new(EventType::WorkloadCreated, serde_json::json!({}));
        let cluster_event = StreamEvent::new(EventType::ClusterHealthChanged, serde_json::json!({}));

        assert!(EventTopic::Nodes.matches(&node_event));
        assert!(!EventTopic::Nodes.matches(&workload_event));

        assert!(EventTopic::Workloads.matches(&workload_event));
        assert!(!EventTopic::Workloads.matches(&node_event));

        assert!(EventTopic::Cluster.matches(&cluster_event));
        assert!(!EventTopic::Cluster.matches(&node_event));

        assert!(EventTopic::All.matches(&node_event));
        assert!(EventTopic::All.matches(&workload_event));
        assert!(EventTopic::All.matches(&cluster_event));
    }

    #[test]
    fn test_stream_event_serialization() {
        let event = StreamEvent::new(
            EventType::NodeAdded,
            serde_json::json!({ "node_id": "test" }),
        );

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"node_added\""));
        assert!(json.contains("\"node_id\":\"test\""));
    }

    #[test]
    fn test_client_subscription() {
        let mut sub = ClientSubscription::new("client-1".to_string());

        assert!(sub.topics.is_empty());

        sub.subscribe(&[EventTopic::Nodes, EventTopic::Workloads]);
        assert!(sub.topics.contains(&EventTopic::Nodes));
        assert!(sub.topics.contains(&EventTopic::Workloads));

        let node_event = StreamEvent::new(EventType::NodeAdded, serde_json::json!({}));
        assert!(sub.should_receive(&node_event));

        sub.unsubscribe(&[EventTopic::Nodes]);
        assert!(!sub.topics.contains(&EventTopic::Nodes));
        assert!(!sub.should_receive(&node_event));
    }

    #[tokio::test]
    async fn test_event_hub_broadcast() {
        let hub = EventHub::new(16);
        let mut rx = hub.subscribe();

        let event = StreamEvent::new(EventType::NodeAdded, serde_json::json!({ "test": true }));
        hub.broadcast(event.clone()).await;

        let received = rx.recv().await.unwrap();
        assert_eq!(received.event_type, EventType::NodeAdded);
    }

    #[tokio::test]
    async fn test_event_hub_client_registration() {
        let hub = EventHub::new(16);

        hub.register_client("client-1".to_string()).await;
        assert_eq!(hub.active_connections().await, 1);

        hub.update_subscription("client-1", &[EventTopic::Nodes], true).await;
        let topics = hub.get_subscription("client-1").await.unwrap();
        assert!(topics.contains(&EventTopic::Nodes));

        hub.unregister_client("client-1").await;
        assert_eq!(hub.active_connections().await, 0);
    }

    #[tokio::test]
    async fn test_event_hub_metrics() {
        let hub = EventHub::new(16);
        let _rx = hub.subscribe();

        hub.broadcast(StreamEvent::new(EventType::NodeAdded, serde_json::json!({}))).await;
        hub.broadcast(StreamEvent::new(EventType::NodeAdded, serde_json::json!({}))).await;

        let metrics = hub.get_metrics().await;
        assert_eq!(metrics.total_events_broadcast, 2);
        assert_eq!(*metrics.events_by_type.get("NodeAdded").unwrap_or(&0), 2);
    }

    #[test]
    fn test_node_event_data_from_node() {
        let node = Node {
            id: Keypair::generate().public_key(),
            address: "10.0.0.1:8080".to_string(),
            status: NodeStatus::Ready,
            labels: HashMap::from([("zone".to_string(), "us-east-1".to_string())]),
            resources_capacity: orchestrator_shared_types::NodeResources {
                cpu_cores: 4.0,
                memory_mb: 8192,
                disk_mb: 100000,
            },
            resources_allocatable: orchestrator_shared_types::NodeResources {
                cpu_cores: 3.6,
                memory_mb: 7372,
                disk_mb: 90000,
            },
        };

        let data = NodeEventData::from(&node);
        assert_eq!(data.status, "Ready");
        assert_eq!(data.cpu_capacity, 4.0);
        assert_eq!(data.memory_capacity_mb, 8192);
    }

    #[test]
    fn test_client_message_deserialization() {
        let json = r#"{"type": "subscribe", "topics": ["nodes", "workloads"]}"#;
        let msg: ClientMessage = serde_json::from_str(json).unwrap();

        match msg {
            ClientMessage::Subscribe { topics } => {
                assert_eq!(topics.len(), 2);
                assert!(topics.contains(&EventTopic::Nodes));
                assert!(topics.contains(&EventTopic::Workloads));
            }
            _ => panic!("Expected Subscribe message"),
        }
    }
}
