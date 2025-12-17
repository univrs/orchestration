# RustOrchestration: Complete Workspace Architecture Guide

## Table of Contents

1. [Workspace Structure Overview](#1-workspace-structure-overview)
2. [Sprint-by-Sprint Workspace Evolution](#2-sprint-by-sprint-workspace-evolution)
3. [Cargo.toml Patterns Explained](#3-cargotoml-patterns-explained)
4. [Feature Flags & Conditional Compilation](#4-feature-flags--conditional-compilation)
5. [Dependency Management Best Practices](#5-dependency-management-best-practices)
6. [Module Organization Patterns](#6-module-organization-patterns)
7. [Error Handling Architecture](#7-error-handling-architecture)
8. [Async Runtime Patterns](#8-async-runtime-patterns)
9. [Trait-Based Design Philosophy](#9-trait-based-design-philosophy)
10. [Testing Strategies](#10-testing-strategies)
11. [Build Optimization](#11-build-optimization)

---

## 1. Workspace Structure Overview

### What is a Rust Workspace?

A Rust workspace is a set of packages (crates) that share:
- A common `Cargo.lock` file (consistent dependency versions)
- A shared output directory (`target/`)
- Common settings and dependency versions

**Why use workspaces?**
- **Compile-time savings**: Shared dependencies compiled once
- **Version consistency**: All crates use same dependency versions
- **Logical separation**: Clear boundaries between components
- **Parallel compilation**: Independent crates build in parallel

### Current vs Target Structure

```
ai_native_orchestrator/                    # WORKSPACE ROOT
│
├── Cargo.toml                             # Workspace manifest
├── Cargo.lock                             # Shared lockfile (auto-generated)
├── rust-toolchain.toml                    # Rust version pinning (optional)
│
│   ╔═══════════════════════════════════════════════════════════════════╗
│   ║                    INTERFACE CRATES (Traits)                       ║
│   ║   These define contracts - no implementations, just signatures     ║
│   ╚═══════════════════════════════════════════════════════════════════╝
│
├── orchestrator_shared_types/             # Common types across all crates
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── workload.rs                    # WorkloadDefinition, WorkloadInstance
│       ├── node.rs                        # Node, NodeId, NodeStatus
│       └── error.rs                       # Common error types
│
├── state_store_interface/                 # StateStore trait + InMemory impl
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs                         # pub trait StateStore
│       ├── in_memory.rs                   # InMemoryStateStore (default)
│       └── error.rs                       # StateStoreError
│
├── container_runtime_interface/           # ContainerRuntime trait
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs                         # pub trait ContainerRuntime
│       └── types.rs                       # ContainerSpec, ContainerId
│
├── cluster_manager_interface/             # ClusterManager trait
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs                         # pub trait ClusterManager
│       └── types.rs                       # MembershipEvent, NodeInfo
│
├── scheduler_interface/                   # Scheduler trait
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs                         # pub trait Scheduler
│       └── types.rs                       # SchedulingDecision, Placement
│
│   ╔═══════════════════════════════════════════════════════════════════╗
│   ║                 IMPLEMENTATION CRATES (Concrete)                   ║
│   ║   These implement the traits with specific technologies            ║
│   ╚═══════════════════════════════════════════════════════════════════╝
│
├── state_store_etcd/                      # Sprint 1: etcd backend
│   ├── Cargo.toml
│   └── src/
│       └── lib.rs                         # EtcdStateStore
│
├── container_runtime/                     # Sprint 2: Youki + Mock
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── mock.rs                        # MockContainerRuntime
│       └── youki.rs                       # YoukiRuntime (feature-gated)
│
├── cluster_manager/                       # Sprint 2: Chitchat gossip
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── mock.rs                        # MockClusterManager
│       └── chitchat.rs                    # ChitchatCluster (feature-gated)
│
├── scheduler/                             # Sprint 3: Mycelial scheduler
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── simple.rs                      # SimpleScheduler (default)
│       └── mycelial.rs                    # MycelialScheduler (credit-based)
│
│   ╔═══════════════════════════════════════════════════════════════════╗
│   ║                    APPLICATION CRATES (Binaries)                   ║
│   ║   These wire everything together and produce executables           ║
│   ╚═══════════════════════════════════════════════════════════════════╝
│
├── orchestrator_core/                     # Main orchestration logic
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs                         # Core logic as library
│       ├── main.rs                        # Binary entry point
│       ├── reconciler.rs                  # Reconciliation loop
│       └── api.rs                         # gRPC/REST API handlers
│
├── mcp_server/                            # Sprint 3: AI-native MCP
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── main.rs
│       └── tools/
│           ├── mod.rs
│           ├── workload.rs                # workload_create, workload_scale
│           ├── cluster.rs                 # cluster_status, node_drain
│           └── scheduler.rs               # scheduler_hint
│
├── orchestrator_cli/                      # Sprint 4: CLI tool
│   ├── Cargo.toml
│   └── src/
│       └── main.rs                        # clap-based CLI
│
│   ╔═══════════════════════════════════════════════════════════════════╗
│   ║                    SUPPORTING DIRECTORIES                          ║
│   ╚═══════════════════════════════════════════════════════════════════╝
│
├── tests/                                 # Workspace-level integration tests
│   ├── integration/
│   │   ├── orchestrator_e2e.rs
│   │   └── mcp_integration.rs
│   └── chaos/
│       └── fault_injection.rs
│
├── benches/                               # Performance benchmarks
│   └── reconciliation_bench.rs
│
├── examples/                              # Usage examples
│   ├── simple_deployment.rs
│   └── multi_node_cluster.rs
│
└── target/                                # Shared build output (git-ignored)
    ├── debug/
    └── release/
```

---

## 2. Sprint-by-Sprint Workspace Evolution

### Sprint 1: Foundation Hardening (Current)

```
[workspace]
members = [
    "orchestrator_core",
    "orchestrator_shared_types",
    "container_runtime_interface",
    "cluster_manager_interface",
    "scheduler_interface",
    "state_store_interface",      # ← Focus: Add tests here
]
```

**Goals:**
- Fix state_store integration in orchestrator_core
- Add comprehensive tests to state_store_interface
- Standardize error handling across crates

**Visual: Sprint 1 Dependency Graph**
```
┌─────────────────────────────────────────────────────────────┐
│                     orchestrator_core                        │
│                     (main binary)                            │
└───────────┬───────────┬───────────┬───────────┬─────────────┘
            │           │           │           │
            ▼           ▼           ▼           ▼
   ┌────────────┐ ┌──────────┐ ┌──────────┐ ┌────────────────┐
   │ scheduler_ │ │ cluster_ │ │container_│ │ state_store_   │
   │ interface  │ │ manager_ │ │ runtime_ │ │ interface      │
   │            │ │interface │ │interface │ │ + InMemory     │
   └─────┬──────┘ └────┬─────┘ └────┬─────┘ └───────┬────────┘
         │             │            │               │
         └─────────────┴────────────┴───────────────┘
                              │
                              ▼
                 ┌────────────────────────┐
                 │ orchestrator_shared_   │
                 │ types                  │
                 │ (Node, Workload, etc.) │
                 └────────────────────────┘
```

---

### Sprint 2: Runtime Integration

```
[workspace]
members = [
    # ... existing ...
    "container_runtime",          # ← NEW: Youki + Mock
    "cluster_manager",            # ← NEW: Chitchat + Mock
    "state_store_etcd",           # ← NEW: etcd backend
]
```

**Visual: Sprint 2 Additions**
```
┌─────────────────────────────────────────────────────────────┐
│                     orchestrator_core                        │
└───────────┬───────────┬───────────┬───────────┬─────────────┘
            │           │           │           │
            ▼           ▼           ▼           ▼
   ┌────────────┐ ┌──────────┐ ┌──────────┐ ┌────────────────┐
   │ scheduler_ │ │ cluster_ │ │container_│ │ state_store_   │
   │ interface  │ │ manager_ │ │ runtime_ │ │ interface      │
   └─────┬──────┘ │interface │ │interface │ └───────┬────────┘
         │        └────┬─────┘ └────┬─────┘         │
         │             │            │               │
         │        ┌────┴─────┐ ┌────┴─────┐    ┌────┴────────┐
         │        │ cluster_ │ │container_│    │state_store_ │
         │        │ manager  │ │ runtime  │    │   etcd      │
         │        │┌────────┐│ │┌────────┐│    └─────────────┘
         │        ││Chitchat││ ││  Youki ││
         │        │└────────┘│ │└────────┘│
         │        └──────────┘ └──────────┘
         │              ▲            ▲
         │              └────────────┘
         │            Feature-gated impls
         ▼
```

---

### Sprint 3: AI-Native Features

```
[workspace]
members = [
    # ... existing ...
    "mcp_server",                 # ← NEW: MCP protocol server
    "scheduler",                  # ← NEW: Mycelial scheduler
]
```

**Visual: Sprint 3 MCP Integration**
```
                    ┌─────────────────────┐
                    │   Claude / AI Agent │
                    └──────────┬──────────┘
                               │ MCP Protocol
                               ▼
                    ┌─────────────────────┐
                    │     mcp_server      │
                    │  ┌───────────────┐  │
                    │  │ Tools:        │  │
                    │  │ - workload_*  │  │
                    │  │ - cluster_*   │  │
                    │  │ - scheduler_* │  │
                    │  └───────────────┘  │
                    └──────────┬──────────┘
                               │
                               ▼
                    ┌─────────────────────┐
                    │  orchestrator_core  │
                    └─────────────────────┘
```

---

### Sprint 4: Production Readiness

```
[workspace]
members = [
    # ... all previous ...
    "orchestrator_cli",           # ← NEW: CLI tool
    "consensus",                  # ← NEW: OpenRaft integration
]
```

**Visual: Complete Production Architecture**
```
┌──────────────────────────────────────────────────────────────────────┐
│                           ENTRY POINTS                                │
├──────────────┬──────────────────┬────────────────────────────────────┤
│orchestrator_ │   mcp_server     │        orchestrator_cli            │
│   core       │  (AI interface)  │        (human interface)           │
│  (daemon)    │                  │                                    │
└──────┬───────┴────────┬─────────┴──────────────┬─────────────────────┘
       │                │                        │
       └────────────────┼────────────────────────┘
                        │
       ┌────────────────┼────────────────────────────────────┐
       │                ▼                                     │
       │     ┌─────────────────────┐                         │
       │     │     consensus       │  OpenRaft for leader    │
       │     │   (distributed)     │  election & state sync  │
       │     └─────────────────────┘                         │
       │                                                     │
├──────┴─────────────────────────────────────────────────────┤
│                    CORE ABSTRACTIONS                        │
├─────────────┬─────────────┬─────────────┬──────────────────┤
│ scheduler   │cluster_mgr  │container_   │   state_store    │
│ _interface  │_interface   │runtime_intf │   _interface     │
├─────────────┼─────────────┼─────────────┼──────────────────┤
│             │             │             │                  │
│IMPLEMENTATIONS (feature-gated)                             │
├─────────────┼─────────────┼─────────────┼──────────────────┤
│ scheduler/  │cluster_mgr/ │container_   │ state_store_     │
│ ├─simple    │├─mock       │runtime/     │ ├─in_memory      │
│ └─mycelial  │└─chitchat   │├─mock       │ └─etcd           │
│             │             │└─youki      │                  │
└─────────────┴─────────────┴─────────────┴──────────────────┘
                        │
                        ▼
            ┌─────────────────────┐
            │orchestrator_shared_ │
            │       types         │
            └─────────────────────┘
```

---

## 3. Cargo.toml Patterns Explained

### Workspace Root Cargo.toml

```toml
# ai_native_orchestrator/Cargo.toml

[workspace]
# List all crates in the workspace
# Order doesn't matter for building, but affects display in `cargo metadata`
members = [
    "orchestrator_core",
    "orchestrator_shared_types",
    "container_runtime_interface",
    "cluster_manager_interface",
    "scheduler_interface",
    "state_store_interface",
]

# Rust 2021 edition resolver handles feature unification better
# Always use resolver = "2" for new projects
resolver = "2"

# ============================================================
# WORKSPACE DEPENDENCIES
# ============================================================
# Define versions ONCE here, reference with { workspace = true }
# Benefits:
#   1. Single source of truth for versions
#   2. Consistent versions across all crates
#   3. Easy upgrades (change one place)
#   4. Cargo.lock ensures reproducible builds

[workspace.dependencies]
# Async runtime - Tokio is the de facto standard
# "full" feature enables all sub-features; consider being more selective in production
tokio = { version = "1", features = ["full"] }

# Serialization - serde is ubiquitous in Rust
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# Error handling
thiserror = "1.0"      # For library crates (structured errors)
anyhow = "1.0"         # For application crates (contextual errors)

# Unique identifiers
uuid = { version = "1", features = ["v4", "serde"] }

# Observability - tracing is the async-aware logging solution
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

# Async traits - required until async fn in traits is stable
async-trait = "0.1.77"

# Runtime type checking for trait objects
downcast-rs = "1.2.1"

# ============================================================
# WORKSPACE METADATA (optional but recommended)
# ============================================================
[workspace.package]
version = "0.1.0"
edition = "2021"
license = "MIT OR Apache-2.0"
repository = "https://github.com/yourorg/RustOrchestration"
rust-version = "1.75"  # Minimum supported Rust version (MSRV)

# ============================================================
# WORKSPACE LINTS (Rust 1.74+)
# ============================================================
# Apply consistent lints across all crates
[workspace.lints.rust]
unsafe_code = "forbid"
missing_docs = "warn"

[workspace.lints.clippy]
all = "warn"
pedantic = "warn"
nursery = "warn"
# Allow certain patterns that are intentional
module_name_repetitions = "allow"
```

### Interface Crate Cargo.toml

```toml
# state_store_interface/Cargo.toml

[package]
name = "state_store_interface"
version.workspace = true          # Inherit from workspace
edition.workspace = true
license.workspace = true

# Crate-specific metadata
description = "Trait definitions for state storage backends"
keywords = ["orchestration", "state", "storage"]
categories = ["development-tools"]

# ============================================================
# DEPENDENCIES
# ============================================================
[dependencies]
# Reference workspace dependencies with { workspace = true }
# This uses the version defined in [workspace.dependencies]
tokio = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
uuid = { workspace = true }
async-trait = { workspace = true }

# Local crate dependencies use path
orchestrator_shared_types = { path = "../orchestrator_shared_types" }

# ============================================================
# DEV DEPENDENCIES
# ============================================================
# Only compiled for tests, examples, and benches
[dev-dependencies]
# Inherit version from workspace, but add test features
tokio = { workspace = true, features = ["test-util", "macros", "rt-multi-thread"] }
uuid = { workspace = true }

# Test-only crates (not in workspace.dependencies)
proptest = "1.4"                  # Property-based testing
test-case = "3.3"                 # Parameterized tests
criterion = "0.5"                 # Benchmarking (if you add benches)

# ============================================================
# FEATURES
# ============================================================
[features]
# Default features enabled when crate is used
default = ["in-memory"]

# Feature flag for in-memory implementation
in-memory = []

# Expose internals for testing (use carefully)
test-utils = []

# ============================================================
# LINTS
# ============================================================
[lints]
workspace = true  # Inherit workspace lints
```

### Implementation Crate Cargo.toml (with optional deps)

```toml
# container_runtime/Cargo.toml

[package]
name = "container_runtime"
version.workspace = true
edition.workspace = true

[dependencies]
# Always required
container_runtime_interface = { path = "../container_runtime_interface" }
orchestrator_shared_types = { path = "../orchestrator_shared_types" }
tokio = { workspace = true }
async-trait = { workspace = true }
thiserror = { workspace = true }
tracing = { workspace = true }

# ============================================================
# OPTIONAL DEPENDENCIES
# ============================================================
# Only compiled when feature is enabled
# Reduces compile time and binary size when not needed

# Youki OCI runtime (Linux only)
youki = { version = "0.3", optional = true }
libcontainer = { version = "0.3", optional = true }

# ============================================================
# FEATURES
# ============================================================
[features]
# By default, only include mock runtime (fast to compile, works everywhere)
default = ["mock-runtime"]

# Mock implementation for testing
mock-runtime = []

# Real Youki integration (Linux only, slower to compile)
youki-runtime = ["dep:youki", "dep:libcontainer"]

# Enable all runtimes
full = ["mock-runtime", "youki-runtime"]

# ============================================================
# TARGET-SPECIFIC DEPENDENCIES
# ============================================================
# Only compile youki on Linux (it doesn't work on macOS/Windows)
[target.'cfg(target_os = "linux")'.dependencies]
youki = { version = "0.3", optional = true }
libcontainer = { version = "0.3", optional = true }

[dev-dependencies]
tokio = { workspace = true, features = ["test-util", "macros", "rt-multi-thread"] }
tempfile = "3"  # For creating temp directories in tests
```

### Binary Crate Cargo.toml

```toml
# orchestrator_core/Cargo.toml

[package]
name = "orchestrator_core"
version.workspace = true
edition.workspace = true

[dependencies]
# All interface crates
state_store_interface = { path = "../state_store_interface" }
container_runtime_interface = { path = "../container_runtime_interface" }
cluster_manager_interface = { path = "../cluster_manager_interface" }
scheduler_interface = { path = "../scheduler_interface" }
orchestrator_shared_types = { path = "../orchestrator_shared_types" }

# Implementation crates (feature-gated)
container_runtime = { path = "../container_runtime", optional = true }
cluster_manager = { path = "../cluster_manager", optional = true }
state_store_etcd = { path = "../state_store_etcd", optional = true }

# Core dependencies
tokio = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
anyhow = { workspace = true }       # Note: anyhow for binary, thiserror for libs
tracing = { workspace = true }
tracing-subscriber = { workspace = true }
async-trait = { workspace = true }

# API server
axum = "0.7"
tower = "0.4"
tower-http = { version = "0.5", features = ["trace"] }

[features]
default = ["mock-backends"]

# Development: use all mock implementations
mock-backends = [
    "container_runtime/mock-runtime",
    "cluster_manager/mock",
    "state_store_interface/in-memory",
]

# Production: use real implementations
production = [
    "container_runtime/youki-runtime",
    "cluster_manager/chitchat",
    "state_store_etcd",
]

# ============================================================
# BINARY CONFIGURATION
# ============================================================
[[bin]]
name = "orchestrator"
path = "src/main.rs"

# Also expose as library for integration tests
[lib]
name = "orchestrator_core"
path = "src/lib.rs"

[dev-dependencies]
tokio = { workspace = true, features = ["test-util", "macros", "rt-multi-thread"] }
```

---

## 4. Feature Flags & Conditional Compilation

### Why Feature Flags?

```
┌─────────────────────────────────────────────────────────────────────┐
│                    COMPILE-TIME DECISIONS                            │
├──────────────────────────────────────────────────────────────â────┤
│                                                                      │
│   cargo build                    cargo build --features youki        │
│   (default features)             (with youki feature)                │
│                                                                      │
│   ┌─────────────────┐            ┌─────────────────┐                │
│   │ container_      │          │ container_      │                │
│   │ runtime         │            │ runtime         │                │
│   │                 │            │                 │                │
│   │ ┌─────────────┐ │            │ ┌─────────────┐ │                │
│   │ │MockRuntime  │ │            │ │MockRuntime  │ │                │
│   │ └─────────────┘ │└─────────────┘ │                │
│   │                 │            │ ┌─────────────┐ │                │
│   │                 │            │ │YoukiRuntime │◄──── Compiled!   │
│   │                 │            │ └─────────────┘ │                │
│   └─────────────────┘            └─────────────â                │
│                                                                      │
│   Binary: 5 MB                   Binary: 12 MB                       │
│   Compile: 30s                   Compile: 2m                         │
│   Platforms: All                 Platforms: Linux only               │
│                                                                      │
└─────────────────────────────────â─────────────────────┘
```

### Feature Flag Implementation Pattern

```rust
// container_runtime/src/lib.rs

// Always available
mod mock;
pub use mock::MockContainerRuntime;

// Conditionally compiled
#[cfg(feature = "youki-runtime")]
mod youki;
#[cfg(feature = "youki-runtime")]
pub use youki::YoukiRuntime;

// Factory function that uses features
pub fn create_runtime(config: &RuntimeConfig) -> Box<dyn ContainerRuntime> {
    match config.backend {
        RuMock => Box::new(MockContainerRuntime::new()),
        
        #[cfg(feature = "youki-runtime")]
        RuntimeBackend::Youki => Box::new(YoukiRuntime::new(&config.youki_config)),
        
        #[cfg(not(feature = "youki-runtime"))]
        RuntimeBackend::Youki => {
            panic!("Youki runtime not compiled. Rebuild with --features youki-runtime")
        }
    }
}
```

### Additive Features Rule

```
╔═══════════════════════════════════════════════════════════════════════╗
║  RULE: Features must be ADDITIVE                                       ║
║                                                                        ║
║  ✅ GOOD: Feature adds functionality                                   ║
║     #[cfg(feature = "metrics")]                                        ║
║     fn record_metrics() { ... }                                    ║                                                                        ║
║  ❌ BAD: Feature changes behavior                                      ║
║     #[cfg(feature = "fast-mode")]                                      ║
║     fn process() { /* skip validation */ }                             ║
║     #[cfg(not(feature = "fast-mode"))]                                 ║
║     fn process() { /* with validation */ }                             ║
║                                                            ║
║  Why? If crate A uses feature X and crate B doesn't, Cargo enables    ║
║  X for both. Subtractive features would break crate B unexpectedly.   ║
╚═══════════════════════════════════════════════════════════════════════╝
```

---

## 5. Dependency Management Best Practices

### Dependency Categories

```
┌────────────────────────────────────────────────────────────┐
│                     DEPENDENCY CLASSIFICATION                         │
├──────────────────┬───────────────────────────────────────────────────┤
│ Category         │ Examples & Guidelines                        │
├──────────────────┼───────────────────────────────────────────────────┤
│ Runtime          │ tokio, serde, tracing                             │
│ Essential        │ Always needed at runtime                          │
│                  │ → Put in [dependencies]                           │
├──────────────────────────────────────────────────────────┤
│ Optional         │ youki, etcd-client, openraft                      │
│ Features         │ Only needed for specific features                  │
│                  │ → Put in [dependencies] with optional = true      │
├──────────────────┼─────────────────────â────────────────────┤
│ Build-time       │ prost-build, tonic-build                          │
│ Only             │ Code generation, proc macros for build            │
│                  │ → Put in [build-dependencies]                     │
├──────────────────┼────────────────────────────────────────────────âing          │ proptest, criterion, tempfile, testcontainers     │
│ Only             │ Never needed in production binary                  │
│                  │ → Put in [dev-dependencies]                       │
├──────────────────┼───────────────────────────────────────────────────┤
│ Platform         │ nix (Unix only), windows-sys (Windows       │
│ Specific         │ Only on certain OS/arch                           │
│                  │ → Use [target.'cfg(...)'.dependencies]            │
└──────────────────┴───────────────────────────────────────────────────┘
```

### Workspace Dependency Inheritance

```toml
# ROOT Cargo.toml
[workspace.dependencies]
tokio = { version = "1.35", features = ["full"] }

# CRATE Cargo.toml - Multiple ways to use workspace deps:

[dependencies]
# 1. Simple inheritance (exact same config as workspace)
tokio = { workspace = true }

# 2. Add features on top of workspace config
tokio = { workspace = true, features = ["test-util"] }

# 3. Make it optional in this crate
tokio = { workspace = true, optional = true }

# 4. Combine
tokio = { workspace = true, features = ["test-util"], optional = true }
```

### Version Specification Best Practices

```toml
[workspace.dependencies]
# ✅ GOOD: Specify minimum version, allow compatible updates
serde = "1.0"           # Equivalent to "^1.0" (>=1.0.0 <2.0.0)

# ✅ GOOD: Pin when API stability matters
tokio = "1.35"          # Will use 1.35.x

# ⚠️ CAREFUL: Exact version (rarely needed)
critical-lib = "=1.2.3" # Exactly this version only

# ✅ GOOD: For pre-1.0 crates (0.x.y has different semver rules)
new-crate = "0.3"       # ^0.3 means >=0.3.0 <0.4.0

# ✅ GOOD: Minimum version for security fix
vulnerable-crat"

# ❌ AVOID: Wildcard versions
bad-crate = "*"         # Any version - unpredictable!
```

---

## 6. Module Organization Patterns

### The Rust Module System

```
┌──────────────────────────────────────────────────────────────────────┐
│                         MODULE HIERARCHY                              │
├───────────────â──────────────────────────────────────────────┤
│                                                                       │
│   crate (lib.rs or main.rs)                                          │
│      │                                                                │
│      ├── mod types          (types.rs or types/mod.rs)               │
│      │      ├── pub struct Workload                                │
│      │      └── pub enum Status                                      │
│      │                                                                │
│      ├── mod storage        (storage.rs or storage/mod.rs)           │
│      │      ├── mod memory      (storage/memory.rs)                  │
│      │      │      └── pub struct InMemory                           │
│      │      └── mod etcd        (storage/etcd.)                    │
│      │             └── pub struct EtcdStore                          │
│      │                                                                │
│      └── mod api            (api.rs or api/mod.rs)                   │
│             ├── pub async fn handle_create                           │
│             └── pub async fn handle_list                             │
│                                                                      ──────────────────────────────────────────────────────────────────────┘

VISIBILITY RULES:
  • Private (default): Only within same module
  • pub(crate):        Anywhere in this crate
  • pub(super):        In parent module
  • pub:               Exported from crate (if reachable)
```

### File Organization Patterns

**Pattern 1: Single file per module (for smdules)**
```
src/
├── lib.rs           # mod types; mod storage; pub use types::*;
├── types.rs         # All type definitions
└── storage.rs       # All storage code
```

**Pattern 2: Directory per module (for larger modules)**
```
src/
├── lib.rs           # mod types; mod storage;
├── types/
│   ├── mod.rs       # mod workload; mod node; pub use workload::*;
│   ├── workload.rs  # Workload types
│   └── node.rs      # Node types
└── storage/
         # mod memory; mod etcd; pub trait StateStore;
    ├── memory.rs    # InMemoryStateStore
    └── etcd.rs      # EtcdStateStore
```

### Re-export Pattern for Clean APIs

```rust
// src/lib.rs - The crate's public API

// Internal modules (private)
mod reconciler;
mod api;
mod config;

// Public modules
pub mod types;
pub mod error;

// Re-export commonly used items at crate root
// Users can do: use orchestrator_core::{Orchestrator, Config};
pub use crate::config::Config;
pub use crate::rechestrator;
pub use crate::error::{Error, Result};

// Prelude for glob imports (optional but convenient)
// Users can do: use orchestrator_core::prelude::*;
pub mod prelude {
    pub use crate::types::*;
    pub use crate::error::{Error, Result};
    pub use crate::Orchestrator;
}
```

---

## 7. Error Handling Architecture

### The thiserror + anyhow Pattern

```
┌───────────────────────────────────────────────â─────────────────────┐
│                    ERROR HANDLING STRATEGY                            │
├────────────────────────────────┬─────────────────────────────────────┤
│         LIBRARY CRATES         │        APPLICATION CRATES           │
│      (state_store_interface)   │       (orchestrator_core)    ──────────────────────────────┼─────────────────────────────────────┤
│                                │                                      │
│  Use: thiserror                │  Use: anyhow                        │
│                                │                                      │
│  Why:                          │  Why:                                │
│  • Structured error types      │  • Easy error context                │
│  • Users can match on variants │  • Propagate any error               │
│  • Good for public APIs        │  • Good for main() and handlers     │
│                                │                                      │
│  #[derive(Error, Debug)]       │  use anyhow::{Context, Result};     │
│  pub enum StateError {         │                                      │
│    error("not found: {0}")]│  async fn load() -> Result<Config> {│
│      NotFound(String),         │      fs::read("config.toml")        │
│      #[error("connection")]    │          .context("read config")?;  │
│      Connection(#[from] io),   │  }                                  │
│  }                             │                                      │
│                                │                                      │
└──────────────â───────────────┴─────────────────────────────────────┘
```

### Implementing Library Errors

```rust
// state_store_interface/src/error.rs

use thiserror::Error;

/// Errors that can occur when interacting with state storage.
/// 
/// # Example
/// ```
/// use state_store_interface::{StateStore, StateError};
/// 
/// async fn get_node(store: &impl StateStore, id: &str) -> Result<Node, StateError> {.get_node(id).await?.ok_or(StateError::NotFound {
///         entity: "node",
///         id: id.to_string(),
///     })
/// }
/// ```
#[derive(Error, Debug)]
pub enum StateError {
    /// The requested entity was not found
    #[error("{entity} not found: {id}")]
    NotFound {
        entity: &'static str,
        id: String,
    },

    /// Serialization or deserialization failed
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// The underlying storage backend failed
    #[error("storage backend error: {0}")]
    Backend(#[source] Box<dyn std::error::Error + Send + Sync>),

    /// A conflict occurred (e.g., concurrent modification)
    #[error("conflict: {0}")]
    Conflict(String),

    /// The operation timed out
    #[error("operation timed out after {0:?}")]
    Timeout(std::time::Duration),
}

/// Result type alias for state store operations
pub type Result<T> = std::result::Result<T, StateError>;

// Implement From for easy conversion from backend-specific errors
impl From<etcd_client::Error> for StateError {
    fn from(err: etcd_client::Error) -> Self {
        StateError::Backend(Box::new(err))
    }
}
```

### Using Errors in Applications

```rust
// orchestrator_core/src/main.rs

use anyhow::{Context, Result, bail};
use tracing::{info, error};

#[tokio::main]
async fn main() -> Result<()> {
    // anyhow::Result can hold any error
    let config = load_config()
        .await
        .context("Failed to load configuration")?;

    let store = create_store(&config)
        .await
        .context("Failed to initialize state store")?;

    // Library errors are automatically converted
    let nodes = store.list_nodes().await
        .context("Failed to list nodes")?;

    info!("Found {} nodes", nodes.len());

    // Use bail! for early returns with custom errors
    if nodes.is_empty() {
        bail!("No nodes available in cluster");
    }

    run_orchestrator(store).await
}

async fn load_config() -> Result<Config> {
    let content = tokio::fs::read_to_string("config.toml")
        .await
        .context("Reading config file")?;
    
    let config: Config = toml::from_str(&content)
        .context("Parsing config TOML")?;
    
    Ok(config)
}
```

---

## 8. Async Runtime Patterns

### Tokio Runtime Configuration

```rust
// orchestrator_core/src/main.rs

use tokio::runtime::Builder;

fn main() -> anyhow::Result<()> {
    // Option 1: Use #[tokio::main] macro (simplest)
    // #[tokio::main]
    // async fn main() { ... }

    // Option 2: Manual runtime (more control)
    let runtime = Builder::new_multi_thread()
        .worker_threads(4)                    // Number of worker threads
        .thread_name("orchestrator-worker")   // Thread name prefix
        .thread_stack_size(3 * 1024 * 1024)  // 3MB stack per thread
        .enable_all()                         // Enable all Tokio features
        .build()
        .context("Failed to create Tokio runtime")?;

    // Run the async main function
    runtime.block_on(async_main())
}

async fn async_main() -> anyhow::Result<()> {
    // Your async code here
    Ok(())
}
```

### Async Trait Pattern

```rust
// Until async fn in traits is stabilized, use async-trait crate

use async_trait::async_trait;

#[async_trait]
pub trait StateStore: Send + Sync {
    //                 ^^^^^^^^^^^
    // CRITICAL: Send + Sync required for:
    // - Sharing across threads (Arc<dyn StateStore>)
    // - Using in async contexts
    // - Storing in structs that cross await points

    async fn get_node(&self, id: &str) -> Result<Option<Node>>;
    async fn put_node(&self, node: &Node) -> Result<()>;
    async fn list_nodes(&self) -> Result<Vec<Node>>;
}

// Implementation must also be Send + Sync
pub struct InMemoryStateStore {
    // RwLock is Send + Sync when T is Send + Sync
    nodes: RwLock<HashMap<String, Node>>,
}

// Verify at compile time
static_assertions::assert_impl_all!(InMemoryStateStore: Send, Sync);
```

### Concurrency Patterns

```rust
// Pattern 1: Shared state with RwLock (read-heavy workloads)
use tokio::sync::RwLock;

pub struct StateStore {
    data: RwLock<HashMap<String, Value>>,
}

impl StateStore {
    pub async fn get(&self, key: &str) -> Option<Value> {
        self.data.read().await.get(key).cloned()
    }
    
    pub async fn put(&self, key: String, value: Value) {
        self.data.write().await.insert(key, value);
    }
}

// Pattern 2: Actor model with channels (complex state machines)
use tokio::sync::mpsc;

enum Command {
    Get { key: String, reply: oneshot::Sender<Option<Value>> },
    Put { key: String, value: Value },
}

async fn state_actor(mut rx: mpsc::Receiver<Command>) {
    let mut state = HashMap::new();
    
    while let Some(cmd) = rx.recv().await {
        match cmd {
            Command::Get { key, reply } => {
                let _ = reply.send(state.get(&key).cloned());
            }
            Command::Put { key, value } => {
                state.insert(key, value);
            }
        }
    }
}

// Pattern 3: Select for multiple event sources
use tokio::select;

async fn reconciliation_loop(
    mut workload_rx: mpsc::Receiver<WorkloadEvent>,
    mut cluster_rx: mpsc::Receiver<ClusterEvent>,
    mut shutdown_rx: broadcast::Receiver<()>,
) {
    let mut interval = tokio::time::interval(Duration::from_secs(30));
    
    loop {
        select! {
            // Handle workload events
            Some(event) = workload_rx.recv() => {
                handle_workload_event(event).await;
            }
            // Handle cluster events
            Some(event) = cluster_rx.recv() => {
                handle_cluster_event(event).await;
            }
            // Periodic reconciliation
            _ = interval.tick() => {
                reconcile_all().await;
            }
            // Graceful shutdown
            _ = shutdown_rx.recv() => {
                info!("Shutting down reconciliation loop");
                break;
            }
        }
    }
}
```

---

## 9. Trait-Based Design Philosophy

### Interface Segregation

```rust
// ❌ BAD: One big trait
trait chestrator {
    fn create_workload(&self);
    fn delete_workload(&self);
    fn scale_workload(&self);
    fn get_nodes(&self);
    fn drain_node(&self);
    fn schedule(&self);
    fn store_state(&self);
    fn load_state(&self);
}

// ✅ GOOD: Segregated traits
trait WorkloadManager {
    fn create(&self, spec: WorkloadSpec) -> Result<WorkloadId>;
    fn delete(&self, id: WorkloadId) -> Result<()>;
    fn scale(&self, id: WorkloadId, replicas: u32) -> Result<()>;
}

trait ClusterManager {
    fn list_nes(&self) -> Result<Vec<Node>>;
    fn drain_node(&self, id: NodeId) -> Result<()>;
}

trait Scheduler {
    fn schedule(&self, workload: &Workload, nodes: &[Node]) -> Result<Placement>;
}

trait StateStore {
    fn get(&self, key: &str) -> Result<Option<Value>>;
    fn put(&self, key: &str, value: Value) -> Result<()>;
}
```

### Trait Objects vs Generics

```rust
// ============================================================
// GENERICS: Resolved at compile time (static dispatch)
// ============================================================
// Pros: Zero runtime cost, inlining possible
// Cons: Code bloat (each T generates new code), can't mix types

fn process_workloads<S: StateStore>(store: &S, workloads: &[Workload]) {
    // Compiler generates specialized code for each S type
}

// ============================================================
// TRAIT OBJECTS: Resolved at runtime (dynamic dispatch)
// ============================================================
// Pros: Flexibility, can store different types together
// Cons: Vtable lookup cost, no inlining

fn process_workloads(store: &dyn StateStore, workloads: &[Workload]) {
    // Single implementation, uses vtable for method calls
}

// ============================================================
// WHEN TO USE WHICH
// ============================================================

// Use GENERICS when:
// - Performance critical hot path
// - Type is known at compile time
// - You want monomorphization benefits

// Use TRAIT OBJECTS when:
// - Runtime flexibility needed (plugins, config-driven selection)
// - Reducing compile times
// - Storing heterogeneous collections

// COMMON PATTERN: Arc<dyn Trait>
pub struct Orchestrator {
    // Dynamic dispatch for pluggable backends
    state_store: Arc<dyn StateStore>,
    runtime: Arc<dyn ContainerRuntime>,
    scheduler: Arc<dyn Scheduler>,
}
```

### Extension Trait Pattern

```rust
// Base trait in interface crate
#[async_trait]
pub trait StateStore: Send + Sync {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>>;
    async fn put(&self, key: &str, value: Vec<u8>) -> Result<()>;
}

// Extension trait adds convenience methods
#[async_trait]
pub trait StateStoreExt: StateStore {
    async fn get_json<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>> {
        match self.get(key).await? {
            Some(bytes) => Ok(Some(serde_json::from_slice(&bytes)?)),
            None => Ok(None),
        }
    }
    
    async fn put_json<T: Serialize + Sync>(&self, key: &str, value: &T) -> Result<()> {
        let bytes = serde_json::to_vec(value)?;
        self.put(key, bytes).await
    }
}

// Blanket implementation: any StateStore gets StateStoreExt for free
impl<T: StateStore> StateStoreExt for T {}

// Usage:
// store.put_json("workload/123", &workload).await?;
// let workload: Workload = store.get_json("workload/123").await?.unwrap();
```

---

## 10. Testing Strategies

### Test Organization

```
crate/
├── src/
│   ├── lib.rs
│   └── storage.rs
├── tests/        # Integration tests (separate compilation)
│   ├── integration_test.rs   # Each file is a separate test binary
│   └── common/
│       └── mod.rs            # Shared test utilities
└── benches/                  # Benchmarks
    └── storage_bench.rs
```

### Unit Tests (in same file)

```rust
// src/storage/memory.rs

pub struct InMemoryStore { /* ... */ }

impl InMemoryStore {
    pub fn new() -> Self { /* ... */ }
}

// Unit tests go in the same file
#[cfg(test)]
mod te{
    use super::*;
    
    // Sync test
    #[test]
    fn test_new_store_is_empty() {
        let store = InMemoryStore::new();
        assert!(store.is_empty());
    }
    
    // Async test
    #[tokio::test]
    async fn test_put_and_get() {
        let store = InMemoryStore::new();
        store.put("key", "value").await.unwrap();
        let result = store.get("key").await.unwrap();
        assert_eq!(result, Some("value".to_string()));
    }
    
    // Parameterized test with test-case
    #[test_case("" ; "empty key")]
    #[test_case("a" ; "single char")]
    #[test_case("a/b/c" ; "path-like key")]
    fn test_key_formats(key: &str) {
        let store = InMemoryStore::new();
        assert!(store.is_valid_key(key));
    }
}
```

### Integration Tests

```rust
// tests/orchestrator_integration.rs

use orchestrator_core::{Orchestrator, Config};
use state_store_interface::InMemoryStateStore;
use std::sync::Arc;

// Shared setup
async fn setup() -> Orchestrator {
    let config = Config::default();
    let store = Arc::new(InMemoryStateStore::new());
    Orchestrator::new(config, store)
}

#[tokio::test]
async fn test_workload_lifecycle() {
    let orch = setup().await;
    
    // Create
    let id = orch.create_workload(WorkloadSpec {
        name: "test".into(),
        replicas: 3,
        ..Default::default()
    }).await.unwrap();
    
    // Verify
    let workload = orch.get_workload(&id).await.unwrap().unwrap();
    assert_eq!(workload.spec.replicas, 3);
    
    // Scale
    orch.scale_workload(&id, 5).await.unwrap();
    let workload = orch.get_workload(&id).await.unwrap().unwrap();
    assert_eq!(workload.spec.replicas, 5);
    
    // Delete
    orch.delete_workload(&id).await.unwrap();
    assert!(orch.get_workload(&id).await.unwrap().is_none());
}
```

### Property-Based Testing with Proptest

```rust
// tests/property_tests.rs

use proptest::prelude::*;

proptest! {
    // Test that any valid key can be stored and retrieved
    #[test]
    fn test_roundtrip(key in "[a-z]{1,100}", value in any::<Vec<u8>>()) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let store = InMemoryStore::new();
            store.put(&key, value.clone()).await.unwrap();
            let retrieved = store.get(&key).await.unwrap().unwrap();
            prop_assert_eq!(retrieved, value);
            Ok(())
        })?;
    }
    
    // Test concurrent operations don't corrupt state
    #[test]
    fn test_concurrent_safety(
        ops in prop::collection::vec(
            (any::<u8>(), "[a-z]{1,10}", any::<Vec<u8>>()),
            1..100
        )
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let store = Arc::new(InMemoryStore::new());
            let handles: Vec<_> = ops.into_iter().map(|(op_type, key, value)| {
                let store = store.clone();
                tokio::spawn(async move {
                    if op_type % 2 == 0 {
                        store.put(&key, value).await
                    } else {
                        store.get(&key).await.map(|_| ())
                    }
                })
            }).collect();
            
            for h in handles {
                h.await.unwrap().unwrap();
            }
            Ok(())
        })?;
    }
}
```

---

## 11. Build Optimization

### Cargo Profile Configuration

```toml
# Cargo.toml (workspace root)

# Development builds: fast compile, slow runtime
[profile.dev]
opt-level = 0           # No optimization
debug = true            # Full debug info
incremental = true      # Faster recompilation

# Development with some optimization (good for testing perf)
[profile.dev.package."*"]
opt-level = 2           # Optimize dependencies

# Release builds: slow compile, fast runtime
[profile.release]
opt-level = 3           # Maximum optimization
lto = "thin"            # Link-time optimization
codegen-units = 1       # Better optimization, slower compile
strip = true            # Strip symbols from binary

# Testing profile
[profile.test]
opt-level = 1           # Some optimization for faster tests
debug = true            # Keep debug info for backtraces

# Benchmarking profile
[profile.bench]
opt-level = 3
debug = false
```

### Compile Time Improvements

```toml
# .cargo/config.toml

[build]
# Use mold linker (much faster than default)
# Install: sudo apt install mold
rustflags = ["-C", "link-arg=-fuse-ld=mold"]

# Or use lld (LLVM linker)
# rustflags = ["-C", "link-arg=-fuse-ld=lld"]

[target.x86_64-unknown-linux-gnu]
linker = "clang"
rustflags = ["-C", "link-arg=-fuse-ld=mold"]

# Parallel frontend (nightly only)
# rustflags = ["-Z", "threads=8"]
```

### Build Script Best Practices

```rust
// build.rs - only if needed!

fn main() {
    // Tell Cargo to rerun only if specific files change
    println!("cargo:rerun-if-changed=proto/api.proto");
    println!("cargo:rerun-if-changed=build.rs");
    
    // Don't rerun on any source change
    // (default behavior without rerun-if-changed)
    
    // Proto compilation
    tonic_build::compile_protos("proto/api.proto")
        .expect("Failed to compile protos");
}
```

---

## Quick Reference Card

```
╔═══════════════════════════════════════════════════════════════════════╗
║                    RUST ORCHESTRATION CHEAT SHEET                      ║
╠════════════════════════════════â═════════════════════════════╣
║                                                                        ║
║  WORKSPACE COMMANDS                                                    ║
║  ─────────────────                                                     ║
║  cargo build --workspace         Build all crates                     ║
║  cargo test --workspace          Test all crates                    ║
║  cargo build -p orchestrator_core  Build specific crate               ║
║  cargo build --features youki    Build with feature                   ║
║  cargo build --release           Optimized build                      ║
║                                                                        ║
║  FEATURE FLAGS                                                         ║
║  ─────────────                                                         ║
║  carld --no-default-features                                    ║
║  cargo build --features "youki,etcd"                                  ║
║  cargo build --all-features                                           ║
║                                                                        ║
║  TESTING                                                               ║
║  ───────                                                               ║
║  cargo test                      Run sts                        ║
║  cargo test --lib                Only unit tests                      ║
║  cargo test --test integration   Only integration tests               ║
║  cargo test -- --nocapture       Show println! output                 ║
║  cargo test state_store          Run tests matching name              ║
║                                                                        ║
║  DEPENDENCY SYNTAX                                                     ║
║  ─────────────────                                                     ║
║  dep = "1.0"                     Version ^1.0.0                       ║
║  dep = { workspace = true }      Inherit from workspace               ║
║  dep = { path = "../other" }     Local crate                          ║
║  dep = { ..., optional = true }  Feature-gated                        ║
║  dep = { ..., features = ["x"] } Enable feature                       ║
║                                                                  ║
║  ERROR HANDLING                                                        ║
║  ──────────────                                                        ║
║  Library: thiserror → #[derive(Error)]                                ║
║  Binary:  anyhow    → .context("msg")?                                ║
║                                                                        ║
║  ASYNC PATTERNS                                                ║
║  ─────────────                                                         ║
║  #[async_trait] for trait async fns                                   ║
║  Arc<dyn Trait + Send + Sync> for shared trait objects                ║
║  tokio::select! for multiple event sources                            ║
║                                                                        ║
╚═════════════════════════════════════════════════════════════════╝
```

---

*Document Version: 1.0*
*Target: RustOrchestration Sprints 1-4*
