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
//! - `API_PORT`: HTTP API port for REST API and health/metrics (default: 9090)
//! - `NODE_CPU`: CPU cores capacity (default: 4.0)
//! - `NODE_MEMORY_MB`: Memory capacity in MB (default: 8192)
//! - `NODE_DISK_MB`: Disk capacity in MB (default: 102400)
//! - `LOG_LEVEL`: Log level (default: "info")
//! - `LOG_JSON`: Use JSON log format (default: false)
//! - `AUTH_DISABLED`: Disable Ed25519 request authentication (default: true for dev)
//! - `RUNTIME_TYPE`: Container runtime type: "mock" or "youki" (default: based on feature)
//! - `YOUKI_BINARY`: Path to youki binary (default: "youki" - searches PATH)
//! - `BUNDLE_ROOT`: Root directory for OCI bundles (default: "/var/lib/orchestrator/bundles")
//! - `STATE_ROOT`: Root directory for runtime state (default: "/run/orchestrator")
//! - `MCP_STDIO`: Enable MCP server over stdio for Claude Code integration (default: false)
//!
//! # API Endpoints (port 9090 by default)
//!
//! ## REST API (requires `rest-api` feature)
//! - `POST /api/v1/workloads` - Create workload
//! - `GET /api/v1/workloads` - List workloads
//! - `GET /api/v1/workloads/:id` - Get workload
//! - `PUT /api/v1/workloads/:id` - Update workload
//! - `DELETE /api/v1/workloads/:id` - Delete workload
//! - `GET /api/v1/workloads/:id/instances` - List instances
//! - `GET /api/v1/nodes` - List nodes
//! - `GET /api/v1/nodes/:id` - Get node
//! - `GET /api/v1/cluster/status` - Cluster status
//!
//! ## Observability (requires `observability` feature)
//! - `GET /health` - Health check
//! - `GET /ready` - Readiness probe
//! - `GET /live` - Liveness probe
//! - `GET /metrics` - Prometheus metrics
//! - `GET /api/v1/events` - WebSocket event streaming

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

#[cfg(feature = "youki-runtime")]
use container_runtime::{YoukiCliConfig, YoukiCliRuntime};
use orchestrator_shared_types::{
    ContainerConfig, ContainerId, Keypair, Node, NodeId, NodeResources, NodeStatus,
    OrchestrationError, Result as OrchResult,
};
use scheduler_interface::SimpleScheduler;
use state_store_interface::{SqliteStateStore, StateStore};

#[cfg(feature = "mcp")]
use mcp_server::OrchestratorMcpServer;

#[cfg(feature = "observability")]
use observability::{
    EventHub, HealthChecker, MetricsRegistry, MockPubSubNetwork, NetworkBridgeConfig,
    NetworkEventBridge, ObservabilityConfig, ObservabilityServer,
};

#[cfg(feature = "rest-api")]
use orchestrator_core::api::{build_router as build_api_router, ApiState, AuthConfig};

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
    /// Disable authentication for development
    auth_disabled: bool,
    /// Container runtime type
    runtime_type: RuntimeType,
    /// Path to youki binary (only used with youki runtime)
    youki_binary: String,
    /// Root directory for OCI bundles
    bundle_root: String,
    /// Root directory for runtime state
    state_root: String,
    /// Enable MCP stdio server for Claude Code integration
    #[cfg(feature = "mcp")]
    mcp_stdio: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum NodeRole {
    Bootstrap,
    Worker,
}

/// Runtime type selection.
#[derive(Debug, Clone, Copy, PartialEq)]
enum RuntimeType {
    Mock,
    #[cfg(feature = "youki-runtime")]
    Youki,
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

        let public_addr_str =
            std::env::var("PUBLIC_ADDR").unwrap_or_else(|_| listen_addr.to_string());

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

        let cluster_id =
            std::env::var("CLUSTER_ID").unwrap_or_else(|_| "orchestrator-cluster".to_string());

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

        let auth_disabled = std::env::var("AUTH_DISABLED")
            .map(|v| v == "true" || v == "1")
            .unwrap_or(true); // Disabled by default for development

        // Runtime configuration
        let runtime_type = match std::env::var("RUNTIME_TYPE")
            .unwrap_or_else(|_| {
                // Default based on feature flag
                #[cfg(feature = "youki-runtime")]
                {
                    "youki".to_string()
                }
                #[cfg(not(feature = "youki-runtime"))]
                {
                    "mock".to_string()
                }
            })
            .to_lowercase()
            .as_str()
        {
            #[cfg(feature = "youki-runtime")]
            "youki" => RuntimeType::Youki,
            _ => RuntimeType::Mock,
        };

        let youki_binary = std::env::var("YOUKI_BINARY").unwrap_or_else(|_| "youki".to_string());

        let bundle_root = std::env::var("BUNDLE_ROOT")
            .unwrap_or_else(|_| "/var/lib/orchestrator/bundles".to_string());

        let state_root =
            std::env::var("STATE_ROOT").unwrap_or_else(|_| "/run/orchestrator".to_string());

        #[cfg(feature = "mcp")]
        let mcp_stdio = std::env::var("MCP_STDIO")
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
            auth_disabled,
            runtime_type,
            youki_binary,
            bundle_root,
            state_root,
            #[cfg(feature = "mcp")]
            mcp_stdio,
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
        self.containers
            .lock()
            .await
            .insert(id.clone(), config.clone());
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

    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(&config.log_level));

    if config.log_json {
        fmt().with_env_filter(filter).json().init();
    } else {
        fmt().with_env_filter(filter).init();
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
        runtime = ?config.runtime_type,
        "Starting orchestrator node"
    );

    // Validate configuration
    if config.role == NodeRole::Worker && config.seed_nodes.is_empty() {
        error!("Worker nodes require at least one seed node (SEED_NODES)");
        std::process::exit(1);
    }

    // Generate node keypair for identity
    let node_keypair = Keypair::generate();
    let node_id = node_keypair.public_key();
    info!("Generated node identity: {}", node_id);

    // Create node info
    let self_node = Node {
        id: node_id,
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
        node_id: node_id.to_string(),
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

    // Create container runtime based on configuration
    let runtime: Arc<dyn ContainerRuntime> = match config.runtime_type {
        RuntimeType::Mock => {
            info!("Using mock container runtime");
            Arc::new(MockRuntime::default())
        }
        #[cfg(feature = "youki-runtime")]
        RuntimeType::Youki => {
            info!(
                youki_binary = %config.youki_binary,
                bundle_root = %config.bundle_root,
                state_root = %config.state_root,
                "Using youki container runtime"
            );
            let youki_config = YoukiCliConfig {
                youki_binary: config.youki_binary.clone().into(),
                bundle_root: config.bundle_root.clone().into(),
                state_root: config.state_root.clone().into(),
                command_timeout: Duration::from_secs(30),
                stop_timeout: Duration::from_secs(10),
            };
            match YoukiCliRuntime::with_config(youki_config).await {
                Ok(runtime) => Arc::new(runtime),
                Err(e) => {
                    error!(
                        "Failed to initialize youki runtime: {}. Falling back to mock runtime.",
                        e
                    );
                    Arc::new(MockRuntime::default())
                }
            }
        }
    };

    // Create concrete stores first (needed for MCP server if enabled)
    let sqlite_store = Arc::new(
        SqliteStateStore::in_memory()
            .await
            .context("Failed to create in-memory SQLite state store")?,
    );
    let simple_scheduler = SimpleScheduler;

    // Wrap in Arc for orchestrator service
    let scheduler = Arc::new(simple_scheduler.clone());
    let state_store: Arc<dyn StateStore> = sqlite_store.clone();

    // Convert to trait object for orchestrator
    let cluster_manager_trait: Arc<dyn ClusterManager> = cluster_manager.clone();

    // Start MCP server over stdio if enabled
    #[cfg(feature = "mcp")]
    if config.mcp_stdio {
        // Create MCP server with cloned concrete types (dereference Arc and clone)
        let mcp_store = (*sqlite_store).clone(); // Clone the SqliteStateStore
        let mcp_cluster = (*cluster_manager).clone(); // Clone the ChitchatClusterManager
        let mcp_scheduler = simple_scheduler;

        tokio::spawn(async move {
            let mcp_server = OrchestratorMcpServer::new(mcp_store, mcp_cluster, mcp_scheduler);

            info!("Starting MCP server over stdio for Claude Code integration");
            if let Err(e) = mcp_server.serve_stdio().await {
                error!("MCP server error: {}", e);
            }
        });
    }

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

    // Start combined API/observability server
    #[cfg(feature = "observability")]
    {
        let health_checker = HealthChecker::new(
            format!("orchestrator-{}", node_id),
            env!("CARGO_PKG_VERSION"),
        );
        health_checker.mark_healthy("cluster_manager").await;
        health_checker.mark_healthy("state_store").await;
        health_checker.mark_healthy("runtime").await;
        health_checker.set_ready().await;

        let metrics_registry = MetricsRegistry::new();
        let obs_config = ObservabilityConfig::with_port(config.api_port);

        // Create event hub for WebSocket streaming
        let event_hub = EventHub::default();
        let event_hub_clone = event_hub.clone();

        // Create P2P network bridge for gossipsub messaging
        let mock_pubsub = Arc::new(MockPubSubNetwork::new(node_id.to_string()));
        let network_bridge = Arc::new(NetworkEventBridge::new(
            event_hub.clone(),
            mock_pubsub,
            NetworkBridgeConfig::default(),
        ));

        // Start the network bridge (subscribes to P2P topics)
        if let Err(e) = network_bridge.start().await {
            warn!(
                "Failed to start network bridge: {}. P2P messaging will be unavailable.",
                e
            );
        } else {
            info!("Network bridge started - P2P messaging enabled for gradient/election/credit/septal topics");
        }

        // Build API router if rest-api feature is enabled
        #[cfg(feature = "rest-api")]
        let api_router = {
            // Create API state
            let auth_config = if config.auth_disabled {
                AuthConfig::disabled()
            } else {
                AuthConfig::default()
            };

            let api_state = ApiState::new(
                state_store.clone(),
                cluster_manager.clone() as Arc<dyn ClusterManager>,
                _workload_tx.clone(),
                auth_config,
            );

            // Build API router
            build_api_router(api_state)
        };

        // Create observability server with optional API routes merged in
        #[cfg(feature = "rest-api")]
        let obs_server = ObservabilityServer::new(obs_config.clone(), health_checker)
            .with_event_hub(event_hub)
            .with_network_bridge(network_bridge)
            .with_metrics(metrics_registry)
            .await
            .with_additional_routes(api_router);

        #[cfg(not(feature = "rest-api"))]
        let obs_server = ObservabilityServer::new(obs_config.clone(), health_checker)
            .with_event_hub(event_hub)
            .with_network_bridge(network_bridge)
            .with_metrics(metrics_registry)
            .await;

        // Start cluster event forwarder to WebSocket clients
        let cluster_manager_for_events = cluster_manager.clone();
        tokio::spawn(async move {
            if let Ok(mut event_rx) = cluster_manager_for_events.subscribe_to_events().await {
                info!("Event forwarder started - streaming cluster events to WebSocket clients");
                loop {
                    match event_rx.recv().await {
                        Ok(cluster_event) => {
                            event_hub_clone
                                .broadcast_cluster_event(&cluster_event)
                                .await;
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                            warn!("Event forwarder lagged, missed {} events", n);
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                            warn!("Cluster event channel closed");
                            break;
                        }
                    }
                }
            } else {
                warn!("Failed to subscribe to cluster events for WebSocket streaming");
            }
        });

        // Start combined server using ObservabilityServer's serve method
        tokio::spawn(async move {
            if let Err(e) = obs_server.serve().await {
                error!("API server error: {}", e);
            }
        });

        #[cfg(feature = "rest-api")]
        info!(
            api_port = config.api_port,
            "Combined server started: REST API + WebSocket events + P2P bridge + health/metrics at port {}",
            config.api_port
        );

        #[cfg(not(feature = "rest-api"))]
        info!(
            api_port = config.api_port,
            "Observability server started with WebSocket event streaming + P2P bridge at /api/v1/events"
        );
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
    if let Err(e) = cluster_manager
        .update_self_status(NodeStatus::NotReady)
        .await
    {
        warn!("Failed to update status on shutdown: {}", e);
    }

    // Give time for status to propagate
    tokio::time::sleep(Duration::from_secs(2)).await;

    info!("Node shutdown complete");
    Ok(())
}
