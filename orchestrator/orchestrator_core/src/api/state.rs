//! API server state.

use std::sync::Arc;

use tokio::sync::mpsc;

use cluster_manager_interface::ClusterManager;
use container_runtime_interface::ContainerRuntime;
use orchestrator_shared_types::WorkloadDefinition;
use state_store_interface::StateStore;

use super::auth::AuthConfig;

/// Shared state for the API server.
#[derive(Clone)]
pub struct ApiState {
    /// State store for persistence.
    pub state_store: Arc<dyn StateStore>,
    /// Cluster manager for node information.
    pub cluster_manager: Arc<dyn ClusterManager>,
    /// Channel to submit workloads to the orchestrator.
    pub workload_tx: mpsc::Sender<WorkloadDefinition>,
    /// Authentication configuration.
    pub auth_config: Arc<AuthConfig>,
    /// Optional container runtime for log access.
    pub container_runtime: Option<Arc<dyn ContainerRuntime>>,
}

impl ApiState {
    /// Create new API state.
    pub fn new(
        state_store: Arc<dyn StateStore>,
        cluster_manager: Arc<dyn ClusterManager>,
        workload_tx: mpsc::Sender<WorkloadDefinition>,
        auth_config: AuthConfig,
    ) -> Self {
        Self {
            state_store,
            cluster_manager,
            workload_tx,
            auth_config: Arc::new(auth_config),
            container_runtime: None,
        }
    }

    /// Create new API state with container runtime for log access.
    pub fn with_runtime(
        state_store: Arc<dyn StateStore>,
        cluster_manager: Arc<dyn ClusterManager>,
        workload_tx: mpsc::Sender<WorkloadDefinition>,
        auth_config: AuthConfig,
        container_runtime: Arc<dyn ContainerRuntime>,
    ) -> Self {
        Self {
            state_store,
            cluster_manager,
            workload_tx,
            auth_config: Arc::new(auth_config),
            container_runtime: Some(container_runtime),
        }
    }

    /// Create API state with auth disabled.
    pub fn new_without_auth(
        state_store: Arc<dyn StateStore>,
        cluster_manager: Arc<dyn ClusterManager>,
        workload_tx: mpsc::Sender<WorkloadDefinition>,
    ) -> Self {
        Self::new(state_store, cluster_manager, workload_tx, AuthConfig::disabled())
    }

    /// Set the container runtime for log access.
    pub fn set_runtime(&mut self, runtime: Arc<dyn ContainerRuntime>) {
        self.container_runtime = Some(runtime);
    }
}
