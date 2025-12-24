# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is an AI-native container orchestration system built in Rust, designed as a next-generation alternative to Kubernetes. The system uses the Model Context Protocol (MCP) as its API core for intent-driven control and integrates autonomous AI agents for maintenance and improvement tasks.

## Build and Development Commands

All Rust code is in the `orchestrator/` directory:

```bash
# Build the entire workspace
cd orchestrator && cargo build --workspace

# Build with all features enabled
cargo build --workspace --features full

# Run tests
cargo test --workspace

# Run a single test
cargo test --workspace test_name

# Run the orchestrator daemon (demo/mock mode)
cargo run --bin orchestrator_daemon

# Run with full features (cluster, runtime, observability, REST API, MCP)
cargo run --bin orchestrator_daemon --features full

# Run with real container runtime (requires youki, Linux only)
cargo run --bin orchestrator_node --features full-youki

# Quick build check (shows only warnings/errors)
./scripts/cb.sh
```

### Docker Compose for Multi-Node Testing

```bash
cd orchestrator
docker-compose up -d          # Start 3-node cluster (1 bootstrap + 2 workers)
docker-compose logs -f        # Follow logs
docker-compose down           # Stop cluster
```

Endpoints after docker-compose up:
- Bootstrap: http://localhost:9090
- Worker 1: http://localhost:9091
- Worker 2: http://localhost:9092

## Architecture

### Workspace Structure

The `orchestrator/` Cargo workspace is organized with clear separation between interfaces (trait definitions) and implementations:

**Interface Crates** (define contracts):
- `orchestrator_shared_types` - Common types: `Node`, `WorkloadDefinition`, `WorkloadInstance`, `OrchestrationError`
- `container_runtime_interface` - `ContainerRuntime` trait for container lifecycle
- `cluster_manager_interface` - `ClusterManager` trait for node membership and events
- `scheduler_interface` - `Scheduler` trait for workload placement decisions
- `state_store_interface` - `StateStore` trait for persistent state

**Implementation Crates**:
- `orchestrator_core` - Main orchestration loop and reconciliation logic
- `container_runtime` - Runtime implementations (mock, youki-cli)
- `cluster_manager` - Gossip-based clustering using chitchat
- `mcp_server` - Model Context Protocol server
- `observability` - Metrics and structured logging
- `user_config` - Configuration management
- `ui_cli` - Command-line interface

### Core Patterns

**Reconciliation Loop**: The `Orchestrator` in `orchestrator_core/src/lib.rs` runs an event-driven loop using `tokio::select!`:
1. Receives workload definitions via channel
2. Subscribes to cluster events (node added/removed/updated)
3. Reconciles desired state with actual state
4. Schedules containers on available nodes

**Dependency Injection**: All components use `Arc<dyn Trait>` for testability and pluggability.

**Feature Flags** (in `orchestrator_core/Cargo.toml`):
- `cluster` - Enables chitchat-based cluster management
- `runtime` - Mock container runtime
- `youki-runtime` - Real OCI runtime via Youki (Linux only)
- `rest-api` - REST API with ed25519 authentication
- `mcp` - MCP protocol server
- `full` / `full-youki` - All features combined

### External Dependencies

The workspace references sibling crates (must exist at paths relative to repo root):
- `../../univrs-identity`
- `../../univrs-state`

## Ontology

The `ontology/` directory contains domain specifications:

- `prospective/` - Future/planned specifications (identity, reconciliation, scheduling, storage)
- `retrospective/` - Current implementation analysis (api, cluster, container, events, state)

These YAML/JSON files define the conceptual model for the orchestrator's behavior.

## Key Files

- `orchestrator/orchestrator_core/src/lib.rs` - Core orchestration logic
- `orchestrator/orchestrator_core/src/main.rs` - Daemon entry point
- `orchestrator/Cargo.toml` - Workspace definition
- `orchestrator/docker-compose.yml` - Multi-node test cluster
- `orchestrator/Dockerfile` - Container image build

## Testing

Integration tests are in `orchestrator/orchestrator_core/tests/`:
- `reconciliation_integration_tests.rs` - Core reconciliation logic
- `api_integration_tests.rs` - REST API tests
- `youki_integration.rs` - Container runtime integration
- `mcp_server/tests/mcp_integration_tests.rs` - MCP protocol tests
