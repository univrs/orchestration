//! API route definitions.

use axum::{
    middleware,
    routing::{delete, get, post, put},
    Router,
};
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

use super::auth::auth_layer;
use super::handlers;
use super::state::ApiState;

/// Build the API router with all routes.
pub fn build_router(state: ApiState) -> Router {
    let auth_config = state.auth_config.clone();

    // Workload routes
    let workload_routes = Router::new()
        .route("/", post(handlers::create_workload))
        .route("/", get(handlers::list_workloads))
        .route("/:workload_id", get(handlers::get_workload))
        .route("/:workload_id", put(handlers::update_workload))
        .route("/:workload_id", delete(handlers::delete_workload))
        .route("/:workload_id/instances", get(handlers::list_workload_instances))
        .route("/:workload_id/logs", get(handlers::get_workload_logs))
        .route("/:workload_id/logs/stream", get(handlers::stream_workload_logs))
        .route("/:workload_id/instances/:instance_id/logs", get(handlers::get_instance_logs));

    // Node routes
    let node_routes = Router::new()
        .route("/", get(handlers::list_nodes))
        .route("/:node_id", get(handlers::get_node));

    // Cluster routes
    let cluster_routes = Router::new()
        .route("/status", get(handlers::get_cluster_status));

    // Combine all v1 API routes
    let api_v1 = Router::new()
        .nest("/workloads", workload_routes)
        .nest("/nodes", node_routes)
        .nest("/cluster", cluster_routes);

    // Build main router with middleware
    let mut router = Router::new()
        .nest("/api/v1", api_v1)
        .layer(middleware::from_fn_with_state(auth_config, auth_layer))
        .with_state(state);

    // Add CORS support
    router = router.layer(
        CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any),
    );

    // Add request tracing
    router = router.layer(TraceLayer::new_for_http());

    router
}

/// Configuration for the API server.
#[derive(Debug, Clone)]
pub struct ApiServerConfig {
    /// Address to bind the server to.
    pub bind_addr: std::net::SocketAddr,
}

impl Default for ApiServerConfig {
    fn default() -> Self {
        Self {
            bind_addr: std::net::SocketAddr::from(([0, 0, 0, 0], 8080)),
        }
    }
}

impl ApiServerConfig {
    /// Create config with specific port.
    pub fn with_port(port: u16) -> Self {
        Self {
            bind_addr: std::net::SocketAddr::from(([0, 0, 0, 0], port)),
        }
    }
}

/// API server.
pub struct ApiServer {
    config: ApiServerConfig,
    state: ApiState,
}

impl ApiServer {
    /// Create a new API server.
    pub fn new(config: ApiServerConfig, state: ApiState) -> Self {
        Self { config, state }
    }

    /// Start the server (blocking).
    pub async fn serve(self) -> Result<(), std::io::Error> {
        let router = build_router(self.state);
        let listener = tokio::net::TcpListener::bind(self.config.bind_addr).await?;

        tracing::info!(
            addr = %self.config.bind_addr,
            "Starting API server"
        );

        axum::serve(listener, router).await
    }

    /// Start the server in the background.
    pub fn spawn(self) -> tokio::task::JoinHandle<Result<(), std::io::Error>> {
        tokio::spawn(async move { self.serve().await })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tokio::sync::mpsc;

    use orchestrator_shared_types::WorkloadDefinition;
    use state_store_interface::in_memory::InMemoryStateStore;

    // Mock cluster manager for tests
    mod mock {
        use async_trait::async_trait;
        use cluster_manager_interface::{ClusterEvent, ClusterManager};
        use orchestrator_shared_types::{Node, NodeId, Result};
        use tokio::sync::watch;

        #[derive(Default)]
        pub struct MockClusterManager;

        #[async_trait]
        impl ClusterManager for MockClusterManager {
            async fn initialize(&self) -> Result<()> {
                Ok(())
            }

            async fn get_node(&self, _node_id: &NodeId) -> Result<Option<Node>> {
                Ok(None)
            }

            async fn list_nodes(&self) -> Result<Vec<Node>> {
                Ok(vec![])
            }

            async fn subscribe_to_events(&self) -> Result<watch::Receiver<Option<ClusterEvent>>> {
                let (_, rx) = watch::channel(None);
                Ok(rx)
            }
        }
    }

    fn create_test_state() -> ApiState {
        let state_store: Arc<dyn state_store_interface::StateStore> =
            Arc::new(InMemoryStateStore::new());
        let cluster_manager: Arc<dyn cluster_manager_interface::ClusterManager> =
            Arc::new(mock::MockClusterManager);
        let (workload_tx, _rx) = mpsc::channel::<WorkloadDefinition>(100);

        ApiState::new_without_auth(state_store, cluster_manager, workload_tx)
    }

    #[test]
    fn test_build_router() {
        let state = create_test_state();
        let _router = build_router(state);
        // Router builds successfully
    }

    #[test]
    fn test_api_server_config_defaults() {
        let config = ApiServerConfig::default();
        assert_eq!(config.bind_addr.port(), 8080);
    }

    #[test]
    fn test_api_server_config_with_port() {
        let config = ApiServerConfig::with_port(9000);
        assert_eq!(config.bind_addr.port(), 9000);
    }
}
