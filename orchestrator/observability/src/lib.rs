//! Observability stack for AI-Native Container Orchestration.
//!
//! This crate provides comprehensive observability capabilities:
//!
//! - **Tracing**: Structured logging with spans for distributed tracing
//! - **Metrics**: Prometheus-compatible metrics for monitoring
//! - **Health Endpoints**: HTTP endpoints for health checks and readiness probes
//! - **Event Streaming**: WebSocket endpoint for real-time cluster events
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────┐
//! │                  Observability Layer                     │
//! ├─────────────┬─────────────┬─────────────┬───────────────┤
//! │   Tracing   │   Metrics   │   Health    │    Events     │
//! │  (tracing)  │ (prometheus)│   Server    │  (WebSocket)  │
//! ├─────────────┴─────────────┴─────────────┴───────────────┤
//! │                 Orchestrator Components                  │
//! └─────────────────────────────────────────────────────────┘
//! ```
//!
//! # WebSocket Event Streaming
//!
//! Connect to `ws://host:9090/api/v1/events` and subscribe to topics:
//!
//! ```json
//! {"type": "subscribe", "topics": ["nodes", "workloads", "cluster"]}
//! ```

pub mod tracing_setup;
pub mod metrics;
pub mod health;
pub mod server;
pub mod events;
pub mod websocket;

pub use tracing_setup::{init_tracing, TracingConfig};
pub use metrics::{OrchestratorMetrics, MetricsRegistry};
pub use health::{HealthChecker, HealthStatus, ComponentHealth};
pub use server::{ObservabilityServer, ObservabilityConfig};
pub use events::{EventHub, EventTopic, StreamEvent, EventType};
pub use websocket::events_handler;

/// Re-export tracing macros for convenience
pub use tracing::{debug, error, info, instrument, trace, warn, span, Level};
