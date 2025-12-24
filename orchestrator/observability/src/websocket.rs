//! WebSocket handler for the `/api/v1/events` endpoint.
//!
//! Handles WebSocket connections, client subscriptions, and event streaming.

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

use crate::events::{ClientMessage, EventHub, StreamEvent};

/// WebSocket handler for event streaming.
pub async fn events_handler(
    ws: WebSocketUpgrade,
    State(event_hub): State<EventHub>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, event_hub))
}

/// Handle an individual WebSocket connection.
async fn handle_socket(socket: WebSocket, event_hub: EventHub) {
    let client_id = Uuid::new_v4().to_string();
    tracing::info!(client_id = %client_id, "WebSocket client connected");

    // Register client
    event_hub.register_client(client_id.clone()).await;

    // Subscribe to event broadcast
    let mut event_rx = event_hub.subscribe();

    // Split socket into sender and receiver
    let (mut ws_sender, mut ws_receiver) = socket.split();

    // Send connected event
    let connected_event = StreamEvent::connected(&client_id);
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
    tracing::info!(client_id = %client_id, "WebSocket client disconnected");
}

/// Handle a message from the client.
async fn handle_client_message(
    text: &str,
    client_id: &str,
    event_hub: &EventHub,
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

            // Send confirmation
            let response = StreamEvent::new(
                crate::events::EventType::Unsubscribed,
                serde_json::json!({
                    "topics": topics,
                    "message": "Successfully unsubscribed from topics"
                }),
            );
            let json = serde_json::to_string(&response)?;
            ws_sender.send(Message::Text(json.into())).await?;
        }
        ClientMessage::Ping => {
            // Respond with pong (using WebSocket native pong or text response)
            let pong = StreamEvent::new(
                crate::events::EventType::Heartbeat,
                serde_json::json!({ "pong": true }),
            );
            let json = serde_json::to_string(&pong)?;
            ws_sender.send(Message::Text(json.into())).await?;
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
