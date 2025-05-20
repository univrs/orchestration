### Rust Orchestration 

Let's break down how we can start realizing this vision with Rust code, focusing on **Phase 1: Core Rust Orchestration Primitives** as outlined in your implementation roadmap.

We'll aim to create a foundational structure that can be expanded upon. This will involve setting up a Cargo workspace and defining some initial crates for the core components.

**Our Initial Goals for Phase 1 (Simplified for a starting point):**

1.  **Project Structure:** Set up a Cargo workspace with initial crates.
2.  **Shared Types:** Define basic data structures (e.g., `Node`, `WorkloadDefinition`, `ContainerConfig`) that will be used across components.
3.  **Container Runtime Abstraction:** Define a trait for interacting with a container runtime (like Youki), without implementing the full runtime itself yet.
4.  **Cluster Management Abstraction:** Define a trait for cluster membership and health, inspired by `memberlist`.
5.  **Workload Scheduler Abstraction:** Define a trait for basic workload scheduling.
6.  **Basic Orchestrator Core:** A rudimentary main loop or service that can (in the future) tie these components together.

Let's get started!

---

**Step 1: Setting up the Cargo Workspace**

A Cargo workspace is ideal for managing multiple related crates.

1.  Create a new directory for your project, let's call it `ai_native_orchestrator`.
2.  Inside `ai_native_orchestrator`, create a `Cargo.toml` file. This will be the workspace manifest.

```bash
mkdir ai_native_orchestrator
cd ai_native_orchestrator
touch Cargo.toml
```

3.  Edit `ai_native_orchestrator/Cargo.toml`:

    ```toml
    [workspace]
    members = [
        "orchestrator_core",
        "orchestrator_shared_types",
        "container_runtime_interface",
        "cluster_manager_interface",
        "scheduler_interface",
        # Future crates:
        # "mcp_server",
        # "cloud_agnostic_layer",
        # "orchestrator_cli",
    ]

    resolver = "2" # Recommended for new projects

    [workspace.dependencies]
    # Common dependencies will go here
    tokio = { version = "1", features = ["full"] }
    serde = { version = "1.0", features = ["derive"] }
    serde_json = "1.0"
    thiserror = "1.0"
    uuid = { version = "1", features = ["v4", "serde"] }
    tracing = "0.1"
    tracing-subscriber = { version = "0.3", features = ["env-filter"] }
    async-trait = "0.1.77" # For async methods in traits
    ```

    *   `resolver = "2"`: Uses the new feature resolver, which is generally better for dependency management.
    *   `workspace.dependencies`: This section allows us to define common dependencies that all member crates can use without specifying the version in each crate's `Cargo.toml`.

---

**Step 2: Creating Initial Crates**

Now, let's create the directories and `Cargo.toml` files for our initial set of crates.

1.  **`orchestrator_shared_types`**: For common data structures.

    ```bash
    cargo new orchestrator_shared_types
    ```
    Edit `ai_native_orchestrator/orchestrator_shared_types/Cargo.toml`:
    ```toml
    [package]
    name = "orchestrator_shared_types"
    version = "0.1.0"
    edition = "2021"

    [dependencies]
    serde = { workspace = true }
    uuid = { workspace = true }
    thiserror = { workspace = true }
    ```

2.  **`container_runtime_interface`**: For the container runtime abstraction.

    ```bash
    cargo new container_runtime_interface
    ```
    Edit `ai_native_orchestrator/container_runtime_interface/Cargo.toml`:
    ```toml
    [package]
    name = "container_runtime_interface"
    version = "0.1.0"
    edition = "2021"

    [dependencies]
    orchestrator_shared_types = { path = "../orchestrator_shared_types" }
    tokio = { workspace = true }
    async-trait = { workspace = true }
    thiserror = { workspace = true }
    serde = { workspace = true } # For config structs
    ```

3.  **`cluster_manager_interface`**: For cluster state and membership.

    ```bash
    cargo new cluster_manager_interface
    ```
    Edit `ai_native_orchestrator/cluster_manager_interface/Cargo.toml`:
    ```toml
    [package]
    name = "cluster_manager_interface"
    version = "0.1.0"
    edition = "2021"

    [dependencies]
    orchestrator_shared_types = { path = "../orchestrator_shared_types" }
    tokio = { workspace = true }
    async-trait = { workspace = true }
    thiserror = { workspace = true }
    ```

4.  **`scheduler_interface`**: For workload scheduling logic.

    ```bash
    cargo new scheduler_interface
    ```
    Edit `ai_native_orchestrator/scheduler_interface/Cargo.toml`:
    ```toml
    [package]
    name = "scheduler_interface"
    version = "0.1.0"
    edition = "2021"

    [dependencies]
    orchestrator_shared_types = { path = "../orchestrator_shared_types" }
    cluster_manager_interface = { path = "../cluster_manager_interface" } # Schedulers need info about nodes
    tokio = { workspace = true }
    async-trait = { workspace = true }
    thiserror = { workspace = true }
    ```

5.  **`orchestrator_core`**: The main orchestrator logic. This will likely be a binary crate eventually, or a library consumed by one. Let's start it as a library.

    ```bash
    cargo new orchestrator_core
    ```
    Edit `ai_native_orchestrator/orchestrator_core/Cargo.toml`:
    ```toml
    [package]
    name = "orchestrator_core"
    version = "0.1.0"
    edition = "2021"

    [dependencies]
    orchestrator_shared_types = { path = "../orchestrator_shared_types" }
    container_runtime_interface = { path = "../container_runtime_interface" }
    cluster_manager_interface = { path = "../cluster_manager_interface" }
    scheduler_interface = { path = "../scheduler_interface" }

    tokio = { workspace = true }
    tracing = { workspace = true }
    thiserror = { workspace = true }
    ```

Your directory structure should now look like this:

```
ai_native_orchestrator/
├── Cargo.toml
├── orchestrator_core/
│   ├── Cargo.toml
│   └── src/
│       └── lib.rs
├── orchestrator_shared_types/
│   ├── Cargo.toml
│   └── src/
│       └── lib.rs
├── container_runtime_interface/
│   ├── Cargo.toml
│   └── src/
│       └── lib.rs
├── cluster_manager_interface/
│   ├── Cargo.toml
│   └── src/
│       └── lib.rs
└── scheduler_interface/
    ├── Cargo.toml
    └── src/
        └── lib.rs
```

You can test if the workspace is set up correctly by running `cargo check --workspace` or `cargo build --workspace` from the `ai_native_orchestrator` root directory.

---

**Step 3: Defining Shared Types (`orchestrator_shared_types`)**

Let's define some basic data structures in `orchestrator_shared_types/src/lib.rs`.

File: `ai_native_orchestrator/orchestrator_shared_types/src/lib.rs`
```rust
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;
use uuid::Uuid;

pub type NodeId = Uuid;
pub type WorkloadId = Uuid;
pub type ContainerId = String; // Typically a hash provided by the runtime

#[derive(Debug, Error)]
pub enum OrchestrationError {
    #[error("Node not found: {0}")]
    NodeNotFound(NodeId),
    #[error("Workload not found: {0}")]
    WorkloadNotFound(WorkloadId),
    #[error("Container runtime error: {0}")]
    RuntimeError(String),
    #[error("Scheduling error: {0}")]
    SchedulingError(String),
    #[error("Cluster management error: {0}")]
    ClusterError(String),
    #[error("State persistence error: {0}")]
    StateError(String),
    #[error("Configuration error: {0}")]
    ConfigError(String),
    #[error("Network error: {0}")]
    NetworkError(String),
    #[error("Internal error: {0}")]
    InternalError(String),
    #[error("Feature not implemented: {0}")]
    NotImplemented(String),
}

// Represents a node in the cluster
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Node {
    pub id: NodeId,
    pub address: String, // e.g., "10.0.0.1:8080"
    pub status: NodeStatus,
    pub labels: HashMap<String, String>,
    pub resources_capacity: NodeResources,
    pub resources_allocatable: NodeResources, // Capacity - system overhead
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum NodeStatus {
    Ready,
    NotReady,
    Unknown,
    Down,
}

// Represents available/requested resources on a node or for a workload
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct NodeResources {
    pub cpu_cores: f32,    // e.g., 2.0 for 2 cores, 0.5 for half a core
    pub memory_mb: u64,    // Memory in Megabytes
    pub disk_mb: u64,      // Disk space in Megabytes
    // Potentially GPU resources, custom resources, etc.
}

// Configuration for a single container within a workload
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContainerConfig {
    pub name: String,
    pub image: String, // e.g., "nginx:latest"
    pub command: Option<Vec<String>>,
    pub args: Option<Vec<String>>,
    pub env_vars: HashMap<String, String>,
    pub ports: Vec<PortMapping>,
    pub resource_requests: NodeResources,
    // Volume mounts, health checks, etc. would go here
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PortMapping {
    pub container_port: u16,
    pub host_port: Option<u16>, // If None, runtime chooses an ephemeral port
    pub protocol: String,       // "tcp" or "udp"
}

// Defines a workload to be run on the cluster
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkloadDefinition {
    pub id: WorkloadId,
    pub name: String, // User-friendly name
    pub containers: Vec<ContainerConfig>,
    pub replicas: u32,
    pub labels: HashMap<String, String>, // For scheduling, selection
    // Placement constraints, update strategy, etc.
}

// Represents an instance of a workload running on a specific node
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkloadInstance {
    pub id: Uuid, // Unique ID for this instance
    pub workload_id: WorkloadId,
    pub node_id: NodeId,
    pub container_ids: Vec<ContainerId>, // IDs of containers run by the runtime for this instance
    pub status: WorkloadInstanceStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum WorkloadInstanceStatus {
    Pending,
    Running,
    Succeeded,
    Failed,
    Unknown,
    Terminating,
}

// Generic result type for orchestration operations
pub type Result<T> = std::result::Result<T, OrchestrationError>;
```
**Note:** This is a starting point. These structs will evolve significantly.

---

**Step 4: Defining Interfaces (Traits)**

Now, let's define the traits for our abstracted components.

**A. `container_runtime_interface`**

File: `ai_native_orchestrator/container_runtime_interface/src/lib.rs`
```rust
use async_trait::async_trait;
use orchestrator_shared_types::{ContainerConfig, ContainerId, NodeId, OrchestrationError, Result, WorkloadId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateContainerOptions {
    pub workload_id: WorkloadId,
    pub node_id: NodeId, // Where the container should run (managed by scheduler)
    // Potentially OCI spec details or other runtime-specific configurations
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerStatus {
    pub id: ContainerId,
    pub state: String, // e.g., "running", "stopped", "error" (OCI states)
    pub exit_code: Option<i32>,
    pub error_message: Option<String>,
}

/// Trait for interacting with a container runtime (e.g., Youki, runc)
#[async_trait]
pub trait ContainerRuntime: Send + Sync {
    /// Initializes the runtime on a given node.
    async fn init_node(&self, node_id: NodeId) -> Result<()>;

    /// Creates and starts a container based on the provided configuration.
    async fn create_container(
        &self,
        config: &ContainerConfig,
        options: &CreateContainerOptions,
    ) -> Result<ContainerId>;

    /// Stops a container.
    async fn stop_container(&self, container_id: &ContainerId) -> Result<()>;

    /// Removes a stopped container.
    async fn remove_container(&self, container_id: &ContainerId) -> Result<()>;

    /// Gets the status of a container.
    async fn get_container_status(&self, container_id: &ContainerId) -> Result<ContainerStatus>;

    /// Lists all containers managed by this runtime on the current node.
    async fn list_containers(&self, node_id: NodeId) -> Result<Vec<ContainerStatus>>;

    // Potentially methods for pulling images, managing networks, volumes, etc.
    // async fn pull_image(&self, image_name: &str) -> Result<()>;
}

// Example of a specific error for this interface
#[derive(Debug, thiserror::Error)]
pub enum RuntimeError {
    #[error("OCI Spec validation failed: {0}")]
    OciValidationError(String),
    #[error("Container not found: {0}")]
    ContainerNotFound(ContainerId),
    #[error("Runtime communication error: {0}")]
    CommunicationError(String),
    #[error("Underlying I/O error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Initialization failed: {0}")]
    InitializationFailed(String),
}

// Helper to convert specific errors to the general OrchestrationError
impl From<RuntimeError> for OrchestrationError {
    fn from(err: RuntimeError) -> Self {
        OrchestrationError::RuntimeError(err.to_string())
    }
}
```

**B. `cluster_manager_interface`**

File: `ai_native_orchestrator/cluster_manager_interface/src/lib.rs`
```rust
use async_trait::async_trait;
use orchestrator_shared_types::{Node, NodeId, OrchestrationError, Result};
use std::sync::Arc; // For sharing state if needed
use tokio::sync::watch; // For broadcasting cluster changes

/// Represents an event related to cluster membership or node status.
#[derive(Debug, Clone, PartialEq)]
pub enum ClusterEvent {
    NodeAdded(Node),
    NodeRemoved(NodeId),
    NodeUpdated(Node), // e.g., status change, resource update
}

#[async_trait]
pub trait ClusterManager: Send + Sync {
    /// Initializes the cluster manager.
    async fn initialize(&self) -> Result<()>;

    /// Gets information about a specific node.
    async fn get_node(&self, node_id: &NodeId) -> Result<Option<Node>>;

    /// Lists all nodes currently known to the cluster.
    async fn list_nodes(&self) -> Result<Vec<Node>>;

    /// Subscribes to cluster events (node additions, removals, updates).
    /// Returns a receiver channel for `ClusterEvent`.
    async fn subscribe_to_events(&self) -> Result<watch::Receiver<Option<ClusterEvent>>>;

    // Methods for leader election might go here if the manager handles it.
    // async fn is_leader(&self) -> Result<bool>;

    // Health checking logic would be invoked by this manager.
    // For example, the manager might periodically ping nodes.
}

// Example of a specific error for this interface
#[derive(Debug, thiserror::Error)]
pub enum ClusterManagerError {
    #[error("Node discovery failed: {0}")]
    DiscoveryFailed(String),
    #[error("Node health check failed for {0}: {1}")]
    HealthCheckFailed(NodeId, String),
    #[error("Communication error with peer: {0}")]
    PeerCommunicationError(String),
    #[error("Subscription failed: {0}")]
    SubscriptionFailed(String),
}

impl From<ClusterManagerError> for OrchestrationError {
    fn from(err: ClusterManagerError) -> Self {
        OrchestrationError::ClusterError(err.to_string())
    }
}
```

**C. `scheduler_interface`**

File: `ai_native_orchestrator/scheduler_interface/src/lib.rs`
```rust
use async_trait::async_trait;
use orchestrator_shared_types::{Node, NodeId, OrchestrationError, Result, WorkloadDefinition, WorkloadInstance};
use cluster_manager_interface::ClusterManager; // To get node information
use std::sync::Arc;

/// Input for a scheduling decision.
#[derive(Debug, Clone)]
pub struct ScheduleRequest {
    pub workload_definition: Arc<WorkloadDefinition>,
    pub current_instances: Vec<WorkloadInstance>, // Existing instances of this workload
    // Potentially other constraints like anti-affinity, taints/tolerations
}

/// Output of a scheduling decision.
#[derive(Debug, Clone)]
pub enum ScheduleDecision {
    /// Assign the workload instance to the specified node.
    AssignNode(NodeId),
    /// No suitable node found, or workload should not be scheduled right now.
    NoPlacement(String), // Reason for no placement
    /// An error occurred during scheduling.
    Error(String),
}

#[async_trait]
pub trait Scheduler: Send + Sync {
    /// Makes a scheduling decision for a given workload.
    /// This would typically be called when a new workload is created or an existing one needs rescheduling.
    async fn schedule(
        &self,
        request: &ScheduleRequest,
        available_nodes: &[Node], // Current state of schedulable nodes
    ) -> Result<Vec<ScheduleDecision>>; // Returns a decision for each replica to be scheduled

    // Potentially, a method to decide if a workload instance should be preempted
    // async fn decide_preemption(&self, instance: &WorkloadInstance, nodes: &[Node]) -> Result<bool>;
}

// Example of a specific error for this interface
#[derive(Debug, thiserror::Error)]
pub enum SchedulerError {
    #[error("No suitable nodes available for workload {0}: {1}")]
    NoSuitableNodes(String, String), // Workload name, reason
    #[error("Insufficient resources on candidate node {0} for workload {1}")]
    InsufficientResources(NodeId, String),
    #[error("Failed to evaluate placement constraints: {0}")]
    ConstraintEvaluationFailed(String),
}

impl From<SchedulerError> for OrchestrationError {
    fn from(err: SchedulerError) -> Self {
        OrchestrationError::SchedulingError(err.to_string())
    }
}

// A very simple scheduler implementation for demonstration
pub struct SimpleScheduler;

#[async_trait]
impl Scheduler for SimpleScheduler {
    async fn schedule(
        &self,
        request: &ScheduleRequest,
        available_nodes: &[Node],
    ) -> Result<Vec<ScheduleDecision>> {
        let mut decisions = Vec::new();
        let needed_replicas = request.workload_definition.replicas.saturating_sub(request.current_instances.len() as u32);

        if needed_replicas == 0 {
            return Ok(decisions);
        }

        if available_nodes.is_empty() {
            for _ in 0..needed_replicas {
                decisions.push(ScheduleDecision::NoPlacement("No nodes available".to_string()));
            }
            return Ok(decisions);
        }

        // Super simple: round-robin or pick first available that meets basic criteria (if any)
        // For now, just pick the first available node for all needed replicas (very naive)
        let mut node_iter = available_nodes.iter().cycle(); // Cycle through nodes

        for _ in 0..needed_replicas {
            if let Some(node) = node_iter.next() {
                // TODO: Actual resource checking against node.resources_allocatable
                // and request.workload_definition.containers[*].resource_requests
                decisions.push(ScheduleDecision::AssignNode(node.id));
            } else {
                // This case should not be hit if available_nodes is not empty due to cycle()
                // but as a safeguard:
                decisions.push(ScheduleDecision::NoPlacement("Failed to pick a node (internal error)".to_string()));
            }
        }
        Ok(decisions)
    }
}
```

---

**Step 5: `orchestrator_core` Structure (Initial Sketch)**

File: `ai_native_orchestrator/orchestrator_core/src/lib.rs`
```rust
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use std::collections::HashMap;

use orchestrator_shared_types::{
    Node, NodeId, OrchestrationError, Result, WorkloadDefinition, WorkloadId, WorkloadInstance,
    WorkloadInstanceStatus,
};
use container_runtime_interface::ContainerRuntime;
use cluster_manager_interface::{ClusterEvent, ClusterManager};
use scheduler_interface::{ScheduleDecision, ScheduleRequest, Scheduler, SimpleScheduler}; // Using SimpleScheduler for now
use tracing::{error, info, warn};

// In-memory state for now. This would be replaced by a persistent store (e.g., etcd, TiKV wrapper).
#[derive(Default)]
struct OrchestratorState {
    nodes: HashMap<NodeId, Node>,
    workloads: HashMap<WorkloadId, Arc<WorkloadDefinition>>,
    instances: HashMap<WorkloadId, Vec<WorkloadInstance>>, // Instances per workload
}

pub struct Orchestrator {
    state: Arc<Mutex<OrchestratorState>>,
    runtime: Arc<dyn ContainerRuntime>,
    cluster_manager: Arc<dyn ClusterManager>,
    scheduler: Arc<dyn Scheduler>,
    // Channel for submitting new workloads or updates
    workload_tx: mpsc::Sender<WorkloadDefinition>,
    workload_rx: mpsc::Receiver<WorkloadDefinition>,
}

impl Orchestrator {
    pub fn new(
        runtime: Arc<dyn ContainerRuntime>,
        cluster_manager: Arc<dyn ClusterManager>,
        scheduler: Arc<dyn Scheduler>,
    ) -> Self {
        let (workload_tx, workload_rx) = mpsc::channel(100); // Buffer size 100
        Orchestrator {
            state: Arc::new(Mutex::new(OrchestratorState::default())),
            runtime,
            cluster_manager,
            scheduler,
            workload_tx,
            workload_rx,
        }
    }

    pub fn get_workload_sender(&self) -> mpsc::Sender<WorkloadDefinition> {
        self.workload_tx.clone()
    }

    pub async fn run(&mut self) -> Result<()> {
        info!("Orchestrator starting...");

        self.cluster_manager.initialize().await?;
        let mut cluster_events_rx = self.cluster_manager.subscribe_to_events().await?;
        
        info!("Cluster manager initialized and subscribed to events.");

        loop {
            tokio::select! {
                // Listen for new/updated workload definitions
                Some(workload_def) = self.workload_rx.recv() => {
                    info!("Received workload definition: {} ({})", workload_def.name, workload_def.id);
                    if let Err(e) = self.handle_workload_update(workload_def).await {
                        error!("Failed to handle workload update: {:?}", e);
                    }
                }
                // Listen for cluster events (node changes)
                Ok(Some(event)) = cluster_events_rx.recv() => {
                    info!("Received cluster event: {:?}", event);
                    if let Err(e) = self.handle_cluster_event(event).await {
                         error!("Failed to handle cluster event: {:?}", e);
                    }
                    // After a cluster event, might need to re-evaluate workload placements
                    if let Err(e) = self.reconcile_all_workloads().await {
                        error!("Failed during reconciliation after cluster event: {:?}", e);
                    }
                }
                // TODO: Periodic reconciliation loop (e.g., every 30 seconds)
                // This would ensure desired state matches actual state.
                // _ = tokio::time::sleep(tokio::time::Duration::from_secs(30)) => {
                //    info!("Periodic reconciliation triggered.");
                //    if let Err(e) = self.reconcile_all_workloads().await {
                //        error!("Failed during periodic reconciliation: {:?}", e);
                //    }
                // }
                else => {
                    warn!("A channel closed or select! branch completed unexpectedly. Orchestrator might be shutting down.");
                    break;
                }
            }
        }
        info!("Orchestrator shutting down.");
        Ok(())
    }

    async fn handle_workload_update(&self, workload_def: WorkloadDefinition) -> Result<()> {
        let workload_id = workload_def.id;
        let workload_def_arc = Arc::new(workload_def);
        {
            let mut state = self.state.lock().await;
            state.workloads.insert(workload_id, workload_def_arc.clone());
            state.instances.entry(workload_id).or_insert_with(Vec::new); // Ensure entry exists
        }
        info!("Workload {} registered. Triggering reconciliation.", workload_id);
        self.reconcile_workload(&workload_def_arc).await
    }
    
    async fn handle_cluster_event(&self, event: ClusterEvent) -> Result<()> {
        let mut state = self.state.lock().await;
        match event {
            ClusterEvent::NodeAdded(node) | ClusterEvent::NodeUpdated(node) => {
                info!("Node {} added/updated.", node.id);
                // Initialize the runtime on the new node if it's newly added and ready
                if node.status == orchestrator_shared_types::NodeStatus::Ready && !state.nodes.contains_key(&node.id) {
                     match self.runtime.init_node(node.id).await {
                        Ok(_) => info!("Runtime initialized on node {}", node.id),
                        Err(e) => error!("Failed to initialize runtime on node {}: {:?}", node.id, e),
                     }
                }
                state.nodes.insert(node.id, node);
            }
            ClusterEvent::NodeRemoved(node_id) => {
                info!("Node {} removed.", node_id);
                state.nodes.remove(&node_id);
                // TODO: Handle workloads running on the removed node (reschedule them)
            }
        }
        Ok(())
    }

    async fn reconcile_all_workloads(&self) -> Result<()> {
        info!("Reconciling all workloads...");
        let workloads_to_reconcile = {
            let state = self.state.lock().await;
            state.workloads.values().cloned().collect::<Vec<_>>()
        };

        for workload_def in workloads_to_reconcile {
            if let Err(e) = self.reconcile_workload(&workload_def).await {
                error!("Failed to reconcile workload {}: {:?}", workload_def.id, e);
                // Continue to next workload
            }
        }
        info!("Finished reconciling all workloads.");
        Ok(())
    }


    async fn reconcile_workload(&self, workload_def: &Arc<WorkloadDefinition>) -> Result<()> {
        info!("Reconciling workload: {} ({})", workload_def.name, workload_def.id);
        let mut state = self.state.lock().await;

        let current_instances = state.instances.entry(workload_def.id).or_default();
        let desired_replicas = workload_def.replicas;
        let current_running_replicas = current_instances
            .iter()
            .filter(|inst| inst.status == WorkloadInstanceStatus::Running || inst.status == WorkloadInstanceStatus::Pending)
            .count() as u32;

        info!("Workload {}: Desired replicas: {}, Current running/pending: {}", workload_def.id, desired_replicas, current_running_replicas);

        if current_running_replicas < desired_replicas {
            let num_to_schedule = desired_replicas - current_running_replicas;
            info!("Need to schedule {} new instances for workload {}", num_to_schedule, workload_def.id);

            let available_nodes: Vec<Node> = state.nodes.values()
                .filter(|n| n.status == orchestrator_shared_types::NodeStatus::Ready) // Only schedule on ready nodes
                .cloned()
                .collect();
            
            if available_nodes.is_empty() {
                warn!("No ready nodes available to schedule workload {}", workload_def.id);
                return Ok(()); // Can't schedule if no nodes are ready
            }

            let schedule_request = ScheduleRequest {
                workload_definition: Arc::clone(workload_def),
                current_instances: current_instances.clone(),
            };
            
            // Drop lock before calling scheduler to avoid holding it too long
            drop(state); 
            let decisions = self.scheduler.schedule(&schedule_request, &available_nodes).await?;
            // Re-acquire lock to update state
            let mut state = self.state.lock().await;
            let current_instances = state.instances.entry(workload_def.id).or_default();


            for decision in decisions.into_iter().take(num_to_schedule as usize) {
                match decision {
                    ScheduleDecision::AssignNode(node_id) => {
                        info!("Scheduling new instance of workload {} on node {}", workload_def.id, node_id);
                        // This is where you'd iterate through `workload_def.containers`
                        // and call `self.runtime.create_container` for each.
                        // For simplicity, we'll just create a placeholder instance.
                        // Actual container creation needs more detail (passing container configs, options).
                        
                        // Placeholder: Assume first container is the main one for now
                        if let Some(container_config) = workload_def.containers.first() {
                            let options = container_runtime_interface::CreateContainerOptions {
                                workload_id: workload_def.id,
                                node_id,
                            };
                            // Drop lock before potentially long-running I/O (runtime call)
                            drop(state);
                            match self.runtime.create_container(container_config, &options).await {
                                Ok(container_id) => {
                                    info!("Container {} created for workload {} on node {}", container_id, workload_def.id, node_id);
                                    // Re-acquire lock
                                    state = self.state.lock().await;
                                    let current_instances = state.instances.entry(workload_def.id).or_default();
                                    
                                    let new_instance = WorkloadInstance {
                                        id: Uuid::new_v4(),
                                        workload_id: workload_def.id,
                                        node_id,
                                        container_ids: vec![container_id], // Store the actual container ID
                                        status: WorkloadInstanceStatus::Pending, // Will update based on runtime status later
                                    };
                                    current_instances.push(new_instance);
                                }
                                Err(e) => {
                                    error!("Failed to create container for workload {} on node {}: {:?}", workload_def.id, node_id, e);
                                    // Re-acquire lock if dropped and error occurred
                                    state = self.state.lock().await; 
                                }
                            }
                        } else {
                            warn!("Workload {} has no container definitions, cannot schedule.", workload_def.id);
                        }
                    }
                    ScheduleDecision::NoPlacement(reason) => {
                        warn!("Could not place instance of workload {}: {}", workload_def.id, reason);
                    }
                    ScheduleDecision::Error(err_msg) => {
                        error!("Scheduler error for workload {}: {}", workload_def.id, err_msg);
                    }
                }
            }

        } else if current_running_replicas > desired_replicas {
            let num_to_remove = current_running_replicas - desired_replicas;
            info!("Need to remove {} instances of workload {}", num_to_remove, workload_def.id);
            // TODO: Implement logic to select and remove surplus instances.
            // This would involve:
            // 1. Selecting which instances to terminate (e.g., oldest, least healthy).
            // 2. Iterating through their `container_ids`.
            // 3. Calling `self.runtime.stop_container()` and `self.runtime.remove_container()`.
            // 4. Updating the `WorkloadInstanceStatus` to Terminating, then removing from `state.instances`.
            warn!("Workload scale-down (removing {} instances) not yet implemented for workload {}", num_to_remove, workload_def.id);
        }

        // TODO: Implement status checking for existing instances.
        // Iterate through `current_instances`, call `self.runtime.get_container_status()` for each container,
        // and update `WorkloadInstanceStatus` accordingly. If an instance failed, it might need rescheduling.
        
        Ok(())
    }
}

// This would typically be in a `main.rs` file if `orchestrator_core` was a binary crate.
// For now, let's imagine a function that sets it up.
pub async fn start_orchestrator_service(
    runtime: Arc<dyn ContainerRuntime>,
    cluster_manager: Arc<dyn ClusterManager>,
    scheduler: Arc<dyn Scheduler>,
) -> Result<mpsc::Sender<WorkloadDefinition>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env().add_directive("info".parse().unwrap()))
        .init();

    let mut orchestrator = Orchestrator::new(runtime, cluster_manager, scheduler);
    let workload_tx = orchestrator.get_workload_sender();

    tokio::spawn(async move {
        if let Err(e) = orchestrator.run().await {
            error!("Orchestrator service exited with error: {:?}", e);
        }
    });

    Ok(workload_tx)
}
```
**Key points for `orchestrator_core/src/lib.rs`:**
*   It holds references (via `Arc<dyn Trait>`) to the abstracted components.
*   It has a main `run` loop that listens to events (new workloads, cluster changes).
*   `reconcile_workload` is the heart of ensuring the desired state matches the actual state.
*   The state is currently in-memory (`OrchestratorState`). This is a major simplification and would need to be backed by a distributed, consistent store in a real system (e.g., etcd, TiKV, or a custom Raft/Paxos implementation).
*   Error handling is basic; more robust error recovery and retry mechanisms would be needed.
*   Container creation is simplified; actual interaction with a runtime involves more complex OCI spec generation.

---

**Step 6: Next Steps and Considerations**

1.  **Implementations of Interfaces:**
    *   **Mock Implementations:** Create mock implementations for `ContainerRuntime`, `ClusterManager`, and `Scheduler` for testing and early development.
        *   The `SimpleScheduler` is a first step.
        *   `MockContainerRuntime`: Could just store container state in a `HashMap` and print actions.
        *   `MockClusterManager`: Could simulate node additions/removals via a simple API or timed events, and use a `watch` channel to send `ClusterEvent`s.
    *   **Real Implementations (Longer Term):**
        *   `YoukiRuntime`: An actual implementation that calls the Youki binary or uses its libraries if available. This is a significant piece of work.
        *   `GossipClusterManager`: Implement cluster membership using a library like `memberlist-rs` (if a Rust equivalent exists or by porting concepts) or by building a SWIM-like protocol.
        *   `AdvancedScheduler`: Implement more sophisticated scheduling algorithms (resource-based, affinity/anti-affinity, etc.).

2.  **State Management:**
    *   The current in-memory `OrchestratorState` is not durable or distributed. This is a critical component.
    *   Investigate Rust crates for interacting with etcd (`etcd-client`), Consul, or even TiKV (`tikv-client-rust`).
    *   Alternatively, for a fully Rust-native approach, implementing or integrating a Raft/Paxos library would be necessary for strong consistency. This is a very complex task.

3.  **API Layer (`main.rs` or separate `orchestrator_api` crate):**
    *   Create a binary crate (e.g., `orchestrator_daemon` or `src/main.rs` in `orchestrator_core`) that instantiates the `Orchestrator` and its components.
    *   This binary would also expose an API (initially simple HTTP, eventually MCP) for users/AI agents to submit workload definitions, query status, etc. Crates like `axum` or `actix-web` would be suitable here.

4.  **Testing:**
    *   Write unit tests for individual components and logic.
    *   Develop integration tests that spin up a mock orchestrator and verify its behavior.

5.  **Configuration:**
    *   Implement a way to configure the orchestrator (e.g., paths to runtime binaries, cluster join addresses, state store endpoints). Libraries like `config-rs` can be helpful.

6.  **Networking (CNI):**
    *   The current abstraction doesn't delve into container networking. This would involve CNI plugin interactions, managed by the `ContainerRuntime` implementation or a dedicated networking component.

**To run what we have (conceptually):**
You'd need a `main.rs` somewhere (e.g., create `orchestrator_core/src/main.rs` and change `lib.rs` to `mod core_logic; pub use core_logic::*;` or similar, then adjust `Cargo.toml` for `orchestrator_core` to define `[[bin]]` and `[lib]`).

Example `orchestrator_core/src/main.rs` (very basic, needs mock implementations):
```rust
use std::sync::Arc;
use tokio;

// Assuming you have mock implementations in these modules or crates
// For example:
// mod mock_runtime; use mock_runtime::MockRuntime;
// mod mock_cluster_manager; use mock_cluster_manager::MockClusterManager;

use orchestrator_core::start_orchestrator_service; // if in lib.rs
use orchestrator_shared_types::{NodeId, WorkloadDefinition, ContainerConfig, NodeResources, PortMapping};
use scheduler_interface::SimpleScheduler;
use uuid::Uuid;
use std::collections::HashMap;

// --- Mock Implementations (simplified, put these in their respective interface crates or a mock crate) ---
use async_trait::async_trait;
use orchestrator_shared_types::{Node, Result, OrchestrationError, ContainerId};
use container_runtime_interface::{ContainerRuntime, CreateContainerOptions, ContainerStatus};
use cluster_manager_interface::{ClusterManager, ClusterEvent};
use tokio::sync::watch;

#[derive(Default, Clone)]
struct MockRuntime {
    containers: Arc<tokio::sync::Mutex<HashMap<ContainerId, (ContainerConfig, ContainerStatus)>>>,
}

#[async_trait]
impl ContainerRuntime for MockRuntime {
    async fn init_node(&self, _node_id: NodeId) -> Result<()> { Ok(()) }
    async fn create_container(&self, config: &ContainerConfig, _options: &CreateContainerOptions) -> Result<ContainerId> {
        let id = Uuid::new_v4().to_string();
        let status = ContainerStatus { id: id.clone(), state: "Pending".to_string(), exit_code: None, error_message: None };
        self.containers.lock().await.insert(id.clone(), (config.clone(), status));
        tracing::info!("[MockRuntime] Created container {}", id);
        // Simulate it starting after a bit
        let containers_clone = self.containers.clone();
        let id_clone = id.clone();
        tokio::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
            let mut locked_containers = containers_clone.lock().await;
            if let Some((_cfg, status)) = locked_containers.get_mut(&id_clone) {
                status.state = "Running".to_string();
                tracing::info!("[MockRuntime] Container {} is now Running", id_clone);
            }
        });
        Ok(id)
    }
    async fn stop_container(&self, container_id: &ContainerId) -> Result<()> {
        if let Some((_cfg, status)) = self.containers.lock().await.get_mut(container_id) {
            status.state = "Stopped".to_string();
            status.exit_code = Some(0);
            tracing::info!("[MockRuntime] Stopped container {}", container_id);
            Ok(())
        } else {
            Err(OrchestrationError::RuntimeError(format!("Container {} not found", container_id)))
        }
    }
    async fn remove_container(&self, container_id: &ContainerId) -> Result<()> {
        if self.containers.lock().await.remove(container_id).is_some() {
            tracing::info!("[MockRuntime] Removed container {}", container_id);
            Ok(())
        } else {
            Err(OrchestrationError::RuntimeError(format!("Container {} not found for removal", container_id)))
        }
    }
    async fn get_container_status(&self, container_id: &ContainerId) -> Result<ContainerStatus> {
        self.containers.lock().await.get(container_id)
            .map(|(_cfg, status)| status.clone())
            .ok_or_else(|| OrchestrationError::RuntimeError(format!("Container {} status not found", container_id)))
    }
    async fn list_containers(&self, _node_id: NodeId) -> Result<Vec<ContainerStatus>> {
        Ok(self.containers.lock().await.values().map(|(_cfg, status)| status.clone()).collect())
    }
}

struct MockClusterManager {
    event_tx: watch::Sender<Option<ClusterEvent>>,
    nodes: Arc<tokio::sync::Mutex<HashMap<NodeId, Node>>>,
}
impl MockClusterManager {
    fn new() -> (Self, watch::Receiver<Option<ClusterEvent>>) {
        let (tx, rx) = watch::channel(None);
        (MockClusterManager { event_tx: tx, nodes: Arc::new(tokio::sync::Mutex::new(HashMap::new())) }, rx)
    }
    async fn add_node(&self, node: Node) {
        self.nodes.lock().await.insert(node.id, node.clone());
        self.event_tx.send(Some(ClusterEvent::NodeAdded(node))).ok();
    }
}

#[async_trait]
impl ClusterManager for MockClusterManager {
    async fn initialize(&self) -> Result<()> { Ok(()) }
    async fn get_node(&self, node_id: &NodeId) -> Result<Option<Node>> {
        Ok(self.nodes.lock().await.get(node_id).cloned())
    }
    async fn list_nodes(&self) -> Result<Vec<Node>> {
        Ok(self.nodes.lock().await.values().cloned().collect())
    }
    async fn subscribe_to_events(&self) -> Result<watch::Receiver<Option<ClusterEvent>>> {
        Ok(self.event_tx.subscribe())
    }
}
// --- End Mock Implementations ---

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let runtime = Arc::new(MockRuntime::default());
    let (cluster_manager_impl, _cluster_event_rx_for_test) = MockClusterManager::new();
    let cluster_manager: Arc<dyn ClusterManager> = Arc::new(cluster_manager_impl);
    let scheduler = Arc::new(SimpleScheduler);

    // Start the orchestrator service (it runs in a spawned task)
    let workload_tx = start_orchestrator_service(runtime.clone(), cluster_manager.clone(), scheduler.clone()).await?;
    tracing::info!("Orchestrator service started in background.");

    // Simulate adding a node to the cluster after a delay
    let mock_cm_accessor = cluster_manager.clone(); // To access add_node if it were public on the trait or specific type
    tokio::spawn(async move {
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        let node1_id = Uuid::new_v4();
        let node1 = Node {
            id: node1_id,
            address: "10.0.0.1:1234".to_string(),
            status: orchestrator_shared_types::NodeStatus::Ready,
            labels: Default::default(),
            resources_capacity: NodeResources { cpu_cores: 4.0, memory_mb: 8192, disk_mb: 100000 },
            resources_allocatable: NodeResources { cpu_cores: 3.8, memory_mb: 7000, disk_mb: 90000 },
        };
        tracing::info!("Simulating add_node: {}", node1_id);
        // This downcast is a bit of a hack for testing, ideally you'd have a way to interact with the mock
        if let Some(mock_cm) = mock_cm_accessor.downcast_arc::<MockClusterManager>().ok() {
             mock_cm.add_node(node1).await;
        } else {
            tracing::error!("Failed to downcast to MockClusterManager to add node");
        }
    });


    // Simulate submitting a workload after a few seconds (to allow node to be added)
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
    let workload_def = WorkloadDefinition {
        id: Uuid::new_v4(),
        name: "my-nginx-service".to_string(),
        containers: vec![ContainerConfig {
            name: "nginx".to_string(),
            image: "nginx:latest".to_string(),
            command: None,
            args: None,
            env_vars: Default::default(),
            ports: vec![PortMapping { container_port: 80, host_port: Some(8080), protocol: "tcp".to_string() }],
            resource_requests: NodeResources { cpu_cores: 0.5, memory_mb: 256, disk_mb: 0 },
        }],
        replicas: 2,
        labels: Default::default(),
    };
    tracing::info!("Submitting workload: {}", workload_def.name);
    workload_tx.send(workload_def.clone()).await?;


    // Keep main alive for a bit to see logs, or until Ctrl+C
    tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
    tracing::info!("Test period finished.");

    Ok(())
}
```
To make the `main.rs` above work, you'd adjust `orchestrator_core/Cargo.toml`:
```toml
[package]
name = "orchestrator_core"
version = "0.1.0"
edition = "2021"

# [[bin]] # Add this section if you have a main.rs
# name = "orchestrator_daemon"
# path = "src/main.rs"

[dependencies]
# ... (existing dependencies)
anyhow = "1.0" # For easy error handling in main
uuid = { workspace = true } # For generating UUIDs in main for test
async-trait = { workspace = true } # For mock impls

# You might need to re-specify these if mocks are in the same crate and not separate
orchestrator_shared_types = { path = "../orchestrator_shared_types" }
container_runtime_interface = { path = "../container_runtime_interface" }
cluster_manager_interface = { path = "../cluster_manager_interface" }
scheduler_interface = { path = "../scheduler_interface" }
```
And also add `downcast_rs = "0.2"` to the `[workspace.dependencies]` and `orchestrator_core` dependencies if you want to use `downcast_arc`. The `ClusterManager` trait would need to inherit from `downcast_rs::DowncastSync`. This is getting a bit complex for a simple mock setup, often mocks are simpler or injected directly as their concrete types in tests.

For now, the `lib.rs` of `orchestrator_core` containing `start_orchestrator_service` is a good library structure. The `main.rs` example is just to show how one might tie it together.

This is a solid foundation for Phase 1. We've defined the core abstractions and a rudimentary orchestrator loop. The next steps would involve filling in these abstractions with mock or simple implementations, then gradually replacing them with more robust ones. This iterative approach aligns well with the phased development plan.
