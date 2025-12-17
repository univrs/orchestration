//! Production orchestrator node binary.
//!
//! This binary runs an orchestrator node that can operate as either a bootstrap
//! node or a worker node in a chitchat gossip cluster.
//!
//! # Configuration via Environment Variables
//!
//! - `NODE_ID`: Unique node identifier (UUID, auto-generated if not set)
//! - `NODE_ROLE`: Node role: "bootstrap" or "worker" (default: "worker")
//! - `LISTEN_ADDR`: Address to bind for gossip (default: "0.0.0.0:7280")
//! - `PUBLIC_ADDR`: Address to advertise to peers (default: same as LISTEN_ADDR)
//! - `SEED_NODES`: Comma-separated list of seed node addresses (required for workers)
//! - `CLUSTER_ID`: Cluster identifier (default: "orchestrator-cluster")
//! - `API_PORT`: HTTP API port for health/metrics (default: 9090)
//! - `NODE_CPU`: CPU cores capacity (default: 4.0)
//! - `NODE_MEMORY_MB`: Memory capacity in MB (default: 8192)
//! - `NODE_DISK_MB`: Disk capacity in MB (default: 102400)
//! - `LOG_LEVEL`: Log level (default: "info")
//! - `LOG_JSON`: Use JSON log format (default: false)

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use tracing::{error, info, warn};
use uuid::Uuid;

use cluster_manager::chitchat_manager::{ChitchatClusterConfig, ChitchatClusterManager};
use cluster_manager_interface::ClusterManager;
use container_runtime_interface::ContainerRuntime;
use orchestrator_core::start_orchestrator_service;
use orchestrator_shared_types::{
    ContainerId, ContainerConfig, Node, NodeId, NodeResources, NodeStatus,
    OrchestrationError, Result as OrchResult,
};
use scheduler_interface::SimpleScheduler;
use state_store_interface::in_memory::InMemoryStateStore;
use state_store_interface::StateStore;

#[cfg(feature = "observability")]
use observability::{
    HealthChecker, MetricsRegistry, ObservabilityConfig, ObservabilityServer,
};

/// Node configuration parsed from environment.
#[derive(Debug, Clone)]
struct NodeConfig {
    node_id: Uuid,
    role: NodeRole,
    listen_addr: SocketAddr,
    public_addr: SocketAddr,
    seed_nodes: Vec<String>,
    cluster_id: String,
    api_port: u16,
    cpu_cores: f32,
    memory_mb: u64,
    disk_mb: u64,
    log_level: String,
    log_json: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum NodeRole {
    Bootstrap,
    Worker,
}

impl NodeConfig {
    fn from_env() -> Result<Self> {
        let node_id = std::env::var("NODE_ID")
            .ok()
            .and_then(|s| Uuid::parse_str(&s).ok())
            .unwrap_or_else(Uuid::new_v4);

        let role = match std::env::var("NODE_ROLE")
            .unwrap_or_else(|_| "worker".to_string())
            .to_lowercase()
            .as_str()
        {
            "bootstrap" => NodeRole::Bootstrap,
            _ => NodeRole::Worker,
        };

        let listen_addr: SocketAddr = std::env::var("LISTEN_ADDR")
            .unwrap_or_else(|_| "0.0.0.0:7280".to_string())
            .parse()
            .context("Invalid LISTEN_ADDR")?;

        let public_addr_str = std::env::var("PUBLIC_ADDR")
            .unwrap_or_else(|_| listen_addr.to_string());

        // Try to parse as socket address first, then resolve as hostname
        let public_addr: SocketAddr = public_addr_str
            .parse()
            .or_else(|_| {
                // Try to resolve as hostname:port
                use std::net::ToSocketAddrs;
                public_addr_str
                    .to_socket_addrs()
                    .map_err(|e| anyhow::anyhow!("Failed to resolve {}: {}", public_addr_str, e))?
                    .next()
                    .ok_or_else(|| anyhow::anyhow!("No addresses found for {}", public_addr_str))
            })
            .context("Invalid PUBLIC_ADDR")?;

        let seed_nodes: Vec<String> = std::env::var("SEED_NODES")
            .unwrap_or_default()
            .split(',')
            .filter(|s| !s.is_empty())
            .map(|s| s.trim().to_string())
            .collect();

        let cluster_id = std::env::var("CLUSTER_ID")
            .unwrap_or_else(|_| "orchestrator-cluster".to_string());

        let api_port: u16 = std::env::var("API_PORT")
            .unwrap_or_else(|_| "9090".to_string())
            .parse()
            .unwrap_or(9090);

        let cpu_cores: f32 = std::env::var("NODE_CPU")
            .unwrap_or_else(|_| "4.0".to_string())
            .parse()
            .unwrap_or(4.0);

        let memory_mb: u64 = std::env::var("NODE_MEMORY_MB")
            .unwrap_or_else(|_| "8192".to_string())
            .parse()
            .unwrap_or(8192);

        let disk_mb: u64 = std::env::var("NODE_DISK_MB")
            .unwrap_or_else(|_| "102400".to_string())
            .parse()
            .unwrap_or(102400);

        let log_level = std::env::var("LOG_LEVEL").unwrap_or_else(|_| "info".to_string());
        let log_json = std::env::var("LOG_JSON")
            .map(|v| v == "true" || v == "1")
            .unwrap_or(false);

        Ok(NodeConfig {
            node_id,
            role,
            listen_addr,
            public_addr,
            seed_nodes,
            cluster_id,
            api_port,
            cpu_cores,
            memory_mb,
            disk_mb,
            log_level,
            log_json,
        })
    }
}

/// Mock container runtime for development/testing.
#[derive(Default, Clone)]
struct MockRuntime {
    containers: Arc<tokio::sync::Mutex<HashMap<ContainerId, ContainerConfig>>>,
}

#[async_trait::async_trait]
impl ContainerRuntime for MockRuntime {
    async fn init_node(&self, node_id: NodeId) -> OrchResult<()> {
        info!("MockRuntime: Initialized node {}", node_id);
        Ok(())
    }

    async fn create_container(
        &self,
        config: &ContainerConfig,
        options: &container_runtime_interface::CreateContainerOptions,
    ) -> OrchResult<ContainerId> {
        let id = Uuid::new_v4().to_string();
        self.containers.lock().await.insert(id.clone(), config.clone());
        info!(
            "MockRuntime: Created container {} for workload {} on node {}",
            id, options.workload_id, options.node_id
        );
        Ok(id)
    }

    async fn stop_container(&self, container_id: &ContainerId) -> OrchResult<()> {
        info!("MockRuntime: Stopped container {}", container_id);
        Ok(())
    }

    async fn remove_container(&self, container_id: &ContainerId) -> OrchResult<()> {
        self.containers.lock().await.remove(container_id);
        info!("MockRuntime: Removed container {}", container_id);
        Ok(())
    }

    async fn get_container_status(
        &self,
        container_id: &ContainerId,
    ) -> OrchResult<container_runtime_interface::ContainerStatus> {
        let containers = self.containers.lock().await;
        if containers.contains_key(container_id) {
            Ok(container_runtime_interface::ContainerStatus {
                id: container_id.clone(),
                state: "Running".to_string(),
                exit_code: None,
                error_message: None,
            })
        } else {
            Err(OrchestrationError::RuntimeError(format!(
                "Container {} not found",
                container_id
            )))
        }
    }

    async fn list_containers(
        &self,
        _node_id: NodeId,
    ) -> OrchResult<Vec<container_runtime_interface::ContainerStatus>> {
        let containers = self.containers.lock().await;
        Ok(containers
            .keys()
            .map(|id| container_runtime_interface::ContainerStatus {
                id: id.clone(),
                state: "Running".to_string(),
                exit_code: None,
                error_message: None,
            })
            .collect())
    }
}

fn init_logging(config: &NodeConfig) {
    use tracing_subscriber::{fmt, EnvFilter};

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(&config.log_level));

    if config.log_json {
        fmt()
            .with_env_filter(filter)
            .json()
            .init();
    } else {
        fmt()
            .with_env_filter(filter)
            .init();
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let config = NodeConfig::from_env()?;

    // Initialize logging
    init_logging(&config);

    info!(
        node_id = %config.node_id,
        role = ?config.role,
        listen_addr = %config.listen_addr,
        public_addr = %config.public_addr,
        cluster_id = %config.cluster_id,
        "Starting orchestrator node"
    );

    // Validate configuration
    if config.role == NodeRole::Worker && config.seed_nodes.is_empty() {
        error!("Worker nodes require at least one seed node (SEED_NODES)");
        std::process::exit(1);
    }

    // Create node info
    let self_node = Node {
        id: config.node_id,
        address: config.public_addr.to_string(),
        status: NodeStatus::Ready,
        labels: HashMap::new(),
        resources_capacity: NodeResources {
            cpu_cores: config.cpu_cores,
            memory_mb: config.memory_mb,
            disk_mb: config.disk_mb,
        },
        resources_allocatable: NodeResources {
            cpu_cores: config.cpu_cores * 0.9, // Reserve 10% for system
            memory_mb: (config.memory_mb as f64 * 0.9) as u64,
            disk_mb: (config.disk_mb as f64 * 0.9) as u64,
        },
    };

    // Create chitchat cluster manager
    let chitchat_config = ChitchatClusterConfig {
        node_id: config.node_id.to_string(),
        listen_addr: config.listen_addr,
        public_addr: config.public_addr,
        seed_nodes: config.seed_nodes.clone(),
        gossip_interval: Duration::from_millis(500),
        failure_detection_threshold: 8.0,
        dead_node_grace_period: Duration::from_secs(60),
        cluster_id: config.cluster_id.clone(),
        generation_id: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };

    let cluster_manager = Arc::new(ChitchatClusterManager::new(chitchat_config));

    // Set self node info before initialization
    cluster_manager.set_self_node(self_node.clone()).await;

    // Create other components
    let runtime: Arc<dyn ContainerRuntime> = Arc::new(MockRuntime::default());
    let scheduler = Arc::new(SimpleScheduler);
    let state_store: Arc<dyn StateStore> = Arc::new(InMemoryStateStore::new());

    // Convert to trait object for orchestrator
    let cluster_manager_trait: Arc<dyn ClusterManager> = cluster_manager.clone();

    // Start the orchestrator service
    let _workload_tx = start_orchestrator_service(
        state_store.clone(),
        runtime.clone(),
        cluster_manager_trait,
        scheduler,
    )
    .await
    .context("Failed to start orchestrator service")?;

    info!(
        node_id = %config.node_id,
        role = ?config.role,
        "Orchestrator service started"
    );

    // Start health/metrics server if observability is enabled
    #[cfg(feature = "observability")]
    {
        let health_checker = HealthChecker::new(
            format!("orchestrator-{}", config.node_id),
            env!("CARGO_PKG_VERSION"),
        );
        health_checker.mark_healthy("cluster_manager").await;
        health_checker.mark_healthy("state_store").await;
        health_checker.mark_healthy("runtime").await;
        health_checker.set_ready().await;

        let metrics_registry = MetricsRegistry::new();
        let obs_config = ObservabilityConfig::with_port(config.api_port);

        let obs_server = ObservabilityServer::new(obs_config, health_checker)
            .with_metrics(metrics_registry)
            .await;

        tokio::spawn(async move {
            if let Err(e) = obs_server.serve().await {
                error!("Observability server error: {}", e);
            }
        });

        info!(api_port = config.api_port, "Observability server started");
    }

    // Print cluster info
    match config.role {
        NodeRole::Bootstrap => {
            info!(
                "Bootstrap node ready. Other nodes can join using: SEED_NODES={}",
                config.public_addr
            );
        }
        NodeRole::Worker => {
            info!(
                "Worker node connected to cluster via seeds: {:?}",
                config.seed_nodes
            );
        }
    }

    // Keep running
    info!("Node is running. Press Ctrl+C to stop.");

    // Wait for shutdown signal
    tokio::signal::ctrl_c()
        .await
        .context("Failed to listen for ctrl+c")?;

    info!("Shutting down...");

    // Update status to NotReady before shutdown
    if let Err(e) = cluster_manager.update_self_status(NodeStatus::NotReady).await {
        warn!("Failed to update status on shutdown: {}", e);
    }

    // Give time for status to propagate
    tokio::time::sleep(Duration::from_secs(2)).await;

    info!("Node shutdown complete");
    Ok(())
}
