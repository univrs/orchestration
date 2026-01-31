//! Observability stack for AI-Native Container Orchestration.
//!
//! This crate provides comprehensive observability capabilities:
//!
//! - **Tracing**: Structured logging with spans for distributed tracing
//! - **Metrics**: Prometheus-compatible metrics for monitoring
//! - **Health Endpoints**: HTTP endpoints for health checks and readiness probes
//! - **Event Streaming**: WebSocket endpoint for real-time cluster events
//! - **P2P Network Bridge**: Gossipsub integration for distributed events
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                     Observability Layer                          │
//! ├──────────┬──────────┬──────────┬────────────┬───────────────────┤
//! │ Tracing  │ Metrics  │  Health  │   Events   │  Network Bridge   │
//! │(tracing) │(prometheus)│ Server │ (WebSocket)│    (Gossipsub)    │
//! ├──────────┴──────────┴──────────┴────────────┴───────────────────┤
//! │                    Orchestrator Components                       │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # WebSocket Event Streaming
//!
//! Connect to `ws://host:9090/api/v1/events` and subscribe to topics:
//!
//! ```json
//! {"type": "subscribe", "topics": ["nodes", "workloads", "cluster"]}
//! ```
//!
//! # P2P Network Topics
//!
//! The network bridge supports four distributed topics:
//!
//! - **gradient**: Resource availability and pricing signals
//! - **election**: Leader election consensus messages
//! - **credit**: Economic/credit transactions
//! - **septal**: Coordination barriers and synchronization

pub mod events;
pub mod health;
pub mod metrics;
pub mod network_bridge;
pub mod network_messages;
pub mod server;
pub mod tracing_setup;
pub mod websocket;

pub use events::{EventHub, EventTopic, EventType, StreamEvent};
pub use health::{ComponentHealth, HealthChecker, HealthStatus};
pub use metrics::{MetricsRegistry, OrchestratorMetrics};
pub use network_bridge::{
    MockPubSubNetwork, NetworkBridgeConfig, NetworkBridgeStats, NetworkError, NetworkEventBridge,
    PubSubNetwork,
};
pub use network_messages::{
    CreditMessage, ElectionMessage, GradientMessage, NetworkMessage, SeptalMessage,
};
pub use server::{ObservabilityConfig, ObservabilityServer};
pub use tracing_setup::{init_tracing, TracingConfig};
pub use websocket::{events_handler, events_handler_with_bridge, WebSocketState};

/// Re-export tracing macros for convenience
pub use tracing::{debug, error, info, instrument, span, trace, warn, Level};
