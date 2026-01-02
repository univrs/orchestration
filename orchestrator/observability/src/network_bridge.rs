//! Network event bridge for P2P gossipsub integration.
//!
//! This module provides the `NetworkEventBridge` which connects the local
//! `EventHub` (WebSocket streaming) to the P2P network layer (gossipsub).
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────┐     ┌──────────────────┐     ┌─────────────────┐
//! │  WebSocket UI   │────▶│ NetworkEventBridge│────▶│  P2P Network   │
//! │   (EventHub)    │◀────│                  │◀────│  (Gossipsub)   │
//! └─────────────────┘     └──────────────────┘     └─────────────────┘
//! ```
//!
//! - **Inbound**: P2P messages → NetworkMessage → StreamEvent → WebSocket clients
//! - **Outbound**: WebSocket actions → NetworkMessage → P2P publish

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info, warn};

use crate::events::{EventHub, EventTopic, EventType, StreamEvent};
use crate::network_messages::{
    CreditMessage, ElectionMessage, GradientMessage, NetworkMessage, SeptalMessage,
};

// ============================================================================
// PUBSUB NETWORK TRAIT (abstraction for P2P implementations)
// ============================================================================

/// Trait for P2P pubsub network implementations.
///
/// This abstraction allows swapping between different pubsub backends
/// (gossipsub, floodsub, custom implementations, or mock for testing).
#[async_trait]
pub trait PubSubNetwork: Send + Sync {
    /// Subscribe to a gossipsub topic.
    async fn subscribe(&self, topic: &str) -> Result<(), NetworkError>;

    /// Unsubscribe from a gossipsub topic.
    async fn unsubscribe(&self, topic: &str) -> Result<(), NetworkError>;

    /// Publish a message to a topic.
    async fn publish(&self, topic: &str, message: Vec<u8>) -> Result<(), NetworkError>;

    /// Receive the next published message from subscribed topics.
    /// Returns (topic, message_bytes, peer_id).
    async fn next_message(&self) -> Option<(String, Vec<u8>, String)>;

    /// Get list of peers subscribed to a topic.
    async fn peers_on_topic(&self, topic: &str) -> Result<Vec<String>, NetworkError>;

    /// Get local peer ID.
    async fn local_peer_id(&self) -> String;
}

/// Network-related errors.
#[derive(Debug, Clone)]
pub enum NetworkError {
    /// Failed to subscribe to topic.
    SubscriptionFailed(String),
    /// Failed to publish message.
    PublishFailed(String),
    /// Topic not found.
    TopicNotFound(String),
    /// Network not initialized.
    NotInitialized,
    /// Generic error.
    Other(String),
}

impl std::fmt::Display for NetworkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NetworkError::SubscriptionFailed(msg) => write!(f, "Subscription failed: {}", msg),
            NetworkError::PublishFailed(msg) => write!(f, "Publish failed: {}", msg),
            NetworkError::TopicNotFound(topic) => write!(f, "Topic not found: {}", topic),
            NetworkError::NotInitialized => write!(f, "Network not initialized"),
            NetworkError::Other(msg) => write!(f, "Network error: {}", msg),
        }
    }
}

impl std::error::Error for NetworkError {}

// ============================================================================
// MOCK PUBSUB (for testing and development)
// ============================================================================

/// Mock PubSub implementation for testing without actual P2P network.
pub struct MockPubSubNetwork {
    /// Local peer ID.
    peer_id: String,
    /// Subscribed topics.
    subscriptions: Arc<RwLock<HashSet<String>>>,
    /// Message queue (simulates received messages) - reserved for future use.
    #[allow(dead_code)]
    message_queue: Arc<RwLock<Vec<(String, Vec<u8>, String)>>>,
    /// Channel for injecting messages.
    message_tx: mpsc::UnboundedSender<(String, Vec<u8>, String)>,
    /// Channel for receiving messages.
    message_rx: Arc<RwLock<mpsc::UnboundedReceiver<(String, Vec<u8>, String)>>>,
}

impl MockPubSubNetwork {
    /// Create a new mock network.
    pub fn new(peer_id: impl Into<String>) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        Self {
            peer_id: peer_id.into(),
            subscriptions: Arc::new(RwLock::new(HashSet::new())),
            message_queue: Arc::new(RwLock::new(Vec::new())),
            message_tx: tx,
            message_rx: Arc::new(RwLock::new(rx)),
        }
    }

    /// Inject a message (simulates receiving from network).
    pub async fn inject_message(&self, topic: String, payload: Vec<u8>, from_peer: String) {
        let _ = self.message_tx.send((topic, payload, from_peer));
    }
}

#[async_trait]
impl PubSubNetwork for MockPubSubNetwork {
    async fn subscribe(&self, topic: &str) -> Result<(), NetworkError> {
        self.subscriptions.write().await.insert(topic.to_string());
        info!("MockPubSub: Subscribed to {}", topic);
        Ok(())
    }

    async fn unsubscribe(&self, topic: &str) -> Result<(), NetworkError> {
        self.subscriptions.write().await.remove(topic);
        info!("MockPubSub: Unsubscribed from {}", topic);
        Ok(())
    }

    async fn publish(&self, topic: &str, message: Vec<u8>) -> Result<(), NetworkError> {
        if !self.subscriptions.read().await.contains(topic) {
            return Err(NetworkError::TopicNotFound(topic.to_string()));
        }
        info!(
            "MockPubSub: Published {} bytes to {}",
            message.len(),
            topic
        );
        // In mock mode, we could echo messages back to simulate network
        Ok(())
    }

    async fn next_message(&self) -> Option<(String, Vec<u8>, String)> {
        let mut rx = self.message_rx.write().await;
        rx.recv().await
    }

    async fn peers_on_topic(&self, topic: &str) -> Result<Vec<String>, NetworkError> {
        if self.subscriptions.read().await.contains(topic) {
            // Return a mock peer list
            Ok(vec!["mock-peer-1".to_string(), "mock-peer-2".to_string()])
        } else {
            Err(NetworkError::TopicNotFound(topic.to_string()))
        }
    }

    async fn local_peer_id(&self) -> String {
        self.peer_id.clone()
    }
}

// ============================================================================
// NETWORK EVENT BRIDGE
// ============================================================================

/// Configuration for the network bridge.
#[derive(Debug, Clone)]
pub struct NetworkBridgeConfig {
    /// Maximum message cache entries for deduplication.
    pub max_cache_entries: usize,
    /// Cache entry TTL in seconds.
    pub cache_ttl_secs: u64,
    /// Enable message deduplication.
    pub enable_deduplication: bool,
    /// Maximum message size in bytes.
    pub max_message_size: usize,
    /// Topics this node participates in.
    pub enabled_topics: HashSet<EventTopic>,
}

impl Default for NetworkBridgeConfig {
    fn default() -> Self {
        Self {
            max_cache_entries: 10_000,
            cache_ttl_secs: 300,
            enable_deduplication: true,
            max_message_size: 1024 * 1024, // 1MB
            enabled_topics: [
                EventTopic::Gradient,
                EventTopic::Election,
                EventTopic::Credit,
                EventTopic::Septal,
            ]
            .into_iter()
            .collect(),
        }
    }
}

/// Statistics for the network bridge.
#[derive(Debug, Default, Clone)]
pub struct NetworkBridgeStats {
    /// Messages received from network.
    pub messages_received: u64,
    /// Messages published to network.
    pub messages_published: u64,
    /// Messages dropped (deduped, oversized, etc.).
    pub messages_dropped: u64,
    /// Current connected peers.
    pub peer_count: usize,
    /// Messages by topic.
    pub messages_by_topic: HashMap<String, u64>,
}

/// Inbound message from P2P network.
struct InboundNetworkMessage {
    topic: EventTopic,
    message: NetworkMessage,
    /// Peer ID that sent the message - reserved for future filtering.
    #[allow(dead_code)]
    received_from: String,
}

/// Outbound action to P2P network.
struct OutboundNetworkAction {
    topic: EventTopic,
    message: NetworkMessage,
}

/// Network event bridge connecting EventHub to P2P network.
pub struct NetworkEventBridge {
    /// Local event hub for UI clients.
    event_hub: EventHub,

    /// P2P network implementation.
    pubsub: Arc<dyn PubSubNetwork>,

    /// Channel for receiving network messages.
    inbound_tx: mpsc::UnboundedSender<InboundNetworkMessage>,

    /// Channel for sending messages to network.
    outbound_tx: mpsc::UnboundedSender<OutboundNetworkAction>,

    /// Track subscribed P2P topics per client.
    client_subscriptions: Arc<RwLock<HashMap<String, HashSet<EventTopic>>>>,

    /// Deduplication cache (message_id -> timestamp).
    message_cache: Arc<RwLock<HashMap<String, Instant>>>,

    /// Statistics.
    stats: Arc<RwLock<NetworkBridgeStats>>,

    /// Configuration.
    config: NetworkBridgeConfig,

    /// Running flag.
    running: Arc<RwLock<bool>>,
}

impl NetworkEventBridge {
    /// Create a new network event bridge.
    pub fn new(
        event_hub: EventHub,
        pubsub: Arc<dyn PubSubNetwork>,
        config: NetworkBridgeConfig,
    ) -> Self {
        let (inbound_tx, inbound_rx) = mpsc::unbounded_channel();
        let (outbound_tx, outbound_rx) = mpsc::unbounded_channel();

        let bridge = Self {
            event_hub,
            pubsub,
            inbound_tx,
            outbound_tx,
            client_subscriptions: Arc::new(RwLock::new(HashMap::new())),
            message_cache: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(NetworkBridgeStats::default())),
            config: config.clone(),
            running: Arc::new(RwLock::new(false)),
        };

        // Spawn background tasks
        bridge.spawn_inbound_handler(inbound_rx);
        bridge.spawn_outbound_handler(outbound_rx);

        bridge
    }

    /// Initialize the bridge: subscribe to P2P topics and start event loops.
    pub async fn start(&self) -> Result<(), NetworkError> {
        info!("NetworkEventBridge starting");

        // Mark as running
        *self.running.write().await = true;

        // Subscribe to all enabled P2P topics
        for topic in &self.config.enabled_topics {
            if let Some(gossipsub_topic) = topic.to_gossipsub_topic() {
                self.pubsub.subscribe(&gossipsub_topic).await?;
                info!("Subscribed to P2P topic: {}", gossipsub_topic);
            }
        }

        // Spawn message receiver loop
        self.spawn_message_receiver();

        // Spawn cache cleanup loop
        self.spawn_cache_cleanup();

        info!("NetworkEventBridge started successfully");
        Ok(())
    }

    /// Stop the bridge.
    pub async fn stop(&self) {
        info!("NetworkEventBridge stopping");
        *self.running.write().await = false;
    }

    // ========================================================================
    // INBOUND: P2P Network -> Local EventHub
    // ========================================================================

    fn spawn_message_receiver(&self) {
        let pubsub = self.pubsub.clone();
        let inbound_tx = self.inbound_tx.clone();
        let config = self.config.clone();
        let stats = self.stats.clone();
        let running = self.running.clone();

        tokio::spawn(async move {
            loop {
                // Check if still running
                if !*running.read().await {
                    break;
                }

                if let Some((topic_str, message_bytes, peer_id)) = pubsub.next_message().await {
                    if let Some(topic) = EventTopic::from_gossipsub_topic(&topic_str) {
                        // Check message size
                        if message_bytes.len() > config.max_message_size {
                            warn!(
                                "Received oversized message: {} bytes from peer {}",
                                message_bytes.len(),
                                peer_id
                            );
                            {
                                let mut stats = stats.write().await;
                                stats.messages_dropped += 1;
                            }
                            continue;
                        }

                        let msg = NetworkMessage::new(
                            peer_id.clone(),
                            topic_str.clone(),
                            message_bytes,
                        );

                        let inbound = InboundNetworkMessage {
                            topic,
                            message: msg,
                            received_from: peer_id,
                        };

                        if let Err(e) = inbound_tx.send(inbound) {
                            error!("Failed to send inbound message: {}", e);
                        }
                    }
                }
            }
        });
    }

    fn spawn_inbound_handler(&self, mut inbound_rx: mpsc::UnboundedReceiver<InboundNetworkMessage>) {
        let event_hub = self.event_hub.clone();
        let message_cache = self.message_cache.clone();
        let stats = self.stats.clone();
        let config = self.config.clone();

        tokio::spawn(async move {
            while let Some(inbound) = inbound_rx.recv().await {
                // Deduplication check
                if config.enable_deduplication {
                    let mut cache = message_cache.write().await;
                    if cache.contains_key(&inbound.message.id) {
                        debug!("Dropping duplicate message: {}", inbound.message.id);
                        {
                            let mut s = stats.write().await;
                            s.messages_dropped += 1;
                        }
                        continue;
                    }
                    // Add to cache
                    if cache.len() >= config.max_cache_entries {
                        // Evict oldest entry
                        if let Some(oldest_id) = cache
                            .iter()
                            .min_by_key(|(_, instant)| *instant)
                            .map(|(id, _)| id.clone())
                        {
                            cache.remove(&oldest_id);
                        }
                    }
                    cache.insert(inbound.message.id.clone(), Instant::now());
                }

                // Update stats
                {
                    let mut s = stats.write().await;
                    s.messages_received += 1;
                    if let Some(topic) = inbound.topic.to_gossipsub_topic() {
                        *s.messages_by_topic.entry(topic).or_insert(0) += 1;
                    }
                }

                // Parse and broadcast based on topic
                match inbound.topic {
                    EventTopic::Gradient => {
                        if let Ok(gradient) = GradientMessage::from_bytes(&inbound.message.payload)
                        {
                            let event = StreamEvent::new(
                                EventType::ResourceGradientUpdate,
                                serde_json::json!({
                                    "source": inbound.message.source,
                                    "gradient": gradient,
                                }),
                            )
                            .with_correlation(inbound.message.id.clone());
                            event_hub.broadcast(event).await;
                        }
                    }
                    EventTopic::Election => {
                        if let Ok(election) = ElectionMessage::from_bytes(&inbound.message.payload)
                        {
                            let event = StreamEvent::new(
                                EventType::LeaderElectionMessage,
                                serde_json::json!({
                                    "source": inbound.message.source,
                                    "election": election,
                                }),
                            )
                            .with_correlation(inbound.message.id.clone());
                            event_hub.broadcast(event).await;
                        }
                    }
                    EventTopic::Credit => {
                        if let Ok(credit) = CreditMessage::from_bytes(&inbound.message.payload) {
                            let event = StreamEvent::new(
                                EventType::CreditTransaction,
                                serde_json::json!({
                                    "source": inbound.message.source,
                                    "transaction": credit,
                                }),
                            )
                            .with_correlation(inbound.message.id.clone());
                            event_hub.broadcast(event).await;
                        }
                    }
                    EventTopic::Septal => {
                        if let Ok(septal) = SeptalMessage::from_bytes(&inbound.message.payload) {
                            let event = StreamEvent::new(
                                EventType::SeptalCoordination,
                                serde_json::json!({
                                    "source": inbound.message.source,
                                    "coordination": septal,
                                }),
                            )
                            .with_correlation(inbound.message.id.clone());
                            event_hub.broadcast(event).await;
                        }
                    }
                    _ => debug!("Ignoring non-P2P topic: {:?}", inbound.topic),
                }
            }
        });
    }

    // ========================================================================
    // OUTBOUND: Local EventHub -> P2P Network
    // ========================================================================

    /// Publish a network message to P2P network.
    pub async fn publish(&self, topic: EventTopic, payload: Vec<u8>) -> Result<(), NetworkError> {
        if !self.config.enabled_topics.contains(&topic) {
            return Err(NetworkError::Other(format!(
                "Topic {:?} not enabled",
                topic
            )));
        }

        let peer_id = self.pubsub.local_peer_id().await;
        let gossipsub_topic = topic
            .to_gossipsub_topic()
            .ok_or_else(|| NetworkError::TopicNotFound(format!("{:?}", topic)))?;

        let message = NetworkMessage::new(peer_id, gossipsub_topic, payload);

        self.outbound_tx
            .send(OutboundNetworkAction { topic, message })
            .map_err(|e| NetworkError::Other(format!("Failed to queue outbound message: {}", e)))
    }

    /// Publish a gradient message.
    pub async fn publish_gradient(&self, gradient: GradientMessage) -> Result<(), NetworkError> {
        let payload = gradient
            .to_bytes()
            .map_err(|e| NetworkError::Other(format!("Serialization error: {}", e)))?;
        self.publish(EventTopic::Gradient, payload).await
    }

    /// Publish an election message.
    pub async fn publish_election(&self, election: ElectionMessage) -> Result<(), NetworkError> {
        let payload = election
            .to_bytes()
            .map_err(|e| NetworkError::Other(format!("Serialization error: {}", e)))?;
        self.publish(EventTopic::Election, payload).await
    }

    /// Publish a credit message.
    pub async fn publish_credit(&self, credit: CreditMessage) -> Result<(), NetworkError> {
        let payload = credit
            .to_bytes()
            .map_err(|e| NetworkError::Other(format!("Serialization error: {}", e)))?;
        self.publish(EventTopic::Credit, payload).await
    }

    /// Publish a septal message.
    pub async fn publish_septal(&self, septal: SeptalMessage) -> Result<(), NetworkError> {
        let payload = septal
            .to_bytes()
            .map_err(|e| NetworkError::Other(format!("Serialization error: {}", e)))?;
        self.publish(EventTopic::Septal, payload).await
    }

    fn spawn_outbound_handler(
        &self,
        mut outbound_rx: mpsc::UnboundedReceiver<OutboundNetworkAction>,
    ) {
        let pubsub = self.pubsub.clone();
        let stats = self.stats.clone();

        tokio::spawn(async move {
            while let Some(action) = outbound_rx.recv().await {
                if let Some(gossipsub_topic) = action.topic.to_gossipsub_topic() {
                    match pubsub
                        .publish(&gossipsub_topic, action.message.payload.clone())
                        .await
                    {
                        Ok(_) => {
                            {
                                let mut s = stats.write().await;
                                s.messages_published += 1;
                                *s.messages_by_topic
                                    .entry(gossipsub_topic.clone())
                                    .or_insert(0) += 1;
                            }
                            debug!(
                                "Published network message to {} (id: {})",
                                gossipsub_topic, action.message.id
                            );
                        }
                        Err(e) => {
                            error!("Failed to publish message to {}: {}", gossipsub_topic, e);
                            {
                                let mut s = stats.write().await;
                                s.messages_dropped += 1;
                            }
                        }
                    }
                }
            }
        });
    }

    // ========================================================================
    // CACHE MANAGEMENT
    // ========================================================================

    fn spawn_cache_cleanup(&self) {
        let message_cache = self.message_cache.clone();
        let cache_ttl_secs = self.config.cache_ttl_secs;
        let running = self.running.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60));

            loop {
                interval.tick().await;

                if !*running.read().await {
                    break;
                }

                let ttl = Duration::from_secs(cache_ttl_secs);
                let cutoff = Instant::now() - ttl;

                let mut cache = message_cache.write().await;
                let before_len = cache.len();
                cache.retain(|_, instant| *instant > cutoff);
                let after_len = cache.len();

                if before_len != after_len {
                    debug!(
                        "Cache cleanup: removed {} expired entries, {} remaining",
                        before_len - after_len,
                        after_len
                    );
                }
            }
        });
    }

    // ========================================================================
    // BRIDGE MANAGEMENT
    // ========================================================================

    /// Get current statistics.
    pub async fn get_stats(&self) -> NetworkBridgeStats {
        self.stats.read().await.clone()
    }

    /// Update subscriptions for a WebSocket client.
    pub async fn update_client_subscriptions(
        &self,
        client_id: &str,
        topics: Vec<EventTopic>,
        subscribe: bool,
    ) {
        let mut subs = self.client_subscriptions.write().await;
        let client_topics = subs
            .entry(client_id.to_string())
            .or_insert_with(HashSet::new);

        for topic in topics {
            if subscribe {
                client_topics.insert(topic);
            } else {
                client_topics.remove(&topic);
            }
        }
    }

    /// Get subscriptions for a client.
    pub async fn get_client_subscriptions(&self, client_id: &str) -> Option<HashSet<EventTopic>> {
        let subs = self.client_subscriptions.read().await;
        subs.get(client_id).cloned()
    }

    /// Remove client subscriptions (on disconnect).
    pub async fn remove_client(&self, client_id: &str) {
        let mut subs = self.client_subscriptions.write().await;
        subs.remove(client_id);
    }

    /// Get list of connected peers on a topic.
    pub async fn get_peers(&self, topic: EventTopic) -> Result<Vec<String>, NetworkError> {
        if let Some(gossipsub_topic) = topic.to_gossipsub_topic() {
            self.pubsub.peers_on_topic(&gossipsub_topic).await
        } else {
            Err(NetworkError::TopicNotFound(format!("{:?}", topic)))
        }
    }

    /// Get local peer ID.
    pub async fn local_peer_id(&self) -> String {
        self.pubsub.local_peer_id().await
    }

    /// Get the event hub.
    pub fn event_hub(&self) -> &EventHub {
        &self.event_hub
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_pubsub() {
        let mock = MockPubSubNetwork::new("test-peer-1");

        // Subscribe
        mock.subscribe("/orchestrator/gradient/1.0.0")
            .await
            .unwrap();

        // Publish
        mock.publish("/orchestrator/gradient/1.0.0", vec![1, 2, 3])
            .await
            .unwrap();

        // Get peers
        let peers = mock
            .peers_on_topic("/orchestrator/gradient/1.0.0")
            .await
            .unwrap();
        assert!(!peers.is_empty());

        // Peer ID
        assert_eq!(mock.local_peer_id().await, "test-peer-1");
    }

    #[tokio::test]
    async fn test_network_bridge_config() {
        let config = NetworkBridgeConfig::default();
        assert!(config.enabled_topics.contains(&EventTopic::Gradient));
        assert!(config.enabled_topics.contains(&EventTopic::Election));
        assert!(config.enabled_topics.contains(&EventTopic::Credit));
        assert!(config.enabled_topics.contains(&EventTopic::Septal));
        assert_eq!(config.max_cache_entries, 10_000);
    }

    #[tokio::test]
    async fn test_network_bridge_creation() {
        let event_hub = EventHub::default();
        let mock_pubsub = Arc::new(MockPubSubNetwork::new("test-node"));
        let config = NetworkBridgeConfig::default();

        let bridge = NetworkEventBridge::new(event_hub, mock_pubsub.clone(), config);

        // Start the bridge
        bridge.start().await.unwrap();

        // Check peer ID
        assert_eq!(bridge.local_peer_id().await, "test-node");

        // Check stats
        let stats = bridge.get_stats().await;
        assert_eq!(stats.messages_received, 0);
        assert_eq!(stats.messages_published, 0);

        // Stop
        bridge.stop().await;
    }

    #[tokio::test]
    async fn test_client_subscriptions() {
        let event_hub = EventHub::default();
        let mock_pubsub = Arc::new(MockPubSubNetwork::new("test-node"));
        let config = NetworkBridgeConfig::default();

        let bridge = NetworkEventBridge::new(event_hub, mock_pubsub, config);

        // Add subscriptions
        bridge
            .update_client_subscriptions(
                "client-1",
                vec![EventTopic::Gradient, EventTopic::Election],
                true,
            )
            .await;

        let subs = bridge.get_client_subscriptions("client-1").await.unwrap();
        assert!(subs.contains(&EventTopic::Gradient));
        assert!(subs.contains(&EventTopic::Election));

        // Remove subscription
        bridge
            .update_client_subscriptions("client-1", vec![EventTopic::Gradient], false)
            .await;

        let subs = bridge.get_client_subscriptions("client-1").await.unwrap();
        assert!(!subs.contains(&EventTopic::Gradient));
        assert!(subs.contains(&EventTopic::Election));

        // Remove client
        bridge.remove_client("client-1").await;
        assert!(bridge.get_client_subscriptions("client-1").await.is_none());
    }
}
