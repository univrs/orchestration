# RustOrchestration Platform - Work Session 2025-10-21

## Executive Summary

RustOrchestration is an AI-Native Container Orchestration Platform built entirely in Rust, designed as a modern Kubernetes alternative. This document provides a comprehensive architectural overview of the Phase 1 implementation and roadmap for Phase 2 development.

**Current Status**: Phase 1 (Core Orchestration Primitives) - 60-70% Complete
**Next Phase**: CLI + API Server + Dockerfiles for production deployment

---

## Table of Contents

1. [Architecture Overview](#architecture-overview)
2. [Component Architecture](#component-architecture)
3. [Data Models](#data-models)
4. [Current Implementation Status](#current-implementation-status)
5. [System Workflows](#system-workflows)
6. [Phase 2 Planning](#phase-2-planning)
7. [Technical Stack](#technical-stack)
8. [Critical Gaps & TODOs](#critical-gaps--todos)

---

## Architecture Overview

### High-Level System Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                        PHASE 2 (TO BUILD)                            │
│  ┌──────────────┐         ┌─────────────────┐                       │
│  │  CLI Client  │────────▶│   API Server    │                       │
│  │  (clap-rs)   │  HTTP/  │   (axum/tonic)  │                       │
│  └──────────────┘  gRPC   └────────┬────────┘                       │
│                                     │                                 │
└─────────────────────────────────────┼─────────────────────────────────┘
                                      │
                                      │ In-Process or IPC
                                      ▼
┌─────────────────────────────────────────────────────────────────────┐
│                    PHASE 1 (CURRENT - 60-70%)                        │
│                                                                       │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │              Orchestrator Core (Main Loop)                   │   │
│  │  ┌────────────────────────────────────────────────────┐     │   │
│  │  │ tokio::select! {                                    │     │   │
│  │  │   workload_rx.recv() => reconcile_workload()       │     │   │
│  │  │   cluster_events => handle_cluster_event()         │     │   │
│  │  │   timer.tick() => periodic_reconciliation()  [TODO]│     │   │
│  │  │ }                                                   │     │   │
│  │  └────────────────────────────────────────────────────┘     │   │
│  │                                                               │   │
│  │  ┌──────────────────────────────────────────────────┐       │   │
│  │  │        OrchestratorState (IN-MEMORY)             │       │   │
│  │  │  • nodes: HashMap<NodeId, Node>                  │       │   │
│  │  │  • workloads: HashMap<WorkloadId, WorkloadDef>   │       │   │
│  │  │  • instances: HashMap<InstanceId, Instance>      │       │   │
│  │  │  ⚠️  CRITICAL: Not persistent/distributed         │       │   │
│  │  └──────────────────────────────────────────────────┘       │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                                                                       │
│  ┌───────────────┐  ┌──────────────┐  ┌─────────────────┐          │
│  │  Container    │  │   Cluster    │  │   Scheduler     │          │
│  │  Runtime      │  │   Manager    │  │   Interface     │          │
│  │  Interface    │  │   Interface  │  │                 │          │
│  │               │  │              │  │  ┌───────────┐  │          │
│  │  Trait-Based  │  │  Trait-Based │  │  │  Simple   │  │          │
│  │  Abstraction  │  │  Abstraction │  │  │ Round-    │  │          │
│  │               │  │              │  │  │  Robin    │  │          │
│  │  [Mock Impl]  │  │  [Mock Impl] │  │  └───────────┘  │          │
│  └───────┬───────┘  └──────┬───────┘  └─────────────────┘          │
│          │                 │                                         │
└──────────┼─────────────────┼─────────────────────────────────────────┘
           │                 │
           ▼                 ▼
┌──────────────────────────────────────────────────────────────────┐
│                    TARGET IMPLEMENTATIONS                         │
│  ┌─────────────┐      ┌──────────────────┐                       │
│  │    Youki    │      │  Gossip Protocol │                       │
│  │  (OCI Rust  │      │   (memberlist)   │                       │
│  │   Runtime)  │      │                  │                       │
│  └─────────────┘      └──────────────────┘                       │
│         │                      │                                  │
│         ▼                      ▼                                  │
│  ┌──────────────────────────────────────┐                        │
│  │      Persistent State Store          │                        │
│  │    (etcd / Raft / Consul)            │                        │
│  │         ⚠️  NOT IMPLEMENTED          │                        │
│  └──────────────────────────────────────┘                        │
└──────────────────────────────────────────────────────────────────┘
```

### Workspace Structure

```
RustOrchestration/
└── ai_native_orchestrator/          # Cargo Workspace Root
    ├── Cargo.toml                    # Workspace manifest
    ├── Cargo.lock
    │
    ├── orchestrator_shared_types/    # Common data structures
    │   ├── Cargo.toml
    │   └── src/
    │       └── lib.rs                # NodeId, WorkloadDefinition, Node, etc.
    │
    ├── container_runtime_interface/  # OCI runtime abstraction
    │   ├── Cargo.toml
    │   └── src/
    │       └── lib.rs                # ContainerRuntime trait
    │
    ├── cluster_manager_interface/    # Cluster membership abstraction
    │   ├── Cargo.toml
    │   └── src/
    │       └── lib.rs                # ClusterManager trait
    │
    ├── scheduler_interface/          # Workload placement abstraction
    │   ├── Cargo.toml
    │   └── src/
    │       └── lib.rs                # Scheduler trait
    │
    ├── orchestrator_core/            # Main orchestration engine
    │   ├── Cargo.toml
    │   ├── src/
    │   │   ├── lib.rs                # Orchestrator struct + reconciliation
    │   │   └── main.rs               # Binary with mock implementations
    │   └── tests/
    │
    └── docs/
        └── README.md                 # Implementation guidance
```

---

## Component Architecture

### 1. Orchestrator Shared Types

**Purpose**: Canonical data structures shared across all components

**Key Types**:

```rust
// Identifiers
pub type NodeId = String;           // Unique node identifier
pub type WorkloadId = String;       // Unique workload identifier
pub type ContainerId = String;      // OCI container ID
pub type WorkloadInstanceId = String;

// Node Representation
pub struct Node {
    pub id: NodeId,
    pub name: String,
    pub status: NodeStatus,         // Ready, NotReady, Unknown, Down
    pub capacity: NodeResources,
    pub labels: HashMap<String, String>,
}

pub struct NodeResources {
    pub cpu_cores: u32,
    pub memory_mb: u64,
    pub disk_mb: u64,
}

// Container Configuration
pub struct ContainerConfig {
    pub image: String,
    pub env: HashMap<String, String>,
    pub port_mappings: Vec<PortMapping>,
    pub resource_requests: Option<ResourceRequirements>,
}

// Workload Definition (Desired State)
pub struct WorkloadDefinition {
    pub workload_id: WorkloadId,
    pub name: String,
    pub containers: Vec<ContainerConfig>,
    pub replicas: u32,
    pub labels: HashMap<String, String>,
}

// Workload Instance (Actual Running State)
pub struct WorkloadInstance {
    pub instance_id: WorkloadInstanceId,
    pub workload_id: WorkloadId,
    pub node_id: NodeId,
    pub status: WorkloadInstanceStatus,  // Pending, Running, Succeeded, Failed, Terminating
    pub containers: Vec<ContainerStatus>,
}

// Error Handling
pub enum OrchestrationError {
    RuntimeError(String),
    SchedulingError(String),
    ClusterError(String),
    StateError(String),
    InvalidInput(String),
    NotFound(String),
}
```

**Dependencies**:
- `serde` for serialization
- `uuid` for ID generation
- `thiserror` for error handling

---

### 2. Container Runtime Interface

**Purpose**: Abstract OCI runtime operations (Youki, runc, containerd)

**Trait Definition**:

```rust
#[async_trait]
pub trait ContainerRuntime: Send + Sync {
    // Initialize runtime on a specific node
    async fn init_node(&self, node_id: NodeId) -> Result<()>;

    // Create and start a container
    async fn create_container(
        &self,
        node_id: &NodeId,
        container_id: &ContainerId,
        config: &ContainerConfig,
    ) -> Result<ContainerId>;

    // Stop a running container
    async fn stop_container(
        &self,
        node_id: &NodeId,
        container_id: &ContainerId,
    ) -> Result<()>;

    // Remove a container
    async fn remove_container(
        &self,
        node_id: &NodeId,
        container_id: &ContainerId,
    ) -> Result<()>;

    // Get container status
    async fn get_container_status(
        &self,
        node_id: &NodeId,
        container_id: &ContainerId,
    ) -> Result<ContainerStatus>;

    // List all containers on a node
    async fn list_containers(
        &self,
        node_id: &NodeId,
    ) -> Result<Vec<ContainerStatus>>;
}
```

**Current Implementation**: Mock runtime for testing
**Target Implementation**: Youki (Rust OCI runtime)

**Key Considerations**:
- Node-aware: Each operation scoped to a specific node
- OCI-compliant: Must generate valid OCI runtime specs
- Status tracking: Maps container states to WorkloadInstanceStatus
- Resource isolation: cgroups, namespaces via OCI runtime

---

### 3. Cluster Manager Interface

**Purpose**: Distributed cluster membership and node lifecycle

**Trait Definition**:

```rust
#[async_trait]
pub trait ClusterManager: Send + Sync {
    // Initialize cluster manager
    async fn initialize(&self) -> Result<()>;

    // Get node by ID
    async fn get_node(&self, node_id: &NodeId) -> Result<Option<Node>>;

    // List all nodes in cluster
    async fn list_nodes(&self) -> Result<Vec<Node>>;

    // Subscribe to cluster events
    async fn subscribe_to_events(&self)
        -> Result<watch::Receiver<Option<ClusterEvent>>>;
}

pub enum ClusterEvent {
    NodeAdded(Node),
    NodeRemoved(NodeId),
    NodeUpdated(Node),
}
```

**Current Implementation**: Static mock with 2 nodes
**Target Implementation**: Gossip protocol (memberlist-inspired)

**Responsibilities**:
- Node discovery and registration
- Health checking and failure detection
- Event propagation to orchestrator
- Leader election (future)
- Distributed consensus (future)

---

### 4. Scheduler Interface

**Purpose**: Decide workload placement across cluster nodes

**Trait Definition**:

```rust
#[async_trait]
pub trait Scheduler: Send + Sync {
    async fn schedule(
        &self,
        request: &ScheduleRequest,
        available_nodes: &[Node],
    ) -> Result<Vec<ScheduleDecision>>;
}

pub struct ScheduleRequest {
    pub workload: WorkloadDefinition,
    pub current_instances: Vec<WorkloadInstance>,
}

pub enum ScheduleDecision {
    AssignNode(NodeId),         // Place replica on this node
    NoPlacement(String),        // Cannot place (reason)
    Error(String),              // Scheduling error
}
```

**Current Implementation**: SimpleScheduler (round-robin)
**Target Implementations**:
- Resource-aware scheduler (bin packing)
- Affinity/anti-affinity scheduler
- Topology-aware scheduler
- Custom policy schedulers

**Scheduling Algorithm Evolution**:
```
Phase 1: Round-robin (current)
         └─> Simple, predictable, no resource awareness

Phase 2: Resource-aware
         └─> CPU/memory validation
         └─> Best-fit bin packing

Phase 3: Advanced
         └─> Affinity/anti-affinity rules
         └─> Taints and tolerations
         └─> Node selectors
         └─> Pod topology spread
```

---

### 5. Orchestrator Core

**Purpose**: Main orchestration engine implementing the reconciliation loop

**Core Struct**:

```rust
pub struct Orchestrator {
    runtime: Arc<dyn ContainerRuntime>,
    cluster_manager: Arc<dyn ClusterManager>,
    scheduler: Arc<dyn Scheduler>,
    state: Arc<Mutex<OrchestratorState>>,
    workload_tx: mpsc::Sender<WorkloadDefinition>,
    workload_rx: Arc<Mutex<mpsc::Receiver<WorkloadDefinition>>>,
}

struct OrchestratorState {
    nodes: HashMap<NodeId, Node>,
    workloads: HashMap<WorkloadId, WorkloadDefinition>,
    instances: HashMap<WorkloadInstanceId, WorkloadInstance>,
}
```

**Main Event Loop**:

```rust
pub async fn run(&self) -> Result<()> {
    // Subscribe to cluster events
    let mut cluster_events = self.cluster_manager
        .subscribe_to_events().await?;

    loop {
        tokio::select! {
            // New workload submitted
            Some(workload) = self.workload_rx.lock().await.recv() => {
                self.register_workload(workload).await?;
                self.reconcile_workload(&workload.workload_id).await?;
            }

            // Cluster topology changed
            Some(Some(event)) = cluster_events.recv() => {
                self.handle_cluster_event(event).await?;
            }

            // TODO: Periodic reconciliation timer
            // _ = interval.tick() => {
            //     self.reconcile_all_workloads().await?;
            // }
        }
    }
}
```

**Reconciliation Pattern**:

The core of Kubernetes-style orchestration:

```rust
async fn reconcile_workload(&self, workload_id: &WorkloadId) -> Result<()> {
    // 1. READ PHASE: Determine desired vs actual state
    let (desired, actual, action) = {
        let state = self.state.lock().await;
        let workload = state.workloads.get(workload_id)?;
        let current_instances: Vec<_> = state.instances.values()
            .filter(|i| i.workload_id == *workload_id)
            .cloned()
            .collect();

        let desired_replicas = workload.replicas as usize;
        let actual_replicas = current_instances.len();

        let action = if actual_replicas < desired_replicas {
            WorkloadAction::ScheduleNew(desired_replicas - actual_replicas)
        } else if actual_replicas > desired_replicas {
            WorkloadAction::RemoveInstances(actual_replicas - desired_replicas)
        } else {
            WorkloadAction::None
        };

        (workload.clone(), current_instances, action)
    }; // Lock released here

    // 2. EXECUTE PHASE: Perform actions outside lock
    match action {
        WorkloadAction::ScheduleNew(count) => {
            for _ in 0..count {
                self.schedule_and_create_instance(&desired).await?;
            }
        }
        WorkloadAction::RemoveInstances(count) => {
            // TODO: Implement scale-down logic
        }
        WorkloadAction::None => {
            // Already at desired state
        }
    }

    Ok(())
}
```

**Key Methods**:

- `register_workload()`: Add workload to state, trigger reconciliation
- `reconcile_workload()`: Ensure actual state matches desired state
- `schedule_and_create_instance()`: Place instance on node, create containers
- `handle_cluster_event()`: React to node changes, re-schedule if needed

**State Management Pattern**:
```
Lock → Read → Release → Execute → Lock → Update → Release
  │                         │
  └─ Minimize contention    └─ Expensive I/O outside lock
```

---

## Data Models

### State Transitions

**Node Lifecycle**:
```
  Discovered
      │
      ▼
  Not Ready ──────┐
      │           │
      │ (health   │ (health check fails)
      │  passes)  │
      ▼           ▼
    Ready ───▶  Down
      │           │
      │           │ (recovery)
      └───────────┘
```

**Workload Instance Lifecycle**:
```
  Submitted
      │
      ▼
   Pending ────────┐
      │            │ (scheduling fails)
      │ (node      │
      │  assigned) │
      ▼            ▼
   Running      Failed
      │
      │ (terminated)
      ▼
  Succeeded / Failed / Terminating
```

**Container Status Flow**:
```
Created → Starting → Running → Stopped
   │                    │
   │                    ▼
   └───────────────▶ Error
```

### Resource Model

```rust
// Node capacity tracking
Node {
    capacity: {
        cpu_cores: 8,
        memory_mb: 16384,
        disk_mb: 102400
    }
}

// Container resource requests
ContainerConfig {
    resource_requests: Some(ResourceRequirements {
        cpu_millicores: 500,    // 0.5 CPU cores
        memory_mb: 512,
        disk_mb: 1024
    })
}

// Scheduling validation (TODO):
// Sum(instance.resources) <= node.capacity
```

---

## Current Implementation Status

### ✅ Implemented (60-70%)

| Component | Status | Implementation |
|-----------|--------|----------------|
| Shared Types | ✅ Complete | All core data structures defined |
| Container Runtime Interface | ✅ Complete | Trait defined, mock impl |
| Cluster Manager Interface | ✅ Complete | Trait defined, mock impl |
| Scheduler Interface | ✅ Complete | Trait + SimpleScheduler |
| Orchestrator Core | ⚠️ Partial | Main loop + reconciliation working |
| Error Handling | ✅ Complete | Comprehensive error types |
| Async Architecture | ✅ Complete | Tokio-based event loop |
| Workspace Structure | ✅ Complete | Proper crate organization |

### ⚠️ Partial / In Progress

| Feature | Status | Notes |
|---------|--------|-------|
| State Management | ⚠️ In-memory only | CRITICAL: Needs persistent store |
| Reconciliation | ⚠️ Scale-up only | Missing scale-down logic |
| Container Monitoring | ⚠️ Creation only | No status polling |
| Cluster Events | ⚠️ Basic | No periodic reconciliation timer |

### ❌ Not Implemented (Critical Gaps)

| Feature | Priority | Phase |
|---------|----------|-------|
| Persistent State Store | 🔴 CRITICAL | Phase 1 |
| Youki Integration | 🔴 High | Phase 1 |
| Gossip Protocol | 🔴 High | Phase 1 |
| Container Status Monitoring | 🔴 High | Phase 1 |
| Scale-Down Logic | 🟡 Medium | Phase 1 |
| Resource Validation | 🟡 Medium | Phase 1 |
| API Server | 🔴 High | Phase 2 |
| CLI Client | 🔴 High | Phase 2 |
| Networking/CNI | 🟡 Medium | Phase 2 |
| Storage/Volumes | 🟡 Medium | Phase 2 |
| Health Checks | 🟡 Medium | Phase 2 |
| MCP Server | 🟢 Low | Phase 3 |
| AI Agents | 🟢 Low | Phase 4 |

---

## System Workflows

### Workload Submission Flow

```
┌─────────────┐
│   User      │ (Phase 2: CLI)
└──────┬──────┘
       │ Submit WorkloadDefinition
       ▼
┌─────────────────────────────────┐
│      API Server (Phase 2)       │
│  - Validation                   │
│  - Authentication (future)      │
└──────┬──────────────────────────┘
       │
       ▼
┌─────────────────────────────────┐
│    Orchestrator Core            │
│  1. register_workload()         │
│     └─> Add to state.workloads  │
│  2. reconcile_workload()        │
│     └─> Determine action        │
└──────┬──────────────────────────┘
       │
       ▼
┌─────────────────────────────────┐
│        Scheduler                │
│  - Select node for placement    │
│  - Return ScheduleDecision      │
└──────┬──────────────────────────┘
       │
       ▼
┌─────────────────────────────────┐
│    Container Runtime            │
│  1. create_container()          │
│     └─> Generate OCI spec       │
│     └─> Start container         │
│  2. Update instance status      │
└──────┬──────────────────────────┘
       │
       ▼
┌─────────────────────────────────┐
│     OrchestratorState           │
│  - Store WorkloadInstance       │
│  - Mark status: Running         │
└─────────────────────────────────┘
```

### Cluster Event Handling Flow

```
┌─────────────────────────────────┐
│      Cluster Manager            │
│  - Gossip protocol (future)     │
│  - Health checks                │
└──────┬──────────────────────────┘
       │ NodeAdded / NodeRemoved / NodeUpdated
       ▼
┌─────────────────────────────────┐
│  Orchestrator Core Event Loop   │
│  handle_cluster_event()         │
└──────┬──────────────────────────┘
       │
       ├─ NodeAdded
       │  └─> Add to state.nodes
       │  └─> Trigger reconciliation
       │
       ├─ NodeRemoved
       │  └─> Remove from state.nodes
       │  └─> Re-schedule instances
       │
       └─ NodeUpdated
          └─> Update node status
          └─> If node goes Down, re-schedule
```

### Reconciliation Loop (Detailed)

```
                    ┌───────────────────────────┐
                    │  Trigger Reconciliation   │
                    │  - Workload submitted     │
                    │  - Cluster event          │
                    │  - Periodic timer (TODO)  │
                    └─────────┬─────────────────┘
                              │
                              ▼
                    ┌───────────────────────────┐
                    │   READ PHASE              │
                    │  Lock state               │
                    │  ├─ Get workload def      │
                    │  ├─ Get current instances │
                    │  └─ Calculate delta       │
                    └─────────┬─────────────────┘
                              │
                              ▼
                    ┌───────────────────────────┐
                    │   DECIDE PHASE            │
                    │  Determine action:        │
                    │  ├─ ScheduleNew(n)        │
                    │  ├─ RemoveInstances(n)    │
                    │  └─ None                  │
                    └─────────┬─────────────────┘
                              │
                  ┌───────────┼───────────┐
                  │           │           │
        ScheduleNew(n)     Remove(n)    None
                  │           │           │
                  ▼           ▼           └─> Done
          ┌─────────────────────────┐
          │  EXECUTE PHASE          │
          │  (Lock released)        │
          │  for each replica:      │
          │    ├─ scheduler.schedule│
          │    ├─ runtime.create    │
          │    └─ Update state      │
          └─────────────────────────┘
```

---

## Phase 2 Planning

### Objective: Production-Ready Deployment

Build CLI, API Server, and containerized deployment for the orchestrator platform.

### Components to Build

#### 1. CLI Client (`orchestrator-cli`)

**Technology**: `clap` v4 (Rust command-line parser)

**Features**:
```bash
# Workload management
orchestrator-cli workload create -f workload.yaml
orchestrator-cli workload list
orchestrator-cli workload describe <id>
orchestrator-cli workload delete <id>
orchestrator-cli workload scale <id> --replicas=5

# Node management
orchestrator-cli node list
orchestrator-cli node describe <id>
orchestrator-cli node drain <id>

# Instance management
orchestrator-cli instance list --workload=<id>
orchestrator-cli instance logs <id>
orchestrator-cli instance exec <id> -- /bin/sh

# Cluster management
orchestrator-cli cluster info
orchestrator-cli cluster health
```

**Implementation**:
```rust
// New crate: orchestrator_cli/

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "orchestrator-cli")]
#[command(about = "RustOrchestration CLI Client")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    #[arg(long, default_value = "http://localhost:8080")]
    server: String,
}

#[derive(Subcommand)]
enum Commands {
    Workload {
        #[command(subcommand)]
        action: WorkloadCommands,
    },
    Node {
        #[command(subcommand)]
        action: NodeCommands,
    },
    // ...
}
```

**API Client**:
- HTTP client using `reqwest`
- JSON serialization via `serde_json`
- Pretty-printed output with `tabled`
- YAML support via `serde_yaml`

---

#### 2. API Server (`orchestrator-api`)

**Technology**:
- `axum` (ergonomic HTTP framework) or `tonic` (gRPC)
- `tower` middleware (auth, logging, tracing)

**Architecture**:

```
┌──────────────────────────────────────────────┐
│           API Server (Port 8080)             │
│  ┌────────────────────────────────────────┐  │
│  │   HTTP/gRPC Handlers                   │  │
│  │  - POST   /api/v1/workloads            │  │
│  │  - GET    /api/v1/workloads            │  │
│  │  - GET    /api/v1/workloads/:id        │  │
│  │  - DELETE /api/v1/workloads/:id        │  │
│  │  - PATCH  /api/v1/workloads/:id/scale  │  │
│  │  - GET    /api/v1/nodes                │  │
│  │  - GET    /api/v1/instances            │  │
│  └────────────┬───────────────────────────┘  │
│               │                               │
│  ┌────────────▼───────────────────────────┐  │
│  │   Business Logic Layer                 │  │
│  │  - Validation                          │  │
│  │  - Request mapping                     │  │
│  └────────────┬───────────────────────────┘  │
│               │                               │
│  ┌────────────▼───────────────────────────┐  │
│  │   Orchestrator Integration             │  │
│  │  - Arc<Orchestrator> shared state      │  │
│  │  - workload_tx.send()                  │  │
│  │  - state queries                       │  │
│  └────────────────────────────────────────┘  │
└──────────────────────────────────────────────┘
```

**Implementation Sketch**:

```rust
// New crate: orchestrator_api/

use axum::{
    Router,
    routing::{get, post, delete, patch},
    extract::{State, Path},
    Json,
};
use orchestrator_core::Orchestrator;

#[derive(Clone)]
struct AppState {
    orchestrator: Arc<Orchestrator>,
}

#[tokio::main]
async fn main() {
    // Initialize orchestrator
    let orchestrator = Arc::new(
        Orchestrator::new(runtime, cluster_mgr, scheduler)
    );

    // Start orchestrator in background
    let orch_clone = orchestrator.clone();
    tokio::spawn(async move {
        orch_clone.run().await
    });

    // Build API routes
    let app = Router::new()
        .route("/api/v1/workloads", post(create_workload))
        .route("/api/v1/workloads", get(list_workloads))
        .route("/api/v1/workloads/:id", get(get_workload))
        .route("/api/v1/workloads/:id", delete(delete_workload))
        .route("/api/v1/workloads/:id/scale", patch(scale_workload))
        .route("/api/v1/nodes", get(list_nodes))
        .route("/api/v1/instances", get(list_instances))
        .route("/health", get(health_check))
        .with_state(AppState { orchestrator });

    // Start server
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080")
        .await
        .unwrap();

    axum::serve(listener, app).await.unwrap();
}

// Handler example
async fn create_workload(
    State(state): State<AppState>,
    Json(workload): Json<WorkloadDefinition>,
) -> Result<Json<WorkloadDefinition>, StatusCode> {
    state.orchestrator.submit_workload(workload.clone()).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(workload))
}
```

**Middleware**:
- Request logging with `tracing`
- CORS headers
- Authentication (JWT - future)
- Rate limiting (future)

---

#### 3. Dockerfiles for Deployment

**Multi-stage build for efficiency**:

```dockerfile
# File: Dockerfile.orchestrator

# Stage 1: Builder
FROM rust:1.75-slim as builder

WORKDIR /build

# Copy manifests
COPY Cargo.toml Cargo.lock ./
COPY ai_native_orchestrator ./ai_native_orchestrator

# Build release binary
RUN cargo build --release --bin orchestrator-api

# Stage 2: Runtime
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Copy binary from builder
COPY --from=builder /build/target/release/orchestrator-api /usr/local/bin/

# Expose API port
EXPOSE 8080

# Run server
CMD ["orchestrator-api"]
```

**Docker Compose for local development**:

```yaml
# File: docker-compose.yml

version: '3.8'

services:
  orchestrator-api:
    build:
      context: .
      dockerfile: Dockerfile.orchestrator
    ports:
      - "8080:8080"
    environment:
      - RUST_LOG=info
      - ORCHESTRATOR_BIND_ADDR=0.0.0.0:8080
    volumes:
      - orchestrator-state:/var/lib/orchestrator
    networks:
      - orchestrator-net

  # Future: etcd for persistent state
  etcd:
    image: quay.io/coreos/etcd:v3.5.10
    ports:
      - "2379:2379"
    environment:
      - ETCD_ADVERTISE_CLIENT_URLS=http://etcd:2379
      - ETCD_LISTEN_CLIENT_URLS=http://0.0.0.0:2379
    networks:
      - orchestrator-net

networks:
  orchestrator-net:
    driver: bridge

volumes:
  orchestrator-state:
```

---

### Phase 2 Development Plan

#### Step 1: API Server Foundation (Week 1-2)

**Tasks**:
1. Create `orchestrator_api` crate
2. Set up axum server with basic routes
3. Implement handlers for workload CRUD
4. Add OpenAPI/Swagger documentation
5. Write integration tests

**Acceptance Criteria**:
- API server starts and binds to port 8080
- Can submit workload via POST /api/v1/workloads
- Can query workloads via GET endpoints
- Health check endpoint returns 200 OK

---

#### Step 2: CLI Client (Week 2-3)

**Tasks**:
1. Create `orchestrator_cli` crate
2. Implement command structure with clap
3. Build HTTP client for API communication
4. Add YAML workload file parsing
5. Implement pretty-printed output
6. Write CLI integration tests

**Acceptance Criteria**:
- `orchestrator-cli workload create -f test.yaml` succeeds
- `orchestrator-cli workload list` shows submitted workloads
- `orchestrator-cli node list` displays cluster nodes
- Error messages are clear and actionable

---

#### Step 3: Dockerization (Week 3)

**Tasks**:
1. Write multi-stage Dockerfile
2. Optimize build cache with layer ordering
3. Create docker-compose.yml for local dev
4. Add health check endpoint for container orchestration
5. Document deployment process
6. Test container build and run

**Acceptance Criteria**:
- `docker build -t orchestrator:latest .` succeeds
- `docker run -p 8080:8080 orchestrator` starts API server
- `docker-compose up` brings up full stack
- Health checks pass in container environment

---

#### Step 4: Integration & Documentation (Week 4)

**Tasks**:
1. End-to-end testing (CLI → API → Core)
2. Performance testing (concurrent requests)
3. Write deployment guide
4. Create example workload definitions
5. Add monitoring/observability hooks
6. Prepare demo environment

**Deliverables**:
- Working CLI + API + Core integration
- Docker images published to registry
- Comprehensive documentation
- Demo scripts and examples

---

## Technical Stack

### Core Libraries

| Category | Library | Version | Purpose |
|----------|---------|---------|---------|
| Async Runtime | tokio | 1.35+ | Async I/O and task scheduling |
| Async Traits | async-trait | 0.1+ | Async methods in traits |
| Serialization | serde | 1.0+ | Data serialization |
| JSON | serde_json | 1.0+ | JSON handling |
| YAML | serde_yaml | 0.9+ | YAML workload files |
| Error Handling | thiserror | 1.0+ | Ergonomic error types |
| Logging | tracing | 0.1+ | Structured logging |
| Logging Subscriber | tracing-subscriber | 0.3+ | Log output formatting |
| HTTP Server | axum | 0.7+ | Web framework |
| HTTP Client | reqwest | 0.11+ | API client |
| CLI Parser | clap | 4.4+ | Command-line interface |
| UUID | uuid | 1.6+ | Unique identifiers |
| Table Display | tabled | 0.15+ | Pretty-printed tables |
| Testing | downcast-rs | 1.2+ | Trait object downcasting |

### Future Dependencies

| Category | Library | Purpose |
|----------|---------|---------|
| State Store | etcd-client | Distributed state storage |
| Consensus | raft-rs | Distributed consensus |
| Container Runtime | youki-lib | OCI runtime integration |
| Networking | netlink-rs | CNI implementation |
| Monitoring | prometheus | Metrics collection |
| gRPC | tonic | High-performance RPC |

---

## Critical Gaps & TODOs

### 🔴 CRITICAL (Phase 1)

#### 1. Persistent State Storage
**Problem**: State is in-memory, lost on restart
**Solution**: Abstract state behind trait, implement etcd/Raft backing

```rust
#[async_trait]
pub trait StateStore: Send + Sync {
    async fn store_workload(&self, workload: WorkloadDefinition) -> Result<()>;
    async fn get_workload(&self, id: &WorkloadId) -> Result<Option<WorkloadDefinition>>;
    async fn store_instance(&self, instance: WorkloadInstance) -> Result<()>;
    async fn get_instances(&self, workload_id: &WorkloadId) -> Result<Vec<WorkloadInstance>>;
    // ... node storage methods
}

// Implementations:
// - InMemoryStateStore (current)
// - EtcdStateStore (production)
// - RaftStateStore (future)
```

#### 2. Container Status Monitoring
**Problem**: No feedback from runtime on container health
**Solution**: Background task to poll container status

```rust
async fn monitor_container_status(&self) {
    let mut interval = tokio::time::interval(Duration::from_secs(10));
    loop {
        interval.tick().await;

        // Get all instances
        let instances = self.state.lock().await.instances.clone();

        for instance in instances.values() {
            // Poll runtime for actual status
            match self.runtime.get_container_status(&instance.node_id, &instance.container_id).await {
                Ok(status) => {
                    // Update instance status in state
                    self.update_instance_status(instance.instance_id, status).await;
                }
                Err(e) => {
                    tracing::error!("Failed to get container status: {}", e);
                }
            }
        }
    }
}
```

#### 3. Youki Runtime Integration
**Problem**: Mock runtime doesn't create real containers
**Solution**: Implement ContainerRuntime trait with Youki

```rust
pub struct YoukiRuntime {
    runtime_path: PathBuf,  // Path to youki binary
    state_dir: PathBuf,      // Runtime state directory
}

#[async_trait]
impl ContainerRuntime for YoukiRuntime {
    async fn create_container(...) -> Result<ContainerId> {
        // 1. Generate OCI runtime spec (config.json)
        let spec = generate_oci_spec(config)?;

        // 2. Write spec to bundle directory
        let bundle_dir = self.state_dir.join(&container_id);
        fs::create_dir_all(&bundle_dir)?;
        write_spec(&bundle_dir, &spec)?;

        // 3. Call youki create
        let output = Command::new(&self.runtime_path)
            .arg("create")
            .arg(&container_id)
            .arg("--bundle")
            .arg(&bundle_dir)
            .output()
            .await?;

        if !output.status.success() {
            return Err(OrchestrationError::RuntimeError(
                String::from_utf8_lossy(&output.stderr).to_string()
            ));
        }

        // 4. Start container
        Command::new(&self.runtime_path)
            .arg("start")
            .arg(&container_id)
            .output()
            .await?;

        Ok(container_id)
    }
}
```

#### 4. Cluster Manager with Gossip Protocol
**Problem**: Static cluster, no dynamic membership
**Solution**: Implement gossip-based membership (memberlist pattern)

```rust
pub struct GossipClusterManager {
    node_id: NodeId,
    bind_addr: SocketAddr,
    peers: Vec<SocketAddr>,
    members: Arc<RwLock<HashMap<NodeId, Node>>>,
    event_tx: watch::Sender<Option<ClusterEvent>>,
}

impl GossipClusterManager {
    async fn gossip_loop(&self) {
        // Periodically exchange membership information
        // Detect failures via timeout
        // Propagate events to orchestrator
    }
}
```

---

### 🟡 MEDIUM (Phase 1-2)

#### 5. Scale-Down Logic
**Problem**: Can only scale up, not down
**Solution**: Implement instance selection for removal

```rust
async fn select_instances_to_remove(
    instances: &[WorkloadInstance],
    count: usize,
) -> Vec<WorkloadInstanceId> {
    // Strategy 1: Remove newest instances first
    // Strategy 2: Remove instances on least-loaded nodes
    // Strategy 3: Honor pod disruption budgets (future)
}
```

#### 6. Resource Validation in Scheduler
**Problem**: Scheduler doesn't check node capacity
**Solution**: Validate resources before placement

```rust
fn can_fit(&self, node: &Node, request: &ResourceRequirements) -> bool {
    let used = self.calculate_used_resources(node);
    node.capacity.cpu_cores >= used.cpu + request.cpu_millicores / 1000 &&
    node.capacity.memory_mb >= used.memory + request.memory_mb
}
```

#### 7. Networking/CNI
**Problem**: No network isolation or service discovery
**Solution**: Implement CNI plugin interface

#### 8. Storage/Volumes
**Problem**: No persistent data for stateful workloads
**Solution**: Volume mount abstraction + CSI driver support

---

### 🟢 LOW (Phase 3-4)

#### 9. MCP Server Integration
**Problem**: No AI-native interface
**Solution**: Model Context Protocol server for intent-based control

#### 10. AI Agent Framework
**Problem**: Manual operations
**Solution**: Autonomous agents for optimization, healing, capacity planning

---

## Next Steps (Immediate)

### 1. Create Project Structure for Phase 2

```bash
cd ai_native_orchestrator
cargo new --lib orchestrator_api
cargo new orchestrator_cli
```

### 2. Update Workspace Manifest

```toml
[workspace]
members = [
    "orchestrator_shared_types",
    "container_runtime_interface",
    "cluster_manager_interface",
    "scheduler_interface",
    "orchestrator_core",
    "orchestrator_api",      # NEW
    "orchestrator_cli",      # NEW
]
```

### 3. Define API Server Dependencies

```toml
# orchestrator_api/Cargo.toml
[dependencies]
orchestrator_core = { path = "../orchestrator_core" }
orchestrator_shared_types = { path = "../orchestrator_shared_types" }
axum = "0.7"
tokio = { version = "1.35", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
tower = "0.4"
tower-http = { version = "0.5", features = ["cors", "trace"] }
```

### 4. Define CLI Dependencies

```toml
# orchestrator_cli/Cargo.toml
[dependencies]
orchestrator_shared_types = { path = "../orchestrator_shared_types" }
clap = { version = "4.4", features = ["derive", "cargo"] }
reqwest = { version = "0.11", features = ["json"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
serde_yaml = "0.9"
tokio = { version = "1.35", features = ["full"] }
tabled = "0.15"
anyhow = "1.0"
```

### 5. Create Dockerfile Template

### 6. Begin API Server Implementation

Start with basic health check and workload submission endpoints.

---

## Success Metrics

### Phase 2 Completion Criteria

- [ ] API server accepts workload submissions via REST
- [ ] CLI can create, list, describe, delete workloads
- [ ] Docker image builds successfully
- [ ] docker-compose brings up orchestrator stack
- [ ] End-to-end test: CLI → API → Core → Mock Runtime
- [ ] Documentation covers deployment and usage
- [ ] Demo video/script prepared

### Future Phases

**Phase 3**: Production-grade features (state persistence, Youki, gossip)
**Phase 4**: MCP integration and AI-native control plane
**Phase 5**: Cloud-agnostic abstractions (AWS, GCP, Azure)
**Phase 6**: Autonomous AI agents for SDLC

---

## References

- **Kubernetes Architecture**: https://kubernetes.io/docs/concepts/architecture/
- **Youki (OCI Runtime)**: https://github.com/containers/youki
- **Linkerd (Rust Service Mesh)**: https://linkerd.io/
- **etcd (Distributed Key-Value)**: https://etcd.io/
- **Raft Consensus**: https://raft.github.io/
- **OCI Runtime Spec**: https://github.com/opencontainers/runtime-spec
- **Model Context Protocol**: https://modelcontextprotocol.io/

---

## Contact & Contribution

**Project**: RustOrchestration Platform
**Status**: Phase 1 → Phase 2 Transition
**License**: TBD
**Branch**: `claude/rust-orchestration-platform-011CULBeBLpjQ7wHVBPsoZSC`

---

**Document Version**: 1.0
**Last Updated**: 2025-10-21
**Next Review**: After Phase 2 completion
