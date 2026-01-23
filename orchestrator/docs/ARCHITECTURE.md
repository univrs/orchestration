# Orchestrator Architecture Deep Dive

> **AI-Native Container Orchestration System**
> A next-generation Kubernetes alternative built in Rust with MCP integration

---

## Table of Contents

1. [Executive Summary](#executive-summary)
2. [System Overview](#system-overview)
3. [Workspace Structure](#workspace-structure)
4. [Core Data Models](#core-data-models)
5. [Trait-Based Architecture](#trait-based-architecture)
6. [Reconciliation Loop](#reconciliation-loop)
7. [Cluster Management](#cluster-management)
8. [Container Runtime](#container-runtime)
9. [API Architecture](#api-architecture)
10. [MCP Server Integration](#mcp-server-integration)
11. [Observability Stack](#observability-stack)
12. [Configuration & Identity](#configuration--identity)
13. [Scheduler Architecture](#scheduler-architecture)
14. [Data Flow Patterns](#data-flow-patterns)
15. [Feature Composition](#feature-composition)

---

## Executive Summary

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    AI-NATIVE CONTAINER ORCHESTRATOR                         │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│   Intent-Driven Control    │    Trait-Based DI    │    Ed25519 Identity    │
│         via MCP            │    for Composability │    Self-Sovereign      │
│                                                                             │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│   ┌─────────┐    ┌─────────┐    ┌──────────┐    ┌──────────┐              │
│   │  REST   │    │   MCP   │    │ WebSocket│    │   CLI    │              │
│   │   API   │    │ Server  │    │  Events  │    │          │              │
│   └────┬────┘    └────┬────┘    └────┬─────┘    └────┬─────┘              │
│        │              │              │               │                     │
│        └──────────────┴──────────────┴───────────────┘                     │
│                              │                                              │
│                    ┌─────────▼─────────┐                                   │
│                    │   ORCHESTRATOR    │                                   │
│                    │    CORE LOOP      │                                   │
│                    └─────────┬─────────┘                                   │
│                              │                                              │
│        ┌─────────────────────┼─────────────────────┐                       │
│        │                     │                     │                       │
│   ┌────▼────┐          ┌─────▼─────┐         ┌────▼────┐                  │
│   │ Cluster │          │   State   │         │Container│                  │
│   │ Manager │          │   Store   │         │ Runtime │                  │
│   └─────────┘          └───────────┘         └─────────┘                  │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Key Design Principles

| Principle | Implementation |
|-----------|----------------|
| **Intent-Driven** | MCP protocol enables AI agents to control orchestration |
| **Trait-Based DI** | All components use `Arc<dyn Trait>` for testability |
| **Control Loop** | Classical Sense → Compare → Plan → Actuate pattern |
| **Event-Driven** | Broadcast channels ensure no lost cluster events |
| **Self-Sovereign Identity** | Ed25519 keys - no central authority needed |

---

## System Overview

```
┌──────────────────────────────────────────────────────────────────────────────┐
│                           ORCHESTRATOR SYSTEM                                │
│                                                                              │
│  ┌────────────────────────────────────────────────────────────────────────┐ │
│  │                         CONTROL PLANE                                   │ │
│  │                                                                         │ │
│  │   ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  │ │
│  │   │  REST API   │  │ MCP Server  │  │  WebSocket  │  │     CLI     │  │ │
│  │   │  (Axum)     │  │ (JSON-RPC)  │  │   Events    │  │  (clap)     │  │ │
│  │   └──────┬──────┘  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘  │ │
│  │          │                │                │                │          │ │
│  │          └────────────────┴────────────────┴────────────────┘          │ │
│  │                                   │                                     │ │
│  │                          ┌────────▼────────┐                           │ │
│  │                          │  ORCHESTRATOR   │                           │ │
│  │                          │   MAIN LOOP     │                           │ │
│  │                          │                 │                           │ │
│  │                          │  tokio::select! │                           │ │
│  │                          └────────┬────────┘                           │ │
│  │                                   │                                     │ │
│  └───────────────────────────────────┼─────────────────────────────────────┘ │
│                                      │                                       │
│  ┌───────────────────────────────────┼─────────────────────────────────────┐ │
│  │                         DATA PLANE                                      │ │
│  │                                   │                                      │ │
│  │     ┌─────────────┬───────────────┼───────────────┬─────────────┐       │ │
│  │     │             │               │               │             │       │ │
│  │     ▼             ▼               ▼               ▼             ▼       │ │
│  │ ┌───────┐   ┌──────────┐   ┌───────────┐   ┌──────────┐   ┌─────────┐  │ │
│  │ │Cluster│   │Container │   │   State   │   │Scheduler │   │Observa- │  │ │
│  │ │Manager│   │ Runtime  │   │   Store   │   │          │   │ bility  │  │ │
│  │ └───┬───┘   └────┬─────┘   └─────┬─────┘   └────┬─────┘   └────┬────┘  │ │
│  │     │            │               │              │              │        │ │
│  │     ▼            ▼               ▼              ▼              ▼        │ │
│  │ ┌───────┐   ┌──────────┐   ┌───────────┐   ┌──────────┐   ┌─────────┐  │ │
│  │ │Chitchat│  │   Youki  │   │  SQLite/  │   │  Simple  │   │Prometheus│ │ │
│  │ │ Gossip │  │   OCI    │   │  Memory   │   │ Scheduler│   │ Metrics  │ │ │
│  │ └───────┘   └──────────┘   └───────────┘   └──────────┘   └─────────┘  │ │
│  │                                                                         │ │
│  └─────────────────────────────────────────────────────────────────────────┘ │
│                                                                              │
└──────────────────────────────────────────────────────────────────────────────┘
```

---

## Workspace Structure

### Crate Organization

```
orchestrator/
│
├── INTERFACE CRATES (Define Contracts)
│   │
│   ├── orchestrator_shared_types/     ─── Common types: Node, Workload, Error
│   ├── container_runtime_interface/   ─── ContainerRuntime trait
│   ├── cluster_manager_interface/     ─── ClusterManager trait
│   ├── scheduler_interface/           ─── Scheduler trait
│   └── state_store_interface/         ─── StateStore trait
│
├── IMPLEMENTATION CRATES
│   │
│   ├── orchestrator_core/             ─── Main orchestration engine
│   │   ├── src/
│   │   │   ├── lib.rs                 ─── Orchestrator struct & main loop
│   │   │   ├── main.rs                ─── Daemon entry point
│   │   │   ├── api/                   ─── REST API (routes, auth, handlers)
│   │   │   └── reconciliation/        ─── Sense/Compare/Plan/Actuate
│   │   └── tests/                     ─── Integration tests
│   │
│   ├── container_runtime/             ─── Runtime implementations
│   │   ├── mock.rs                    ─── In-memory simulation
│   │   ├── youki_cli.rs               ─── Youki CLI wrapper
│   │   └── oci_bundle/                ─── OCI spec generation
│   │
│   ├── cluster_manager/               ─── Cluster implementations
│   │   ├── mock.rs                    ─── In-memory simulation
│   │   └── chitchat_manager.rs        ─── Gossip-based clustering
│   │
│   ├── mcp_server/                    ─── Model Context Protocol
│   │   ├── server.rs                  ─── JSON-RPC server
│   │   ├── tools/                     ─── MCP tools (workload, node, cluster)
│   │   └── resources/                 ─── MCP resources
│   │
│   ├── observability/                 ─── Monitoring & telemetry
│   │   ├── metrics.rs                 ─── Prometheus metrics
│   │   ├── tracing_setup.rs           ─── Structured logging
│   │   ├── health.rs                  ─── Health checks
│   │   ├── events.rs                  ─── Event hub
│   │   └── websocket.rs               ─── Real-time streaming
│   │
│   ├── user_config/                   ─── Configuration management
│   │   ├── identity.rs                ─── Ed25519 identity
│   │   ├── trust_policy.rs            ─── Trust configuration
│   │   └── encryption.rs              ─── Secret storage
│   │
│   └── ui_cli/                        ─── Command-line interface
│
└── External Dependencies
    ├── univrs-identity/               ─── Identity primitives
    └── univrs-state/                  ─── State management
```

### Dependency Graph

```
                    ┌─────────────────────────────┐
                    │  orchestrator_shared_types  │
                    │  (Core domain types)        │
                    └──────────────┬──────────────┘
                                   │
           ┌───────────────────────┼───────────────────────┐
           │                       │                       │
           ▼                       ▼                       ▼
┌──────────────────┐    ┌──────────────────┐    ┌──────────────────┐
│ container_runtime│    │  cluster_manager │    │   state_store    │
│    _interface    │    │    _interface    │    │    _interface    │
└────────┬─────────┘    └────────┬─────────┘    └────────┬─────────┘
         │                       │                       │
         ▼                       ▼                       ▼
┌──────────────────┐    ┌──────────────────┐    ┌──────────────────┐
│container_runtime │    │  cluster_manager │    │  (InMemory/      │
│ (mock/youki)     │    │  (mock/chitchat) │    │   SQLite impl)   │
└────────┬─────────┘    └────────┬─────────┘    └────────┬─────────┘
         │                       │                       │
         └───────────────────────┼───────────────────────┘
                                 │
                    ┌────────────▼────────────┐
                    │    orchestrator_core    │
                    │    (Main engine)        │
                    └────────────┬────────────┘
                                 │
              ┌──────────────────┼──────────────────┐
              │                  │                  │
              ▼                  ▼                  ▼
     ┌─────────────┐    ┌─────────────┐    ┌─────────────┐
     │ mcp_server  │    │observability│    │ user_config │
     └─────────────┘    └─────────────┘    └─────────────┘
```

---

## Core Data Models

### Identity Model (Ed25519-Based)

```
┌─────────────────────────────────────────────────────────────────────┐
│                     IDENTITY MODEL                                  │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│   NodeId = Ed25519 PublicKey (32 bytes)                            │
│   ┌──────────────────────────────────────┐                         │
│   │  Self-sovereign: No central registry │                         │
│   │  Cryptographic: Can sign messages    │                         │
│   │  Verifiable: Anyone can verify       │                         │
│   └──────────────────────────────────────┘                         │
│                                                                     │
│   WorkloadId = UUID v4                                              │
│   ┌──────────────────────────────────────┐                         │
│   │  Globally unique                     │                         │
│   │  Generated at creation time          │                         │
│   └──────────────────────────────────────┘                         │
│                                                                     │
│   ContainerId = String                                              │
│   ┌──────────────────────────────────────┐                         │
│   │  Runtime-specific identifier         │                         │
│   │  Typically hash or UUID format       │                         │
│   └──────────────────────────────────────┘                         │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### State Model

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              STATE MODEL                                    │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                            NODE                                      │   │
│  ├─────────────────────────────────────────────────────────────────────┤   │
│  │  id: NodeId (Ed25519 PublicKey)                                     │   │
│  │  address: String ("10.0.0.1:8080")                                  │   │
│  │  status: NodeStatus                                                  │   │
│  │    ├── Ready        (accepting workloads)                           │   │
│  │    ├── NotReady     (not accepting workloads)                       │   │
│  │    ├── Unknown      (status undetermined)                           │   │
│  │    └── Down         (node unreachable)                              │   │
│  │  labels: HashMap<String, String>                                    │   │
│  │  resources_capacity: NodeResources                                  │   │
│  │  resources_allocatable: NodeResources                               │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                     WORKLOAD DEFINITION                              │   │
│  ├─────────────────────────────────────────────────────────────────────┤   │
│  │  id: WorkloadId (UUID)                                              │   │
│  │  name: String                                                        │   │
│  │  containers: Vec<ContainerConfig>                                   │   │
│  │    ├── name: String                                                 │   │
│  │    ├── image: String                                                │   │
│  │    ├── command: Option<Vec<String>>                                 │   │
│  │    ├── args: Option<Vec<String>>                                    │   │
│  │    ├── env_vars: HashMap<String, String>                            │   │
│  │    ├── ports: Vec<PortMapping>                                      │   │
│  │    └── resources: ContainerResources                                │   │
│  │  replicas: u32                                                       │   │
│  │  labels: HashMap<String, String>                                    │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                     WORKLOAD INSTANCE                                │   │
│  ├─────────────────────────────────────────────────────────────────────┤   │
│  │  id: UUID                                                           │   │
│  │  workload_id: WorkloadId                                            │   │
│  │  node_id: NodeId                                                    │   │
│  │  container_ids: Vec<ContainerId>                                    │   │
│  │  status: WorkloadInstanceStatus                                     │   │
│  │    ├── Pending      (waiting to start)                              │   │
│  │    ├── Running      (containers running)                            │   │
│  │    ├── Succeeded    (completed successfully)                        │   │
│  │    ├── Failed       (execution failed)                              │   │
│  │    └── Unknown      (status undetermined)                           │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Trait-Based Architecture

### Core Traits Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        TRAIT-BASED DEPENDENCY INJECTION                     │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│                         Arc<dyn Trait + Send + Sync>                        │
│                                    │                                        │
│         ┌──────────────────────────┼──────────────────────────┐            │
│         │                          │                          │            │
│         ▼                          ▼                          ▼            │
│  ┌─────────────┐           ┌─────────────┐           ┌─────────────┐       │
│  │  Interface  │           │  Interface  │           │  Interface  │       │
│  │   Crate     │           │   Crate     │           │   Crate     │       │
│  └──────┬──────┘           └──────┬──────┘           └──────┬──────┘       │
│         │                          │                          │            │
│         ▼                          ▼                          ▼            │
│  ┌─────────────┐           ┌─────────────┐           ┌─────────────┐       │
│  │    Impl     │           │    Impl     │           │    Impl     │       │
│  │   Crate     │           │   Crate     │           │   Crate     │       │
│  └─────────────┘           └─────────────┘           └─────────────┘       │
│                                                                             │
│  Benefits:                                                                  │
│  ✓ Swappable implementations (mock vs production)                          │
│  ✓ Easy testing with mock components                                       │
│  ✓ Feature-flag based composition                                          │
│  ✓ Clear separation of interface and implementation                        │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### ContainerRuntime Trait

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         ContainerRuntime Trait                              │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  #[async_trait]                                                             │
│  pub trait ContainerRuntime: Send + Sync {                                  │
│      async fn init_node(&self, node_id: NodeId) -> Result<()>;             │
│      async fn create_container(...) -> Result<ContainerId>;                │
│      async fn stop_container(&self, id: &ContainerId) -> Result<()>;       │
│      async fn remove_container(&self, id: &ContainerId) -> Result<()>;     │
│      async fn get_container_status(...) -> Result<ContainerStatus>;        │
│      async fn list_containers(...) -> Result<Vec<ContainerStatus>>;        │
│      async fn get_container_logs(...) -> Result<String>;                   │
│  }                                                                          │
│                                                                             │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  IMPLEMENTATIONS                                                            │
│                                                                             │
│  ┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐         │
│  │  MockRuntime    │    │ YoukiCliRuntime │    │  YoukiRuntime   │         │
│  ├─────────────────┤    ├─────────────────┤    ├─────────────────┤         │
│  │ In-memory       │    │ CLI invocation  │    │ Direct library  │         │
│  │ simulation      │    │ of youki binary │    │ integration     │         │
│  │                 │    │                 │    │                 │         │
│  │ Feature:        │    │ Feature:        │    │ Feature:        │         │
│  │ mock-runtime    │    │ youki-cli       │    │ youki-lib       │         │
│  └─────────────────┘    └─────────────────┘    └─────────────────┘         │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### ClusterManager Trait

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                          ClusterManager Trait                               │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  #[async_trait]                                                             │
│  pub trait ClusterManager: Downcast + Send + Sync {                         │
│      async fn initialize(&self) -> Result<()>;                              │
│      async fn get_node(&self, node_id: &NodeId) -> Result<Option<Node>>;   │
│      async fn list_nodes(&self) -> Result<Vec<Node>>;                       │
│      async fn subscribe_to_events(&self)                                    │
│          -> Result<broadcast::Receiver<ClusterEvent>>;                      │
│  }                                                                          │
│                                                                             │
│  pub enum ClusterEvent {                                                    │
│      NodeAdded(Node),                                                       │
│      NodeRemoved(NodeId),                                                   │
│      NodeUpdated(Node),                                                     │
│  }                                                                          │
│                                                                             │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  BROADCAST CHANNEL DESIGN                                                   │
│                                                                             │
│  ┌─────────────┐                                                           │
│  │   Sender    │──────┬──────────┬──────────┬──────────────────────        │
│  └─────────────┘      │          │          │                              │
│                       ▼          ▼          ▼                              │
│               ┌──────────┐ ┌──────────┐ ┌──────────┐                       │
│               │Receiver 1│ │Receiver 2│ │Receiver 3│                       │
│               └──────────┘ └──────────┘ └──────────┘                       │
│                                                                             │
│  ✓ All subscribers receive ALL events                                       │
│  ✓ No event loss (unlike mpsc)                                             │
│  ✓ Handles lagging receivers gracefully                                    │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### StateStore Trait

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                            StateStore Trait                                 │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  #[async_trait]                                                             │
│  pub trait StateStore: Send + Sync {                                        │
│      // Lifecycle                                                           │
│      async fn initialize(&self) -> Result<()>;                              │
│      async fn health_check(&self) -> Result<bool>;                          │
│                                                                             │
│      // Node operations                                                     │
│      async fn put_node(&self, node: Node) -> Result<()>;                   │
│      async fn get_node(&self, id: &NodeId) -> Result<Option<Node>>;        │
│      async fn list_nodes(&self) -> Result<Vec<Node>>;                       │
│      async fn delete_node(&self, id: &NodeId) -> Result<()>;               │
│                                                                             │
│      // Workload operations                                                 │
│      async fn put_workload(&self, w: WorkloadDefinition) -> Result<()>;    │
│      async fn get_workload(&self, id: &WorkloadId) -> ...;                 │
│      async fn list_workloads(&self) -> Result<Vec<WorkloadDefinition>>;    │
│      async fn delete_workload(&self, id: &WorkloadId) -> Result<()>;       │
│                                                                             │
│      // Instance operations                                                 │
│      async fn put_instance(&self, i: WorkloadInstance) -> Result<()>;      │
│      async fn put_instances_batch(&self, ...) -> Result<()>;               │
│      async fn list_instances_for_workload(...) -> ...;                     │
│      async fn list_all_instances(&self) -> ...;                            │
│      async fn delete_instance(&self, id: &str) -> Result<()>;              │
│  }                                                                          │
│                                                                             │
├─────────────────────────────────────────────────────────────────────────────┤
│  IMPLEMENTATIONS                                                            │
│                                                                             │
│  ┌──────────────────┐         ┌──────────────────┐                         │
│  │ InMemoryStore    │         │   SqliteStore    │                         │
│  ├──────────────────┤         ├──────────────────┤                         │
│  │ RwLock<HashMap>  │         │ rusqlite + r2d2  │                         │
│  │ Testing/Dev      │         │ Production       │                         │
│  └──────────────────┘         └──────────────────┘                         │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Scheduler Trait

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                            Scheduler Trait                                  │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  #[async_trait]                                                             │
│  pub trait Scheduler: Send + Sync {                                         │
│      async fn schedule(                                                     │
│          &self,                                                             │
│          request: &ScheduleRequest,                                         │
│          available_nodes: &[Node],                                          │
│      ) -> Result<Vec<ScheduleDecision>>;                                    │
│  }                                                                          │
│                                                                             │
│  pub struct ScheduleRequest {                                               │
│      pub workload_definition: Arc<WorkloadDefinition>,                      │
│      pub current_instances: Vec<WorkloadInstance>,                          │
│  }                                                                          │
│                                                                             │
│  pub enum ScheduleDecision {                                                │
│      AssignNode(NodeId),     // Place on this node                          │
│      NoPlacement(String),    // Cannot place (with reason)                  │
│      Error(String),          // Scheduling error                            │
│  }                                                                          │
│                                                                             │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  SCHEDULING PIPELINE (Kubernetes-style)                                     │
│                                                                             │
│  ┌──────────┐      ┌──────────┐      ┌──────────┐                          │
│  │  FILTER  │ ──▶  │  SCORE   │ ──▶  │   BIND   │                          │
│  └──────────┘      └──────────┘      └──────────┘                          │
│       │                 │                 │                                 │
│       ▼                 ▼                 ▼                                 │
│  Eliminate         Rank eligible     Select top                            │
│  ineligible        nodes by          node and                              │
│  nodes             preference        assign                                 │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Reconciliation Loop

### Classical Control Loop Pattern

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         RECONCILIATION LOOP                                 │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│                    ┌────────────────────────────────┐                       │
│                    │                                │                       │
│                    ▼                                │                       │
│             ┌─────────────┐                         │                       │
│             │             │                         │                       │
│             │    SENSE    │  Observe current state  │                       │
│             │             │                         │                       │
│             └──────┬──────┘                         │                       │
│                    │                                │                       │
│                    ▼                                │                       │
│             ┌─────────────┐                         │                       │
│             │             │                         │                       │
│             │   COMPARE   │  Diff desired vs current│                       │
│             │             │                         │                       │
│             └──────┬──────┘                         │                       │
│                    │                                │                       │
│                    ▼                                │                       │
│             ┌─────────────┐                         │                       │
│             │             │                         │                       │
│             │    PLAN     │  Generate action sequence                       │
│             │             │                         │                       │
│             └──────┬──────┘                         │                       │
│                    │                                │                       │
│                    ▼                                │                       │
│             ┌─────────────┐                         │                       │
│             │             │                         │                       │
│             │   ACTUATE   │  Execute with retry     │                       │
│             │             │                         │                       │
│             └──────┬──────┘                         │                       │
│                    │                                │                       │
│                    └────────────────────────────────┘                       │
│                                                                             │
│  Properties:                                                                │
│  ✓ Idempotent: Same input → same output                                    │
│  ✓ Declarative: Focus on desired state                                     │
│  ✓ Self-healing: Continuous reconciliation                                 │
│  ✓ Observable: Events at each phase                                        │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Phase 1: SENSE

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           SENSE PHASE                                       │
│                     orchestrator_core/src/reconciliation/sense.rs           │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  PURPOSE: Atomic, read-only observation of cluster state                    │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                      SenseScope                                      │   │
│  │  ┌──────────────┐ ┌──────────────┐ ┌──────────────┐ ┌────────────┐  │   │
│  │  │  containers  │ │    nodes     │ │  workloads   │ │ instances  │  │   │
│  │  │   (bool)     │ │   (bool)     │ │   (bool)     │ │  (bool)    │  │   │
│  │  └──────────────┘ └──────────────┘ └──────────────┘ └────────────┘  │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                        │                                    │
│                                        ▼                                    │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                      SenseOperation                                  │   │
│  │                                                                      │   │
│  │  containers: Vec<ContainerState>                                    │   │
│  │  nodes: Vec<NodeState>                                              │   │
│  │  workloads: Vec<WorkloadState>                                      │   │
│  │  instances: Vec<InstanceState>                                      │   │
│  │  completeness: Completeness { Full | Partial }                      │   │
│  │  correlation_id: UUID  ─── For distributed tracing                  │   │
│  │                                                                      │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  GUARANTEES:                                                                │
│  • Read-only (no mutations)                                                 │
│  • Atomic snapshot                                                          │
│  • Tracks completeness                                                      │
│  • Correlation for tracing                                                  │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Phase 2: COMPARE

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                          COMPARE PHASE                                      │
│                    orchestrator_core/src/reconciliation/compare.rs          │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  PURPOSE: Compute difference between desired and current state              │
│                                                                             │
│      DesiredState                     CurrentState                          │
│  ┌─────────────────┐              ┌─────────────────┐                      │
│  │ workload_defs   │              │ nodes           │                      │
│  │ version         │              │ workloads       │                      │
│  │ timestamp       │              │ instances       │                      │
│  └────────┬────────┘              │ containers      │                      │
│           │                       └────────┬────────┘                      │
│           │                                │                                │
│           └──────────────┬─────────────────┘                               │
│                          │                                                  │
│                          ▼                                                  │
│                   ┌─────────────┐                                          │
│                   │    DIFF     │                                          │
│                   └──────┬──────┘                                          │
│                          │                                                  │
│           ┌──────────────┼──────────────┬──────────────┐                   │
│           ▼              ▼              ▼              ▼                   │
│    ┌───────────┐  ┌───────────┐  ┌───────────┐  ┌───────────┐             │
│    │ Additions │  │Modifications│  │ Deletions │  │ Unchanged │             │
│    │  (new)    │  │ (changed)  │  │ (removed) │  │  (same)   │             │
│    └───────────┘  └───────────┘  └───────────┘  └───────────┘             │
│                                                                             │
│  DRIFT METRICS:                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  magnitude: u64      (total number of changes)                       │   │
│  │  urgency: Urgency    (Critical | High | Medium | Low)               │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Phase 3: PLAN

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                            PLAN PHASE                                       │
│                     orchestrator_core/src/reconciliation/plan.rs            │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  PURPOSE: Generate ordered, reversible action sequences                     │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                      ReconciliationPlan                              │   │
│  │                                                                      │   │
│  │  actions: Vec<Action>                                               │   │
│  │  action_sequences: Vec<ActionSequence>                              │   │
│  │  rollback_points: Vec<RollbackPoint>                                │   │
│  │  estimated_duration: Duration                                       │   │
│  │  risk_level: RiskLevel                                              │   │
│  │                                                                      │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  ACTION STRUCTURE:                                                          │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  Action                                                              │   │
│  │  ├── id: ActionId                                                   │   │
│  │  ├── entity: EntityReference                                        │   │
│  │  ├── action_type: Create | Update | Delete | Migrate | Scale       │   │
│  │  ├── risk_level: Low | Medium | High                                │   │
│  │  ├── reversibility: Reversible | Irreversible                       │   │
│  │  ├── preconditions: Vec<Precondition>                               │   │
│  │  │   ├── ActionCompleted(ActionId)                                  │   │
│  │  │   ├── EntityExists(EntityId)                                     │   │
│  │  │   ├── EntityState { id, required_state }                         │   │
│  │  │   └── ResourceAvailable { type, amount }                         │   │
│  │  └── rollback_action: Option<Box<Action>>                           │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  SEQUENCING STRATEGIES:                                                     │
│  • Minimal disruption (rolling updates)                                     │
│  • Resource efficiency (capacity awareness)                                 │
│  • Failure isolation (grouped execution)                                    │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Phase 4: ACTUATE

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                          ACTUATE PHASE                                      │
│                    orchestrator_core/src/reconciliation/actuate.rs          │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  PURPOSE: Execute plan with concurrency control, retry, and rollback        │
│                                                                             │
│  EXECUTION FLOW:                                                            │
│                                                                             │
│  ┌──────────────────────────────────────────────────────────────────────┐  │
│  │  Plan                                                                 │  │
│  │  ┌────────┐ ┌────────┐ ┌────────┐ ┌────────┐ ┌────────┐             │  │
│  │  │Action 1│ │Action 2│ │Action 3│ │Action 4│ │Action 5│             │  │
│  │  └───┬────┘ └───┬────┘ └────────┘ └───┬────┘ └───┬────┘             │  │
│  │      │          │                     │          │                   │  │
│  │      │          │   Dependency        │          │                   │  │
│  │      │          │   ───────────▶      │          │                   │  │
│  │      │          │                     │          │                   │  │
│  │      ▼          ▼                     ▼          ▼                   │  │
│  │  ┌──────────────────────────────────────────────────────────────┐   │  │
│  │  │              CONCURRENT EXECUTOR                              │   │  │
│  │  │                                                               │   │  │
│  │  │  max_concurrent: 10                                          │   │  │
│  │  │  retry_policy: RetryPolicy                                   │   │  │
│  │  │                                                               │   │  │
│  │  └──────────────────────────────────────────────────────────────┘   │  │
│  └──────────────────────────────────────────────────────────────────────┘  │
│                                                                             │
│  RETRY STRATEGIES:                                                          │
│  ┌─────────────────┐ ┌─────────────────┐ ┌─────────────────┐               │
│  │     Fixed       │ │   Exponential   │ │     Linear      │               │
│  │   ───────       │ │   ───────────   │ │   ───────       │               │
│  │   1s, 1s, 1s    │ │   1s, 2s, 4s    │ │   1s, 2s, 3s    │               │
│  └─────────────────┘ └─────────────────┘ └─────────────────┘               │
│                                                                             │
│  EXECUTION PROGRESS:                                                        │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  total_actions: 5                                                   │   │
│  │  completed_actions: 3  ████████████░░░░░░░░                         │   │
│  │  failed_actions: 0                                                  │   │
│  │  skipped_actions: 0                                                 │   │
│  │  in_progress_actions: 2                                             │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Cluster Management

### Chitchat Gossip Protocol

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        CHITCHAT CLUSTER MANAGER                             │
│                      cluster_manager/src/chitchat_manager.rs                │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  GOSSIP-BASED MEMBERSHIP                                                    │
│                                                                             │
│  ┌─────────┐         ┌─────────┐         ┌─────────┐                       │
│  │ Node A  │◀───────▶│ Node B  │◀───────▶│ Node C  │                       │
│  └────┬────┘         └────┬────┘         └────┬────┘                       │
│       │                   │                   │                             │
│       │    ┌──────────────┴──────────────┐    │                             │
│       │    │                             │    │                             │
│       └───▶│          UDP Gossip         │◀───┘                             │
│            │     (CRDT-based state)      │                                  │
│            │                             │                                  │
│            └─────────────────────────────┘                                  │
│                                                                             │
│  FEATURES:                                                                  │
│  • Phi-accrual failure detection                                            │
│  • CRDT-based state synchronization                                         │
│  • Automatic peer discovery                                                 │
│  • Generation ID for restarts                                               │
│                                                                             │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  NODE METADATA (Key-Value Store)                                            │
│                                                                             │
│  ┌───────────────────────────────────────────────────────────────────────┐ │
│  │  Key                          │ Value                                 │ │
│  ├───────────────────────────────┼───────────────────────────────────────┤ │
│  │  node:address                 │ "10.0.0.1:8080"                       │ │
│  │  node:status                  │ "Ready"                               │ │
│  │  node:cpu_capacity            │ "4.0"                                 │ │
│  │  node:memory_mb_capacity      │ "8192"                                │ │
│  │  node:disk_mb_capacity        │ "102400"                              │ │
│  │  node:cpu_allocatable         │ "3.5"                                 │ │
│  │  node:memory_mb_allocatable   │ "7168"                                │ │
│  │  node:disk_mb_allocatable     │ "92160"                               │ │
│  │  node:labels                  │ '{"zone":"us-east-1"}'                │ │
│  └───────────────────────────────┴───────────────────────────────────────┘ │
│                                                                             │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  CONFIGURATION                                                              │
│                                                                             │
│  ChitchatClusterConfig {                                                    │
│      node_id: "ed25519-public-key-string",                                 │
│      listen_addr: "0.0.0.0:7280",                                          │
│      public_addr: "192.168.1.10:7280",                                     │
│      seed_nodes: ["192.168.1.1:7280", "192.168.1.2:7280"],                 │
│      gossip_interval: 500ms,                                               │
│      failure_detection_threshold: 8.0,    // Phi-accrual threshold         │
│      dead_node_grace_period: 60s,                                          │
│      cluster_id: "orchestrator-cluster",                                   │
│      generation_id: 0,                    // Incremented on restart        │
│  }                                                                          │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Event Broadcasting

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                          EVENT BROADCASTING                                 │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  BROADCAST CHANNEL ARCHITECTURE                                             │
│                                                                             │
│                    ┌───────────────────────────┐                           │
│                    │   ClusterManager          │                           │
│                    │   broadcast::Sender       │                           │
│                    └─────────────┬─────────────┘                           │
│                                  │                                          │
│          ┌───────────────────────┼───────────────────────┐                 │
│          │                       │                       │                 │
│          ▼                       ▼                       ▼                 │
│  ┌───────────────┐       ┌───────────────┐       ┌───────────────┐        │
│  │ Orchestrator  │       │   REST API    │       │  MCP Server   │        │
│  │   Main Loop   │       │   Handlers    │       │   Handlers    │        │
│  │               │       │               │       │               │        │
│  │ broadcast::   │       │ broadcast::   │       │ broadcast::   │        │
│  │   Receiver    │       │   Receiver    │       │   Receiver    │        │
│  └───────────────┘       └───────────────┘       └───────────────┘        │
│                                                                             │
│  GUARANTEES:                                                                │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  ✓ All events delivered to all active subscribers                    │   │
│  │  ✓ FIFO ordering within each subscriber                              │   │
│  │  ✓ Lagging subscribers receive warning, not dropped                  │   │
│  │  ✓ No event loss if at least one receiver exists                     │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  EVENT TYPES:                                                               │
│  ┌─────────────────────┐                                                   │
│  │  ClusterEvent       │                                                   │
│  │  ├── NodeAdded      │  →  New node joined cluster                       │
│  │  ├── NodeRemoved    │  →  Node left or failed                           │
│  │  └── NodeUpdated    │  →  Node status/resources changed                 │
│  └─────────────────────┘                                                   │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Container Runtime

### Runtime Abstraction

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         CONTAINER RUNTIME                                   │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  RUNTIME IMPLEMENTATIONS                                                    │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                        MockRuntime                                   │   │
│  │                     (Feature: mock-runtime)                          │   │
│  ├─────────────────────────────────────────────────────────────────────┤   │
│  │  • In-memory container simulation                                   │   │
│  │  • State machine: created → running → stopped → removed             │   │
│  │  • Per-node container tracking                                      │   │
│  │  • No actual process execution                                      │   │
│  │  • Perfect for testing and development                              │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                      YoukiCliRuntime                                 │   │
│  │                      (Feature: youki-cli)                            │   │
│  ├─────────────────────────────────────────────────────────────────────┤   │
│  │  • CLI wrapper around youki binary                                  │   │
│  │  • OCI-compliant container execution                                │   │
│  │  • Bundle generation via oci_bundle module                          │   │
│  │  • Log capture to {state_root}/{container_id}/container.log         │   │
│  │  • Recommended for production                                       │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  CONTAINER LIFECYCLE:                                                       │
│                                                                             │
│      ┌────────┐      ┌─────────┐      ┌─────────┐      ┌─────────┐        │
│      │ Create │ ───▶ │  Start  │ ───▶ │  Stop   │ ───▶ │ Remove  │        │
│      └────────┘      └─────────┘      └─────────┘      └─────────┘        │
│           │               │                │                │              │
│           ▼               ▼                ▼                ▼              │
│      ┌────────┐      ┌─────────┐      ┌─────────┐      ┌─────────┐        │
│      │Created │      │ Running │      │ Stopped │      │ Deleted │        │
│      └────────┘      └─────────┘      └─────────┘      └─────────┘        │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### OCI Bundle Generation

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         OCI BUNDLE GENERATION                               │
│                     container_runtime/src/oci_bundle/                       │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  BUNDLE STRUCTURE                                                           │
│                                                                             │
│  {bundle_root}/{container_id}/                                              │
│  ├── config.json          ← OCI runtime specification                       │
│  └── rootfs/              ← Container root filesystem                       │
│      ├── bin/                                                               │
│      ├── etc/                                                               │
│      ├── lib/                                                               │
│      └── ...                                                                │
│                                                                             │
│  MODULES:                                                                   │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  spec.rs    → OCI spec generation (process, mounts, resources)      │   │
│  │  rootfs.rs  → Root filesystem management                            │   │
│  │  builder.rs → Bundle construction orchestration                     │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  RESOURCE LIMITS:                                                           │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  CPU:    cpu.shares, cpu.quota, cpu.period                          │   │
│  │  Memory: memory.limit_in_bytes, memory.swap                         │   │
│  │  PIDs:   pids.max                                                   │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## API Architecture

### REST API with Ed25519 Authentication

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                            REST API                                         │
│                      orchestrator_core/src/api/                             │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ENDPOINTS                                                                  │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  WORKLOADS                                                           │   │
│  │  POST   /api/v1/workloads              Create workload               │   │
│  │  GET    /api/v1/workloads              List workloads                │   │
│  │  GET    /api/v1/workloads/:id          Get workload                  │   │
│  │  PUT    /api/v1/workloads/:id          Update workload               │   │
│  │  DELETE /api/v1/workloads/:id          Delete workload               │   │
│  │  GET    /api/v1/workloads/:id/instances   List instances             │   │
│  │  GET    /api/v1/workloads/:id/logs     Get logs                      │   │
│  │  GET    /api/v1/workloads/:id/logs/stream  Stream logs (WebSocket)   │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  NODES                                                               │   │
│  │  GET    /api/v1/nodes                  List nodes                    │   │
│  │  GET    /api/v1/nodes/:id              Get node                      │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  CLUSTER                                                             │   │
│  │  GET    /api/v1/cluster/status         Cluster status                │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ED25519 AUTHENTICATION                                                     │
│                                                                             │
│  REQUEST HEADERS:                                                           │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  X-Auth-PublicKey:  Base64(ed25519_public_key)                      │   │
│  │  X-Auth-Timestamp:  ISO8601 timestamp                               │   │
│  │  X-Auth-Signature:  Base64(ed25519_signature)                       │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  SIGNATURE PAYLOAD:                                                         │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  {METHOD}\n{PATH}\n{TIMESTAMP}\n{SHA256(BODY)}                      │   │
│  │                                                                      │   │
│  │  Example:                                                            │   │
│  │  POST\n/api/v1/workloads\n2026-01-26T10:00:00Z\n{body_hash}         │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  VALIDATION:                                                                │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  1. Check timestamp age (configurable skew tolerance)               │   │
│  │  2. Reconstruct signature payload                                   │   │
│  │  3. Verify Ed25519 signature                                        │   │
│  │  4. (Optional) Check against trusted key list                       │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## MCP Server Integration

### Model Context Protocol

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           MCP SERVER                                        │
│                         mcp_server/src/                                     │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  PURPOSE: Expose orchestrator to AI agents via JSON-RPC 2.0                 │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                                                                      │   │
│  │    AI Agent                                                          │   │
│  │       │                                                              │   │
│  │       │  JSON-RPC 2.0                                               │   │
│  │       ▼                                                              │   │
│  │  ┌─────────────────────────────────────────────────────────────┐    │   │
│  │  │                    MCP Server                                │    │   │
│  │  │                                                              │    │   │
│  │  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │    │   │
│  │  │  │    Tools     │  │  Resources   │  │   Prompts    │      │    │   │
│  │  │  └──────────────┘  └──────────────┘  └──────────────┘      │    │   │
│  │  │                                                              │    │   │
│  │  └─────────────────────────────────────────────────────────────┘    │   │
│  │       │                                                              │   │
│  │       ▼                                                              │   │
│  │  ┌─────────────────────────────────────────────────────────────┐    │   │
│  │  │              Orchestrator Components                         │    │   │
│  │  │   StateStore │ ClusterManager │ Scheduler                   │    │   │
│  │  └─────────────────────────────────────────────────────────────┘    │   │
│  │                                                                      │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  AVAILABLE TOOLS                                                            │
│                                                                             │
│  WORKLOAD MANAGEMENT                                                        │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  list_workloads    List all workloads with optional name filter     │   │
│  │  create_workload   Create new workload with containers & replicas   │   │
│  │  get_workload      Get workload details by ID                       │   │
│  │  update_workload   Update workload configuration                    │   │
│  │  scale_workload    Scale number of replicas                         │   │
│  │  delete_workload   Delete workload                                  │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  NODE MANAGEMENT                                                            │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  list_nodes        List all nodes with resource availability        │   │
│  │  get_node          Get node details by ID                           │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  CLUSTER OPERATIONS                                                         │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  get_cluster_status   Overall cluster health and capacity           │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  EXAMPLE TOOL CALL                                                          │
│                                                                             │
│  Request:                                                                   │
│  {                                                                          │
│    "jsonrpc": "2.0",                                                        │
│    "method": "tools/call",                                                  │
│    "params": {                                                              │
│      "name": "create_workload",                                             │
│      "arguments": {                                                         │
│        "name": "my-app",                                                    │
│        "replicas": 3,                                                       │
│        "image": "nginx:latest",                                             │
│        "cpu_cores": 0.5,                                                    │
│        "memory_mb": 512                                                     │
│      }                                                                      │
│    },                                                                       │
│    "id": 1                                                                  │
│  }                                                                          │
│                                                                             │
│  Response:                                                                  │
│  {                                                                          │
│    "jsonrpc": "2.0",                                                        │
│    "result": {                                                              │
│      "content": [{                                                          │
│        "type": "text",                                                      │
│        "text": "{\"workload_id\":\"...\",\"name\":\"my-app\",...}"         │
│      }]                                                                     │
│    },                                                                       │
│    "id": 1                                                                  │
│  }                                                                          │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Observability Stack

### Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         OBSERVABILITY STACK                                 │
│                           observability/src/                                │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                                                                      │   │
│  │  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐  │   │
│  │  │ Tracing  │ │ Metrics  │ │  Health  │ │  Events  │ │ Network  │  │   │
│  │  │          │ │          │ │          │ │          │ │  Bridge  │  │   │
│  │  └────┬─────┘ └────┬─────┘ └────┬─────┘ └────┬─────┘ └────┬─────┘  │   │
│  │       │            │            │            │            │         │   │
│  │       ▼            ▼            ▼            ▼            ▼         │   │
│  │  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐  │   │
│  │  │ Console  │ │Prometheus│ │  HTTP    │ │WebSocket │ │  P2P     │  │   │
│  │  │ + File   │ │ Exporter │ │Endpoints │ │ Server   │ │ Gossip   │  │   │
│  │  └──────────┘ └──────────┘ └──────────┘ └──────────┘ └──────────┘  │   │
│  │                                                                      │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Metrics

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         PROMETHEUS METRICS                                  │
│                          observability/src/metrics.rs                       │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  CONTAINER METRICS                                                          │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  containers_total              Counter    Total containers created   │   │
│  │  container_creation_errors     Counter    Creation failures          │   │
│  │  container_run_duration        Histogram  Container runtime duration │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  WORKLOAD METRICS                                                           │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  workloads_total               Counter    Total workloads created    │   │
│  │  instances_total               Gauge      Current instance count     │   │
│  │  instances_by_status           Gauge      Instances per status       │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  CLUSTER METRICS                                                            │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  nodes_total                   Gauge      Total nodes in cluster     │   │
│  │  nodes_ready                   Gauge      Ready nodes count          │   │
│  │  cluster_capacity_cpu          Gauge      Total CPU capacity         │   │
│  │  cluster_allocatable_cpu       Gauge      Allocatable CPU            │   │
│  │  cluster_allocatable_memory    Gauge      Allocatable memory         │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  SCHEDULER METRICS                                                          │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  schedule_attempts             Counter    Scheduling attempts        │   │
│  │  schedule_successes            Counter    Successful placements      │   │
│  │  schedule_failures             Counter    Failed placements          │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  RECONCILIATION METRICS                                                     │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  reconciliation_cycles         Counter    Reconciliation runs        │   │
│  │  reconciliation_duration       Histogram  Cycle duration             │   │
│  │  reconciliation_errors         Counter    Failed reconciliations     │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Health Checks

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                          HEALTH CHECKS                                      │
│                        observability/src/health.rs                          │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  HTTP ENDPOINTS (Kubernetes-compatible)                                     │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  GET /health        Full health status (all components)             │   │
│  │  GET /healthz       Kubernetes health check (brief)                 │   │
│  │  GET /live          Liveness probe (is process alive?)              │   │
│  │  GET /ready         Readiness probe (ready to serve?)               │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  HEALTH STATUS MODEL                                                        │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  HealthStatus                                                        │   │
│  │  ├── Healthy                   All systems operational              │   │
│  │  ├── Degraded { reason }       Partial functionality                │   │
│  │  └── Unhealthy { reason }      Service unavailable                  │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  COMPONENT TRACKING                                                         │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  ComponentHealth {                                                   │   │
│  │      status: HealthStatus,                                          │   │
│  │      last_check: Instant,                                           │   │
│  │      check_count: u64,                                              │   │
│  │  }                                                                   │   │
│  │                                                                      │   │
│  │  Components tracked:                                                 │   │
│  │  • state_store                                                       │   │
│  │  • cluster_manager                                                   │   │
│  │  • container_runtime                                                 │   │
│  │  • scheduler                                                         │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### WebSocket Event Streaming

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                       WEBSOCKET EVENT STREAMING                             │
│                 observability/src/events.rs + websocket.rs                  │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  CONNECTION FLOW                                                            │
│                                                                             │
│   Client                                Server                              │
│     │                                     │                                 │
│     │──── WebSocket Connect ─────────────▶│                                 │
│     │                                     │                                 │
│     │──── Subscribe Message ─────────────▶│                                 │
│     │     {"type": "subscribe",           │                                 │
│     │      "topics": ["nodes","workloads"]}│                                │
│     │                                     │                                 │
│     │◀──── StreamEvent ──────────────────│                                 │
│     │     {"type": "node_added",          │                                 │
│     │      "timestamp": "...",            │                                 │
│     │      "data": {...}}                 │                                 │
│     │                                     │                                 │
│     │◀──── StreamEvent ──────────────────│                                 │
│     │◀──── StreamEvent ──────────────────│                                 │
│     │               ...                   │                                 │
│                                                                             │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  EVENT TOPICS                                                               │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  Workloads    Create, update, delete, scale events                  │   │
│  │  Nodes        Add, remove, status change events                     │   │
│  │  Cluster      Health, capacity change events                        │   │
│  │  All          All events combined                                   │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  P2P NETWORK TOPICS (via gossipsub)                                         │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  Gradient     Resource availability signals                         │   │
│  │  Election     Leader election consensus                             │   │
│  │  Credit       Economic transactions                                 │   │
│  │  Septal       Coordination barriers                                 │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Configuration & Identity

### User Configuration

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                       USER CONFIGURATION                                    │
│                          user_config/src/                                   │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  CONFIG DIRECTORY: ~/.config/univrs/ (XDG-compliant)                        │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                        Identity                                      │   │
│  │                     (identity.rs)                                    │   │
│  ├─────────────────────────────────────────────────────────────────────┤   │
│  │                                                                      │   │
│  │  struct Identity {                                                   │   │
│  │      id: String,              // Derived from public key            │   │
│  │      signing_key: SigningKey, // Ed25519 private key                │   │
│  │      display_name: Option<String>,                                  │   │
│  │      email: Option<String>,                                         │   │
│  │      created_at: DateTime<Utc>,                                     │   │
│  │  }                                                                   │   │
│  │                                                                      │   │
│  │  Methods:                                                            │   │
│  │  • generate()              Create new random identity               │   │
│  │  • sign(&[u8]) -> [u8;64]  Sign a message                          │   │
│  │  • verify(msg, sig) -> bool   Verify signature                     │   │
│  │  • public_key() -> PublicKey  Get public key                       │   │
│  │                                                                      │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                      Trust Policy                                    │   │
│  │                    (trust_policy.rs)                                 │   │
│  ├─────────────────────────────────────────────────────────────────────┤   │
│  │                                                                      │   │
│  │  struct TrustPolicy {                                                │   │
│  │      network: NetworkPolicy,        // Outbound access rules        │   │
│  │      filesystem: FilesystemPolicy,  // File read/write rules        │   │
│  │      execution: ExecutionPolicy,    // Command execution rules      │   │
│  │      trusted_identities: Vec<TrustedIdentity>,                      │   │
│  │      approval: ApprovalPolicy,                                      │   │
│  │  }                                                                   │   │
│  │                                                                      │   │
│  │  Presets:                                                            │   │
│  │  • restrictive()  Paranoid mode (deny-by-default)                   │   │
│  │  • permissive()   Development mode (allow-by-default)               │   │
│  │                                                                      │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                       Encryption                                     │   │
│  │                     (encryption.rs)                                  │   │
│  ├─────────────────────────────────────────────────────────────────────┤   │
│  │                                                                      │   │
│  │  Age-based encryption for secrets                                   │   │
│  │                                                                      │   │
│  │  struct SecretStore {                                                │   │
│  │      key: age::secrecy::Secret<String>,                             │   │
│  │      secrets_dir: PathBuf,                                          │   │
│  │  }                                                                   │   │
│  │                                                                      │   │
│  │  Methods:                                                            │   │
│  │  • store_secret(name, value)                                        │   │
│  │  • get_secret(name) -> Option<String>                               │   │
│  │  • delete_secret(name)                                              │   │
│  │  • list_secrets() -> Vec<String>                                    │   │
│  │                                                                      │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Scheduler Architecture

### Filter-Score-Bind Pipeline

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                       SCHEDULER ARCHITECTURE                                │
│                        scheduler_interface/src/                             │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  THREE-PHASE PIPELINE                                                       │
│                                                                             │
│       All Nodes                                                             │
│           │                                                                 │
│           ▼                                                                 │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                         FILTER                                       │   │
│  │                       (filter.rs)                                    │   │
│  │                                                                      │   │
│  │  Eliminate ineligible nodes based on hard constraints:              │   │
│  │  • Node selectors (label matching)                                  │   │
│  │  • Node affinity (required scheduling rules)                        │   │
│  │  • Taints & tolerations                                             │   │
│  │  • Resource feasibility (CPU, memory, disk)                         │   │
│  │  • Topology constraints                                             │   │
│  │                                                                      │   │
│  └──────────────────────────┬──────────────────────────────────────────┘   │
│                             │                                               │
│                             ▼                                               │
│                    Eligible Nodes                                           │
│                             │                                               │
│                             ▼                                               │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                          SCORE                                       │   │
│  │                        (score.rs)                                    │   │
│  │                                                                      │   │
│  │  Rank eligible nodes based on soft constraints:                     │   │
│  │  • Weighted scoring functions                                       │   │
│  │  • Resource balance (bin-packing vs spreading)                      │   │
│  │  • Node affinity preferences                                        │   │
│  │  • Pod affinity/anti-affinity preferences                           │   │
│  │  • Custom scoring plugins                                           │   │
│  │                                                                      │   │
│  └──────────────────────────┬──────────────────────────────────────────┘   │
│                             │                                               │
│                             ▼                                               │
│                    Ranked Nodes                                             │
│                             │                                               │
│                             ▼                                               │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                           BIND                                       │   │
│  │                         (bind.rs)                                    │   │
│  │                                                                      │   │
│  │  Select top-ranked node:                                            │   │
│  │  • Conflict resolution                                              │   │
│  │  • Tie-breaking strategies                                          │   │
│  │  • Resource reservation                                             │   │
│  │                                                                      │   │
│  └──────────────────────────┬──────────────────────────────────────────┘   │
│                             │                                               │
│                             ▼                                               │
│                    Selected Node                                            │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Resource Model

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                          RESOURCE MODEL                                     │
│                    scheduler_interface/src/resources.rs                     │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  RESOURCE TYPES                                                             │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  Millicores(u64)    CPU measurement (1000m = 1 core)                │   │
│  │  Bytes(u64)         Memory measurement (Ki, Mi, Gi, Ti)             │   │
│  │  Units(u64)         Extended resources (GPUs, FPGAs)                │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  QOS CLASSES (Kubernetes-compatible)                                        │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                                                                      │   │
│  │  ┌────────────────┐                                                 │   │
│  │  │   Guaranteed   │  requests == limits (all resources)             │   │
│  │  │    (highest)   │  Highest scheduling priority                    │   │
│  │  └────────────────┘                                                 │   │
│  │          │                                                          │   │
│  │          ▼                                                          │   │
│  │  ┌────────────────┐                                                 │   │
│  │  │   Burstable    │  requests < limits OR partial specification     │   │
│  │  │    (medium)    │  Can burst beyond requests up to limits         │   │
│  │  └────────────────┘                                                 │   │
│  │          │                                                          │   │
│  │          ▼                                                          │   │
│  │  ┌────────────────┐                                                 │   │
│  │  │   BestEffort   │  No requests or limits specified                │   │
│  │  │    (lowest)    │  First to be evicted under pressure             │   │
│  │  └────────────────┘                                                 │   │
│  │                                                                      │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Data Flow Patterns

### Workload Creation Flow

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                      WORKLOAD CREATION FLOW                                 │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ┌──────────┐                                                              │
│  │  Client  │                                                              │
│  └────┬─────┘                                                              │
│       │ POST /api/v1/workloads                                             │
│       │ (signed with Ed25519)                                              │
│       ▼                                                                     │
│  ┌──────────────────────────────────────────────────────────────────────┐  │
│  │                      REST API Layer                                   │  │
│  │                                                                       │  │
│  │  1. Auth middleware validates signature                              │  │
│  │  2. Parse CreateWorkloadRequest                                      │  │
│  │  3. Generate WorkloadId (UUID)                                       │  │
│  │  4. Send to workload_tx channel                                      │  │
│  │                                                                       │  │
│  └────────────────────────────────┬─────────────────────────────────────┘  │
│                                   │                                         │
│                                   ▼  mpsc channel                           │
│  ┌──────────────────────────────────────────────────────────────────────┐  │
│  │                   Orchestrator Main Loop                              │  │
│  │                                                                       │  │
│  │  tokio::select! {                                                    │  │
│  │      workload = workload_rx.recv() => {                              │  │
│  │          handle_workload_update(workload)                            │  │
│  │      }                                                                │  │
│  │  }                                                                    │  │
│  │                                                                       │  │
│  └────────────────────────────────┬─────────────────────────────────────┘  │
│                                   │                                         │
│                                   ▼                                         │
│  ┌──────────────────────────────────────────────────────────────────────┐  │
│  │                     handle_workload_update()                          │  │
│  │                                                                       │  │
│  │  1. state_store.put_workload(definition)    ← Persist                │  │
│  │  2. reconcile_workload(definition)          ← Reconcile              │  │
│  │                                                                       │  │
│  └────────────────────────────────┬─────────────────────────────────────┘  │
│                                   │                                         │
│                                   ▼                                         │
│  ┌──────────────────────────────────────────────────────────────────────┐  │
│  │                      reconcile_workload()                             │  │
│  │                                                                       │  │
│  │  SENSE:   Get current instances from state store                     │  │
│  │  COMPARE: Determine if scale up/down needed                          │  │
│  │  PLAN:    Generate actions (create/remove instances)                 │  │
│  │  ACTUATE: Execute via scheduler + container runtime                  │  │
│  │                                                                       │  │
│  └────────────────────────────────┬─────────────────────────────────────┘  │
│                                   │                                         │
│         ┌─────────────────────────┼─────────────────────────┐              │
│         │                         │                         │              │
│         ▼                         ▼                         ▼              │
│  ┌─────────────┐          ┌─────────────┐          ┌─────────────┐        │
│  │  Scheduler  │          │  Container  │          │    State    │        │
│  │             │          │   Runtime   │          │    Store    │        │
│  │  Decide     │          │  Create     │          │   Save      │        │
│  │  placement  │          │  containers │          │  instances  │        │
│  └─────────────┘          └─────────────┘          └─────────────┘        │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Node Join Flow

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                          NODE JOIN FLOW                                     │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ┌─────────────┐                                                           │
│  │  New Node   │                                                           │
│  │  (youki)    │                                                           │
│  └──────┬──────┘                                                           │
│         │ Start chitchat, connect to seeds                                 │
│         ▼                                                                   │
│  ┌──────────────────────────────────────────────────────────────────────┐  │
│  │                     Chitchat Gossip Layer                             │  │
│  │                                                                       │  │
│  │  • Node publishes metadata to key-value store                        │  │
│  │  • State propagates via gossip protocol                              │  │
│  │  • Phi-accrual failure detection monitors liveness                   │  │
│  │                                                                       │  │
│  └────────────────────────────────┬─────────────────────────────────────┘  │
│                                   │                                         │
│                                   ▼  live_nodes_watcher                     │
│  ┌──────────────────────────────────────────────────────────────────────┐  │
│  │                  ChitchatClusterManager                               │  │
│  │                                                                       │  │
│  │  Membership monitor detects new node:                                │  │
│  │  • Parse node metadata from chitchat state                           │  │
│  │  • Build Node struct                                                 │  │
│  │  • Broadcast ClusterEvent::NodeAdded                                 │  │
│  │                                                                       │  │
│  └────────────────────────────────┬─────────────────────────────────────┘  │
│                                   │                                         │
│                                   ▼  broadcast channel                      │
│  ┌──────────────────────────────────────────────────────────────────────┐  │
│  │                   Orchestrator Main Loop                              │  │
│  │                                                                       │  │
│  │  tokio::select! {                                                    │  │
│  │      Ok(ClusterEvent::NodeAdded(node)) = events_rx.recv() => {       │  │
│  │          handle_cluster_event(event)                                 │  │
│  │      }                                                                │  │
│  │  }                                                                    │  │
│  │                                                                       │  │
│  └────────────────────────────────┬─────────────────────────────────────┘  │
│                                   │                                         │
│                                   ▼                                         │
│  ┌──────────────────────────────────────────────────────────────────────┐  │
│  │                    handle_cluster_event()                             │  │
│  │                                                                       │  │
│  │  1. Check if node exists in state store                              │  │
│  │  2. If new and Ready: runtime.init_node(node_id)                     │  │
│  │  3. state_store.put_node(node)                                       │  │
│  │  4. reconcile_all_workloads()  ← May reschedule                      │  │
│  │                                                                       │  │
│  └──────────────────────────────────────────────────────────────────────┘  │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Feature Composition

### Feature Flags

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         FEATURE FLAGS                                       │
│                    orchestrator_core/Cargo.toml                             │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  [features]                                                                 │
│  default = []                                                               │
│                                                                             │
│  INDIVIDUAL FEATURES                                                        │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  cluster        Chitchat gossip-based clustering                    │   │
│  │  runtime        Mock container runtime                              │   │
│  │  youki-runtime  Real Youki OCI runtime                              │   │
│  │  observability  Metrics, tracing, health checks                     │   │
│  │  rest-api       HTTP API with Ed25519 auth                          │   │
│  │  mcp            Model Context Protocol server                       │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  PROFILE COMBINATIONS                                                       │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                                                                      │   │
│  │  full = [                                                           │   │
│  │      "cluster",                                                     │   │
│  │      "runtime",        ← Mock runtime for dev/test                  │   │
│  │      "observability",                                               │   │
│  │      "rest-api",                                                    │   │
│  │      "mcp"                                                          │   │
│  │  ]                                                                   │   │
│  │                                                                      │   │
│  │  full-youki = [                                                     │   │
│  │      "cluster",                                                     │   │
│  │      "youki-runtime",  ← Real runtime for production                │   │
│  │      "observability",                                               │   │
│  │      "rest-api",                                                    │   │
│  │      "mcp"                                                          │   │
│  │  ]                                                                   │   │
│  │                                                                      │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  BUILD COMMANDS                                                             │
│                                                                             │
│  # Development (mock runtime)                                               │
│  cargo run --bin orchestrator_daemon --features full                       │
│                                                                             │
│  # Production (real containers, Linux only)                                 │
│  cargo run --bin orchestrator_node --features full-youki                   │
│                                                                             │
│  # Minimal (no optional features)                                           │
│  cargo build --workspace                                                    │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Component Composition

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                      COMPONENT COMPOSITION                                  │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│                        ┌─────────────────────┐                             │
│                        │  orchestrator_core  │                             │
│                        │    (Main Engine)    │                             │
│                        └──────────┬──────────┘                             │
│                                   │                                         │
│         ┌─────────────────────────┼─────────────────────────┐              │
│         │                         │                         │              │
│         ▼                         ▼                         ▼              │
│  ┌─────────────┐          ┌─────────────┐          ┌─────────────┐        │
│  │   cluster   │          │   runtime   │          │  rest-api   │        │
│  │   feature   │          │   feature   │          │   feature   │        │
│  └──────┬──────┘          └──────┬──────┘          └──────┬──────┘        │
│         │                        │                        │                │
│         ▼                        ▼                        ▼                │
│  ┌─────────────┐          ┌─────────────┐          ┌─────────────┐        │
│  │  cluster_   │          │ container_  │          │ user_config │        │
│  │  manager    │          │  runtime    │          │    axum     │        │
│  │             │          │             │          │   tower     │        │
│  │ (chitchat)  │          │ (mock/youki)│          │   auth      │        │
│  └─────────────┘          └─────────────┘          └─────────────┘        │
│                                                                             │
│         ┌─────────────────────────┼─────────────────────────┐              │
│         │                         │                         │              │
│         ▼                         ▼                         ▼              │
│  ┌─────────────┐          ┌─────────────┐          ┌─────────────┐        │
│  │observability│          │     mcp     │          │  External   │        │
│  │   feature   │          │   feature   │          │    Deps     │        │
│  └──────┬──────┘          └──────┬──────┘          └─────────────┘        │
│         │                        │                                         │
│         ▼                        ▼                                         │
│  ┌─────────────┐          ┌─────────────┐                                  │
│  │observability│          │ mcp_server  │                                  │
│  │   crate     │          │   crate     │                                  │
│  │             │          │             │                                  │
│  │ • metrics   │          │ • tools     │                                  │
│  │ • tracing   │          │ • resources │                                  │
│  │ • health    │          │ • json-rpc  │                                  │
│  │ • events    │          │             │                                  │
│  │ • websocket │          │             │                                  │
│  └─────────────┘          └─────────────┘                                  │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Summary

This orchestrator represents a modern, AI-native approach to container orchestration with:

| Aspect | Design Choice |
|--------|---------------|
| **Language** | Rust for safety, performance, and correctness |
| **Architecture** | Trait-based DI for maximum flexibility |
| **Control Loop** | Classical Sense → Compare → Plan → Actuate |
| **Clustering** | Gossip-based with broadcast channels for consistency |
| **Identity** | Ed25519 self-sovereign (no central authority) |
| **API** | REST + MCP + WebSocket for humans and AI agents |
| **Observability** | Prometheus metrics, structured tracing, health checks |
| **Extensibility** | Feature flags for different deployment scenarios |

The system is designed for:
- **AI-driven operations** through MCP protocol
- **High availability** through stateless reconciliation
- **Testability** through trait-based mocking
- **Flexibility** through feature composition

---

*Generated: 2026-01-26*
*Version: 0.1.0*
