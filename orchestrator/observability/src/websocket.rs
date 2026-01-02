//! WebSocket handler for the `/api/v1/events` endpoint.
//!
//! Handles WebSocket connections, client subscriptions, event streaming,
//! and P2P network actions via the NetworkEventBridge.

use std::sync::Arc;
use std::time::Duration;

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::events::{ClientMessage, EventHub, EventType, StreamEvent};
use crate::network_bridge::NetworkEventBridge;
use crate::network_messages::{
    BarrierType, CreditMessage, ElectionMessage, ElectionPhase, GradientMessage, SeptalMessage,
};

/// WebSocket handler state (event hub + optional network bridge).
#[derive(Clone)]
pub struct WebSocketState {
    /// Local event hub for subscriptions and broadcasts.
    pub event_hub: EventHub,
    /// Optional network bridge for P2P messaging.
    pub network_bridge: Option<Arc<NetworkEventBridge>>,
}

impl WebSocketState {
    /// Create state with just event hub (no P2P).
    pub fn new(event_hub: EventHub) -> Self {
        Self {
            event_hub,
            network_bridge: None,
        }
    }

    /// Create state with event hub and network bridge.
    pub fn with_network_bridge(event_hub: EventHub, network_bridge: Arc<NetworkEventBridge>) -> Self {
        Self {
            event_hub,
            network_bridge: Some(network_bridge),
        }
    }
}

/// WebSocket handler for event streaming (without P2P).
pub async fn events_handler(
    ws: WebSocketUpgrade,
    State(event_hub): State<EventHub>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, event_hub, None))
}

/// WebSocket handler for event streaming with P2P network bridge.
pub async fn events_handler_with_bridge(
    ws: WebSocketUpgrade,
    State(state): State<WebSocketState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| {
        handle_socket(socket, state.event_hub, state.network_bridge)
    })
}

/// Handle an individual WebSocket connection.
async fn handle_socket(
    socket: WebSocket,
    event_hub: EventHub,
    network_bridge: Option<Arc<NetworkEventBridge>>,
) {
    let client_id = Uuid::new_v4().to_string();
    let has_network = network_bridge.is_some();
    tracing::info!(
        client_id = %client_id,
        has_network = %has_network,
        "WebSocket client connected"
    );

    // Register client
    event_hub.register_client(client_id.clone()).await;

    // Subscribe to event broadcast
    let mut event_rx = event_hub.subscribe();

    // Split socket into sender and receiver
    let (mut ws_sender, mut ws_receiver) = socket.split();

    // Send connected event with network capability info
    let connected_event = StreamEvent::new(
        EventType::Connected,
        serde_json::json!({
            "client_id": client_id,
            "message": "Connected to event stream",
            "network_enabled": has_network,
            "p2p_topics": if has_network {
                vec!["gradient", "election", "credit", "septal"]
            } else {
                vec![]
            }
        }),
    );
    if let Ok(json) = serde_json::to_string(&connected_event) {
        let _ = ws_sender.send(Message::Text(json.into())).await;
    }

    // Heartbeat interval
    let heartbeat_interval = Duration::from_secs(30);
    let mut heartbeat_timer = tokio::time::interval(heartbeat_interval);
    heartbeat_timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    // Main event loop
    loop {
        tokio::select! {
            // Handle incoming messages from client
            msg = ws_receiver.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        if let Err(e) = handle_client_message(
                            &text,
                            &client_id,
                            &event_hub,
                            network_bridge.as_ref(),
                            &mut ws_sender,
                        ).await {
                            tracing::warn!(
                                client_id = %client_id,
                                error = %e,
                                "Error handling client message"
                            );
                        }
                    }
                    Some(Ok(Message::Close(_))) => {
                        tracing::info!(client_id = %client_id, "Client initiated close");
                        break;
                    }
                    Some(Ok(Message::Ping(data))) => {
                        let _ = ws_sender.send(Message::Pong(data)).await;
                    }
                    Some(Ok(Message::Pong(_))) => {
                        // Client responded to our ping
                    }
                    Some(Err(e)) => {
                        tracing::warn!(client_id = %client_id, error = %e, "WebSocket error");
                        break;
                    }
                    None => {
                        tracing::info!(client_id = %client_id, "WebSocket stream ended");
                        break;
                    }
                    _ => {}
                }
            }

            // Handle broadcast events
            event = event_rx.recv() => {
                match event {
                    Ok(stream_event) => {
                        // Check if client is subscribed to this event type
                        let subscribed_topics = event_hub.get_subscription(&client_id).await;
                        let should_send = subscribed_topics
                            .map(|topics| topics.iter().any(|t| t.matches(&stream_event)))
                            .unwrap_or(false);

                        if should_send {
                            if let Ok(json) = serde_json::to_string(&stream_event) {
                                if ws_sender.send(Message::Text(json.into())).await.is_err() {
                                    tracing::info!(
                                        client_id = %client_id,
                                        "Failed to send event, client disconnected"
                                    );
                                    break;
                                }
                            }
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!(
                            client_id = %client_id,
                            missed_events = n,
                            "Client lagging behind event stream"
                        );
                        // Send a warning to the client
                        let warning = StreamEvent::error(format!(
                            "Missed {} events due to slow consumption",
                            n
                        ));
                        if let Ok(json) = serde_json::to_string(&warning) {
                            let _ = ws_sender.send(Message::Text(json.into())).await;
                        }
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        tracing::info!(client_id = %client_id, "Event hub closed");
                        break;
                    }
                }
            }

            // Send heartbeat
            _ = heartbeat_timer.tick() => {
                let heartbeat = StreamEvent::heartbeat();
                if let Ok(json) = serde_json::to_string(&heartbeat) {
                    if ws_sender.send(Message::Text(json.into())).await.is_err() {
                        tracing::info!(
                            client_id = %client_id,
                            "Failed to send heartbeat, client disconnected"
                        );
                        break;
                    }
                }
            }
        }
    }

    // Cleanup
    event_hub.unregister_client(&client_id).await;
    if let Some(ref bridge) = network_bridge {
        bridge.remove_client(&client_id).await;
    }
    tracing::info!(client_id = %client_id, "WebSocket client disconnected");
}

/// Handle a message from the client.
async fn handle_client_message(
    text: &str,
    client_id: &str,
    event_hub: &EventHub,
    network_bridge: Option<&Arc<NetworkEventBridge>>,
    ws_sender: &mut futures_util::stream::SplitSink<WebSocket, Message>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let msg: ClientMessage = serde_json::from_str(text)?;

    match msg {
        ClientMessage::Subscribe { topics } => {
            tracing::debug!(
                client_id = %client_id,
                topics = ?topics,
                "Client subscribing to topics"
            );

            event_hub.update_subscription(client_id, &topics, true).await;

            // Also update network bridge subscriptions for P2P topics
            if let Some(bridge) = network_bridge {
                let p2p_topics: Vec<_> = topics.iter().filter(|t| t.is_p2p_topic()).copied().collect();
                if !p2p_topics.is_empty() {
                    bridge.update_client_subscriptions(client_id, p2p_topics, true).await;
                }
            }

            // Send confirmation
            let response = StreamEvent::subscribed(&topics);
            let json = serde_json::to_string(&response)?;
            ws_sender.send(Message::Text(json.into())).await?;
        }
        ClientMessage::Unsubscribe { topics } => {
            tracing::debug!(
                client_id = %client_id,
                topics = ?topics,
                "Client unsubscribing from topics"
            );

            event_hub.update_subscription(client_id, &topics, false).await;

            // Also update network bridge subscriptions
            if let Some(bridge) = network_bridge {
                let p2p_topics: Vec<_> = topics.iter().filter(|t| t.is_p2p_topic()).copied().collect();
                if !p2p_topics.is_empty() {
                    bridge.update_client_subscriptions(client_id, p2p_topics, false).await;
                }
            }

            // Send confirmation
            let response = StreamEvent::new(
                EventType::Unsubscribed,
                serde_json::json!({
                    "topics": topics,
                    "message": "Successfully unsubscribed from topics"
                }),
            );
            let json = serde_json::to_string(&response)?;
            ws_sender.send(Message::Text(json.into())).await?;
        }
        ClientMessage::Ping => {
            // Respond with pong
            let pong = StreamEvent::new(
                EventType::Heartbeat,
                serde_json::json!({ "pong": true }),
            );
            let json = serde_json::to_string(&pong)?;
            ws_sender.send(Message::Text(json.into())).await?;
        }

        // P2P Network Actions
        ClientMessage::PublishGradient {
            cpu_available,
            memory_available,
            disk_available,
        } => {
            if let Some(bridge) = network_bridge {
                let peer_id = bridge.local_peer_id().await;
                let gradient = GradientMessage::new(
                    peer_id,
                    cpu_available,
                    memory_available,
                    disk_available,
                );

                match bridge.publish_gradient(gradient).await {
                    Ok(_) => {
                        let response = StreamEvent::new(
                            EventType::NetworkMessagePublished,
                            serde_json::json!({
                                "topic": "gradient",
                                "message": "Gradient published to network"
                            }),
                        );
                        let json = serde_json::to_string(&response)?;
                        ws_sender.send(Message::Text(json.into())).await?;
                    }
                    Err(e) => {
                        let error = StreamEvent::error(format!("Failed to publish gradient: {}", e));
                        let json = serde_json::to_string(&error)?;
                        ws_sender.send(Message::Text(json.into())).await?;
                    }
                }
            } else {
                let error = StreamEvent::error("Network bridge not available");
                let json = serde_json::to_string(&error)?;
                ws_sender.send(Message::Text(json.into())).await?;
            }
        }

        ClientMessage::PublishElection {
            round,
            ballot,
            phase,
            proposer,
        } => {
            if let Some(bridge) = network_bridge {
                let peer_id = bridge.local_peer_id().await;
                let proposer_id = proposer.unwrap_or(peer_id);

                let election_phase = match phase.as_str() {
                    "prepare" => ElectionPhase::Prepare,
                    "promise" => ElectionPhase::Promise,
                    "accept" => ElectionPhase::Accept,
                    "accepted" => ElectionPhase::Accepted,
                    "elected" => ElectionPhase::Elected,
                    _ => ElectionPhase::Prepare,
                };

                let election = match election_phase {
                    ElectionPhase::Prepare => ElectionMessage::prepare(proposer_id, round, ballot, 3),
                    ElectionPhase::Elected => ElectionMessage::elected(proposer_id, round, ballot),
                    _ => ElectionMessage::prepare(proposer_id, round, ballot, 3),
                };

                match bridge.publish_election(election).await {
                    Ok(_) => {
                        let response = StreamEvent::new(
                            EventType::NetworkMessagePublished,
                            serde_json::json!({
                                "topic": "election",
                                "phase": phase,
                                "message": "Election message published to network"
                            }),
                        );
                        let json = serde_json::to_string(&response)?;
                        ws_sender.send(Message::Text(json.into())).await?;
                    }
                    Err(e) => {
                        let error = StreamEvent::error(format!("Failed to publish election: {}", e));
                        let json = serde_json::to_string(&error)?;
                        ws_sender.send(Message::Text(json.into())).await?;
                    }
                }
            } else {
                let error = StreamEvent::error("Network bridge not available");
                let json = serde_json::to_string(&error)?;
                ws_sender.send(Message::Text(json.into())).await?;
            }
        }

        ClientMessage::PublishCredit { to, amount, resource } => {
            if let Some(bridge) = network_bridge {
                let peer_id = bridge.local_peer_id().await;

                let credit = if let Some(res) = resource {
                    CreditMessage::resource_payment(peer_id, to, amount, res)
                } else {
                    CreditMessage::transfer(peer_id, to, amount)
                };

                match bridge.publish_credit(credit).await {
                    Ok(_) => {
                        let response = StreamEvent::new(
                            EventType::NetworkMessagePublished,
                            serde_json::json!({
                                "topic": "credit",
                                "message": "Credit transaction published to network"
                            }),
                        );
                        let json = serde_json::to_string(&response)?;
                        ws_sender.send(Message::Text(json.into())).await?;
                    }
                    Err(e) => {
                        let error = StreamEvent::error(format!("Failed to publish credit: {}", e));
                        let json = serde_json::to_string(&error)?;
                        ws_sender.send(Message::Text(json.into())).await?;
                    }
                }
            } else {
                let error = StreamEvent::error("Network bridge not available");
                let json = serde_json::to_string(&error)?;
                ws_sender.send(Message::Text(json.into())).await?;
            }
        }

        ClientMessage::PublishSeptal {
            barrier_type,
            barrier_id: _,
            participants,
            timeout_ms,
        } => {
            if let Some(bridge) = network_bridge {
                let peer_id = bridge.local_peer_id().await;

                let barrier = match barrier_type.as_str() {
                    "two_phase_commit" => BarrierType::TwoPhaseCommit,
                    "state_consistency" => BarrierType::StateConsistency,
                    "scheduling_sync" => BarrierType::SchedulingSync,
                    "reservation_sync" => BarrierType::ReservationSync,
                    _ => BarrierType::Checkpoint,
                };

                let septal = SeptalMessage::initiate(barrier, peer_id, participants, timeout_ms);

                match bridge.publish_septal(septal).await {
                    Ok(_) => {
                        let response = StreamEvent::new(
                            EventType::NetworkMessagePublished,
                            serde_json::json!({
                                "topic": "septal",
                                "barrier_type": barrier_type,
                                "message": "Septal coordination published to network"
                            }),
                        );
                        let json = serde_json::to_string(&response)?;
                        ws_sender.send(Message::Text(json.into())).await?;
                    }
                    Err(e) => {
                        let error = StreamEvent::error(format!("Failed to publish septal: {}", e));
                        let json = serde_json::to_string(&error)?;
                        ws_sender.send(Message::Text(json.into())).await?;
                    }
                }
            } else {
                let error = StreamEvent::error("Network bridge not available");
                let json = serde_json::to_string(&error)?;
                ws_sender.send(Message::Text(json.into())).await?;
            }
        }

        ClientMessage::GetPeers { topic } => {
            if let Some(bridge) = network_bridge {
                match bridge.get_peers(topic).await {
                    Ok(peers) => {
                        let response = StreamEvent::new(
                            EventType::NetworkMessageReceived,
                            serde_json::json!({
                                "topic": topic,
                                "peers": peers,
                                "peer_count": peers.len()
                            }),
                        );
                        let json = serde_json::to_string(&response)?;
                        ws_sender.send(Message::Text(json.into())).await?;
                    }
                    Err(e) => {
                        let error = StreamEvent::error(format!("Failed to get peers: {}", e));
                        let json = serde_json::to_string(&error)?;
                        ws_sender.send(Message::Text(json.into())).await?;
                    }
                }
            } else {
                let error = StreamEvent::error("Network bridge not available");
                let json = serde_json::to_string(&error)?;
                ws_sender.send(Message::Text(json.into())).await?;
            }
        }

        ClientMessage::GetNetworkStats => {
            if let Some(bridge) = network_bridge {
                let stats = bridge.get_stats().await;
                let response = StreamEvent::new(
                    EventType::NetworkMessageReceived,
                    serde_json::json!({
                        "messages_received": stats.messages_received,
                        "messages_published": stats.messages_published,
                        "messages_dropped": stats.messages_dropped,
                        "peer_count": stats.peer_count,
                        "messages_by_topic": stats.messages_by_topic
                    }),
                );
                let json = serde_json::to_string(&response)?;
                ws_sender.send(Message::Text(json.into())).await?;
            } else {
                let error = StreamEvent::error("Network bridge not available");
                let json = serde_json::to_string(&error)?;
                ws_sender.send(Message::Text(json.into())).await?;
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::EventTopic;

    #[tokio::test]
    async fn test_event_hub_integration() {
        let hub = EventHub::default();

        // Register a client
        hub.register_client("test-client".to_string()).await;

        // Subscribe to nodes
        hub.update_subscription("test-client", &[EventTopic::Nodes], true).await;

        // Check subscription
        let topics = hub.get_subscription("test-client").await.unwrap();
        assert!(topics.contains(&EventTopic::Nodes));

        // Cleanup
        hub.unregister_client("test-client").await;
        assert_eq!(hub.active_connections().await, 0);
    }
}
