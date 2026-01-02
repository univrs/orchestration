//! HTTP server for observability endpoints.
//!
//! Provides health, readiness, metrics, and WebSocket event streaming via axum.
//!
//! # Endpoints
//!
//! - `GET /health` - Health check
//! - `GET /healthz` - Kubernetes-style health check
//! - `GET /ready` - Readiness probe
//! - `GET /readyz` - Kubernetes-style readiness
//! - `GET /live` - Liveness probe
//! - `GET /livez` - Kubernetes-style liveness
//! - `GET /metrics` - Prometheus metrics
//! - `GET /api/v1/events` - WebSocket event streaming

use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

use crate::events::EventHub;
use crate::health::{AggregatedHealth, HealthChecker};
use crate::metrics::MetricsRegistry;
use crate::network_bridge::NetworkEventBridge;
use crate::websocket::{events_handler, events_handler_with_bridge, WebSocketState};

/// Configuration for the observability server.
#[derive(Debug, Clone)]
pub struct ObservabilityConfig {
    /// Address to bind the server to.
    pub bind_addr: SocketAddr,
    /// Enable CORS for all origins.
    pub enable_cors: bool,
    /// Enable request tracing.
    pub enable_tracing: bool,
}

impl Default for ObservabilityConfig {
    fn default() -> Self {
        Self {
            bind_addr: SocketAddr::from(([0, 0, 0, 0], 9090)),
            enable_cors: true,
            enable_tracing: true,
        }
    }
}

impl ObservabilityConfig {
    /// Create a new config with the specified port.
    pub fn with_port(port: u16) -> Self {
        Self {
            bind_addr: SocketAddr::from(([0, 0, 0, 0], port)),
            ..Default::default()
        }
    }

    /// Create a new config with the specified address.
    pub fn with_addr(addr: impl Into<SocketAddr>) -> Self {
        Self {
            bind_addr: addr.into(),
            ..Default::default()
        }
    }
}

/// Shared state for the observability server.
#[derive(Clone)]
pub struct ObservabilityState {
    health_checker: HealthChecker,
    metrics_registry: Arc<RwLock<Option<MetricsRegistry>>>,
    event_hub: EventHub,
    network_bridge: Option<Arc<NetworkEventBridge>>,
}

impl ObservabilityState {
    /// Create new state with a health checker.
    pub fn new(health_checker: HealthChecker) -> Self {
        Self {
            health_checker,
            metrics_registry: Arc::new(RwLock::new(None)),
            event_hub: EventHub::default(),
            network_bridge: None,
        }
    }

    /// Create new state with a health checker and event hub.
    pub fn with_event_hub(health_checker: HealthChecker, event_hub: EventHub) -> Self {
        Self {
            health_checker,
            metrics_registry: Arc::new(RwLock::new(None)),
            event_hub,
            network_bridge: None,
        }
    }

    /// Set the metrics registry.
    pub async fn set_metrics_registry(&self, registry: MetricsRegistry) {
        *self.metrics_registry.write().await = Some(registry);
    }

    /// Set the network bridge for P2P messaging.
    pub fn set_network_bridge(&mut self, bridge: Arc<NetworkEventBridge>) {
        self.network_bridge = Some(bridge);
    }

    /// Get the health checker.
    pub fn health_checker(&self) -> &HealthChecker {
        &self.health_checker
    }

    /// Get the event hub.
    pub fn event_hub(&self) -> &EventHub {
        &self.event_hub
    }

    /// Get the network bridge.
    pub fn network_bridge(&self) -> Option<&Arc<NetworkEventBridge>> {
        self.network_bridge.as_ref()
    }
}

/// Observability HTTP server.
///
/// Supports merging additional routes (e.g., REST API routes) into the server.
///
/// # Example
///
/// ```ignore
/// let server = ObservabilityServer::new(config, health_checker)
///     .with_event_hub(event_hub)
///     .with_metrics(metrics)
///     .with_additional_routes(api_router)
///     .await;
/// server.serve().await?;
/// ```
pub struct ObservabilityServer {
    config: ObservabilityConfig,
    state: ObservabilityState,
    additional_routes: Option<Router>,
}

impl ObservabilityServer {
    /// Create a new observability server.
    pub fn new(config: ObservabilityConfig, health_checker: HealthChecker) -> Self {
        Self {
            config,
            state: ObservabilityState::new(health_checker),
            additional_routes: None,
        }
    }

    /// Create with default config.
    pub fn with_defaults(health_checker: HealthChecker) -> Self {
        Self::new(ObservabilityConfig::default(), health_checker)
    }

    /// Set the metrics registry.
    pub async fn with_metrics(self, registry: MetricsRegistry) -> Self {
        self.state.set_metrics_registry(registry).await;
        self
    }

    /// Set a custom event hub.
    pub fn with_event_hub(mut self, event_hub: EventHub) -> Self {
        self.state = ObservabilityState::with_event_hub(
            self.state.health_checker.clone(),
            event_hub,
        );
        self
    }

    /// Set the network bridge for P2P messaging.
    ///
    /// When a network bridge is set, the WebSocket event endpoint will
    /// use the P2P-enabled handler that can publish and receive network messages.
    pub fn with_network_bridge(mut self, bridge: Arc<NetworkEventBridge>) -> Self {
        self.state.set_network_bridge(bridge);
        self
    }

    /// Add additional routes to be merged with the observability routes.
    ///
    /// This allows external routers (e.g., REST API routes) to be served
    /// on the same server as the observability endpoints.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let api_router = Router::new()
    ///     .route("/api/v1/workloads", get(list_workloads))
    ///     .with_state(api_state);
    ///
    /// let server = ObservabilityServer::new(config, health_checker)
    ///     .with_additional_routes(api_router);
    /// ```
    pub fn with_additional_routes(mut self, router: Router) -> Self {
        self.additional_routes = Some(router);
        self
    }

    /// Get the shared state for external use.
    pub fn state(&self) -> ObservabilityState {
        self.state.clone()
    }

    /// Get the event hub for broadcasting events.
    pub fn event_hub(&self) -> &EventHub {
        self.state.event_hub()
    }

    /// Build the router.
    ///
    /// Note: This method consumes any additional routes that were set.
    /// Calling it multiple times will only include additional routes on the first call.
    pub fn router(&mut self) -> Router {
        // Create the events API router with appropriate state
        // Use P2P-enabled handler when network bridge is available
        let events_router = if let Some(ref bridge) = self.state.network_bridge {
            let ws_state = WebSocketState::with_network_bridge(
                self.state.event_hub.clone(),
                bridge.clone(),
            );
            Router::new()
                .route("/events", get(events_ws_handler_with_bridge))
                .with_state(ws_state)
        } else {
            Router::new()
                .route("/events", get(events_ws_handler))
                .with_state(self.state.event_hub.clone())
        };

        let mut router = Router::new()
            // Health endpoints
            .route("/health", get(health_handler))
            .route("/healthz", get(health_handler))
            .route("/ready", get(ready_handler))
            .route("/readyz", get(ready_handler))
            .route("/live", get(live_handler))
            .route("/livez", get(live_handler))
            .route("/metrics", get(metrics_handler))
            .with_state(self.state.clone())
            // Nest the events API under /api/v1
            .nest("/api/v1", events_router);

        // Merge additional routes if provided (e.g., REST API routes)
        // Note: take() consumes the Option, so additional routes are only included once
        if let Some(additional) = self.additional_routes.take() {
            router = router.merge(additional);
        }

        if self.config.enable_cors {
            router = router.layer(CorsLayer::new().allow_origin(Any).allow_methods(Any));
        }

        if self.config.enable_tracing {
            router = router.layer(TraceLayer::new_for_http());
        }

        router
    }

    /// Start the server (blocking).
    pub async fn serve(mut self) -> Result<(), std::io::Error> {
        let router = self.router();
        let listener = tokio::net::TcpListener::bind(self.config.bind_addr).await?;

        tracing::info!(
            addr = %self.config.bind_addr,
            "Starting observability server"
        );

        axum::serve(listener, router).await
    }

    /// Start the server in the background.
    pub fn spawn(self) -> tokio::task::JoinHandle<Result<(), std::io::Error>> {
        tokio::spawn(async move { self.serve().await })
    }
}

/// Health check endpoint handler.
async fn health_handler(State(state): State<ObservabilityState>) -> impl IntoResponse {
    let health = state.health_checker.get_health().await;
    let status_code = health_to_status_code(&health);
    (status_code, Json(health))
}

/// Readiness check endpoint handler.
async fn ready_handler(State(state): State<ObservabilityState>) -> impl IntoResponse {
    let readiness = state.health_checker.get_readiness().await;
    let status_code = if readiness.ready {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    (status_code, Json(readiness))
}

/// Liveness check endpoint handler.
async fn live_handler(State(state): State<ObservabilityState>) -> impl IntoResponse {
    let liveness = state.health_checker.get_liveness().await;
    // Liveness is always OK if we can respond
    (StatusCode::OK, Json(liveness))
}

/// Metrics endpoint handler (Prometheus format).
async fn metrics_handler(State(state): State<ObservabilityState>) -> Response {
    let registry = state.metrics_registry.read().await;
    match registry.as_ref() {
        Some(reg) => {
            let body = reg.render();
            Response::builder()
                .status(StatusCode::OK)
                .header("content-type", "text/plain; version=0.0.4; charset=utf-8")
                .body(body.into())
                .unwrap()
        }
        None => Response::builder()
            .status(StatusCode::OK)
            .header("content-type", "text/plain; version=0.0.4; charset=utf-8")
            .body("# No metrics registry configured\n".into())
            .unwrap(),
    }
}

/// WebSocket events endpoint handler.
///
/// Accepts WebSocket upgrade requests and delegates to the events handler.
async fn events_ws_handler(
    ws: axum::extract::ws::WebSocketUpgrade,
    State(event_hub): State<EventHub>,
) -> impl IntoResponse {
    events_handler(ws, State(event_hub)).await
}

/// WebSocket events endpoint handler with P2P network bridge.
///
/// Accepts WebSocket upgrade requests and delegates to the P2P-enabled events handler.
async fn events_ws_handler_with_bridge(
    ws: axum::extract::ws::WebSocketUpgrade,
    State(ws_state): State<WebSocketState>,
) -> impl IntoResponse {
    events_handler_with_bridge(ws, State(ws_state)).await
}

/// Convert health status to HTTP status code.
fn health_to_status_code(health: &AggregatedHealth) -> StatusCode {
    match health.status {
        crate::health::HealthStatus::Healthy => StatusCode::OK,
        crate::health::HealthStatus::Degraded => StatusCode::OK, // Still serving
        crate::health::HealthStatus::Unhealthy => StatusCode::SERVICE_UNAVAILABLE,
        crate::health::HealthStatus::Unknown => StatusCode::SERVICE_UNAVAILABLE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    async fn setup_test_server() -> Router {
        let health_checker = HealthChecker::new("test-service", "1.0.0");
        let mut server = ObservabilityServer::new(
            ObservabilityConfig::default(),
            health_checker,
        );
        server.router()
    }

    #[tokio::test]
    async fn test_health_endpoint() {
        let router = setup_test_server().await;

        let response = router
            .oneshot(Request::builder().uri("/health").body(Body::empty()).unwrap())
            .await
            .unwrap();

        // Unknown status initially
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn test_healthz_endpoint() {
        let router = setup_test_server().await;

        let response = router
            .oneshot(Request::builder().uri("/healthz").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn test_ready_endpoint() {
        let health_checker = HealthChecker::new("test", "1.0.0");
        let mut server = ObservabilityServer::new(
            ObservabilityConfig::default(),
            health_checker.clone(),
        );
        let router = server.router();

        // Initially not ready
        let response = router
            .clone()
            .oneshot(Request::builder().uri("/ready").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);

        // Mark ready
        health_checker.set_ready().await;
        let response = router
            .oneshot(Request::builder().uri("/ready").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_live_endpoint() {
        let router = setup_test_server().await;

        let response = router
            .oneshot(Request::builder().uri("/live").body(Body::empty()).unwrap())
            .await
            .unwrap();

        // Liveness is always OK if responding
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_metrics_endpoint() {
        let router = setup_test_server().await;

        let response = router
            .oneshot(Request::builder().uri("/metrics").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[test]
    fn test_config_defaults() {
        let config = ObservabilityConfig::default();
        assert_eq!(config.bind_addr.port(), 9090);
        assert!(config.enable_cors);
        assert!(config.enable_tracing);
    }

    #[test]
    fn test_config_with_port() {
        let config = ObservabilityConfig::with_port(8080);
        assert_eq!(config.bind_addr.port(), 8080);
    }
}
