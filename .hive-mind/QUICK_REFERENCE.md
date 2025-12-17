# Quick Reference Guide - Rust Orchestration Patterns
## Hive Mind Collective - RESEARCHER Agent

**Last Updated**: 2025-12-17

---

## Essential Crates Cheat Sheet

### Consensus & Coordination
```rust
// Leader Election with etcd
use etcd_client::{Client, LeaseKeeper};

let mut client = Client::connect(["localhost:2379"], None).await?;
let lease = client.lease_grant(10, None).await?;
let _keeper = LeaseKeeper::new(client.clone(), lease.id());

// OpenRaft for State Machine Replication
use openraft::{Raft, Config, RaftStorage};

let config = Config::default();
let raft = Raft::new(node_id, config, network, storage).await?;
```

### Cluster Membership
```rust
// Chitchat for Gossip-based Membership
use chitchat::{Chitchat, ChitchatConfig, ChitchatHandle};

let config = ChitchatConfig {
    cluster_id: "orchestrator-cluster".to_string(),
    listen_addr: "0.0.0.0:7280".parse()?,
    seed_nodes: vec!["node1:7280".to_string()],
    ..Default::default()
};

let chitchat = Chitchat::spawn(config).await?;
```

### Observability
```rust
// Tracing Setup
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

tracing_subscriber::registry()
    .with(tracing_subscriber::EnvFilter::from_default_env())
    .with(tracing_subscriber::fmt::layer())
    .init();

// Add spans to async functions
#[tracing::instrument(skip(self))]
async fn reconcile_workload(&self, workload: &WorkloadDefinition) -> Result<()> {
    tracing::info!("Starting reconciliation for workload {}", workload.id);
    // ...
}

// Prometheus Metrics
use metrics::{counter, histogram, gauge};

counter!("orchestrator.workloads.created").increment(1);
histogram!("orchestrator.reconciliation.duration").record(duration.as_secs_f64());
gauge!("orchestrator.cluster.nodes").set(node_count as f64);
```

### MCP Server
```rust
// Basic MCP Server
use rmcp::{ServerHandler, tool_handler, Transport};

#[async_trait]
impl ServerHandler for MyServer {
    fn metadata(&self) -> ServerMetadata {
        ServerMetadata {
            name: "orchestrator".to_string(),
            version: "0.1.0".to_string(),
        }
    }
}

#[tool_handler]
impl MyServer {
    async fn deploy(&self, name: String, image: String) -> Result<String> {
        // Implementation
        Ok(workload_id)
    }
}
```

---

## Common Patterns

### Error Handling
```rust
// Library/Interface Crate (use thiserror)
#[derive(Debug, Error)]
pub enum MyError {
    #[error("Resource not found: {0}")]
    NotFound(String),

    #[error("Invalid state transition from {from} to {to}")]
    InvalidTransition { from: String, to: String },
}

// Application Crate (use anyhow)
use anyhow::{Context, Result};

async fn process() -> Result<()> {
    do_something()
        .await
        .context("Failed to process workload")?;

    Ok(())
}
```

### Channel Selection
```rust
// Work Queue: mpsc
let (tx, mut rx) = mpsc::channel(100);
tx.send(work_item).await?;
while let Some(item) = rx.recv().await {
    process(item).await;
}

// State Updates: watch
let (tx, mut rx) = watch::channel(initial_value);
tx.send(new_value)?;
rx.changed().await?;
let current = *rx.borrow();

// Pub/Sub Events: broadcast
let (tx, mut rx) = broadcast::channel(100);
tx.send(event)?;
match rx.recv().await {
    Ok(event) => handle(event),
    Err(RecvError::Lagged(n)) => resync().await?,
    Err(RecvError::Closed) => break,
}

// Single Response: oneshot
let (tx, rx) = oneshot::channel();
tokio::spawn(async move { tx.send(result).unwrap() });
let result = rx.await?;
```

### Tokio Select Pattern
```rust
use tokio::time::{sleep, Duration};

loop {
    tokio::select! {
        Some(msg) = msg_rx.recv() => {
            process_message(msg).await;
        }
        Ok(()) = event_rx.changed() => {
            let event = event_rx.borrow().clone();
            handle_event(event).await;
        }
        _ = sleep(Duration::from_secs(30)) => {
            periodic_task().await;
        }
        _ = shutdown.recv() => {
            info!("Shutting down");
            break;
        }
    }
}
```

### Retry with Exponential Backoff
```rust
use tokio::time::{sleep, Duration};

async fn retry_with_backoff<F, T, E>(
    mut operation: F,
    max_attempts: u32,
) -> Result<T, E>
where
    F: FnMut() -> Pin<Box<dyn Future<Output = Result<T, E>> + Send>>,
{
    let mut attempts = 0;

    loop {
        match operation().await {
            Ok(result) => return Ok(result),
            Err(e) if attempts < max_attempts - 1 => {
                let delay = Duration::from_secs(2_u64.pow(attempts));
                sleep(delay).await;
                attempts += 1;
            }
            Err(e) => return Err(e),
        }
    }
}
```

### Graceful Shutdown
```rust
use tokio::signal;
use tokio::sync::broadcast;

async fn shutdown_signal() -> broadcast::Receiver<()> {
    let (shutdown_tx, shutdown_rx) = broadcast::channel(1);

    tokio::spawn(async move {
        signal::ctrl_c().await.expect("failed to listen for Ctrl+C");
        let _ = shutdown_tx.send(());
    });

    shutdown_rx
}

// In main loop
let mut shutdown = shutdown_signal().await;

tokio::select! {
    // ... other branches
    _ = shutdown.recv() => {
        graceful_shutdown().await;
        break;
    }
}
```

### Health Check Server
```rust
use axum::{Router, routing::get, Json};
use std::net::SocketAddr;

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}

async fn readiness(State(store): State<Arc<dyn StateStore>>) -> &'static str {
    if store.health_check().await.unwrap_or(false) {
        "ready"
    } else {
        "not ready"
    }
}

let app = Router::new()
    .route("/healthz", get(health))
    .route("/readyz", get(readiness));

let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
axum::Server::bind(&addr)
    .serve(app.into_make_service())
    .await?;
```

---

## Testing Patterns

### Async Test Setup
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tokio::test;

    #[tokio::test]
    async fn test_scheduler() {
        let scheduler = SimpleScheduler;
        let nodes = vec![create_test_node()];
        let request = create_test_request();

        let decisions = scheduler.schedule(&request, &nodes).await.unwrap();

        assert_eq!(decisions.len(), 1);
    }
}
```

### Mock with mockall
```rust
use mockall::{automock, predicate::*};

#[automock]
#[async_trait]
pub trait StateStore: Send + Sync {
    async fn get_node(&self, id: &NodeId) -> Result<Option<Node>>;
}

#[tokio::test]
async fn test_with_mock() {
    let mut mock = MockStateStore::new();

    mock.expect_get_node()
        .with(eq(node_id))
        .times(1)
        .returning(|_| Ok(Some(test_node())));

    // Use mock in test
}
```

---

## Performance Tips

### Avoid Common Pitfalls

```rust
// ❌ BAD: Holding lock across await
{
    let mut data = mutex.lock().await;
    expensive_async_operation().await; // Lock held!
    data.update();
}

// ✅ GOOD: Release lock before await
{
    let value = {
        let data = mutex.lock().await;
        data.clone()
    }; // Lock released

    expensive_async_operation().await;

    let mut data = mutex.lock().await;
    data.update();
}

// ❌ BAD: Sequential when parallel is possible
let result1 = operation1().await;
let result2 = operation2().await;

// ✅ GOOD: Parallel execution
let (result1, result2) = tokio::join!(
    operation1(),
    operation2()
);

// ❌ BAD: Spawning without bounds
for item in items {
    tokio::spawn(process(item));
}

// ✅ GOOD: Bounded concurrency
use futures::stream::{self, StreamExt};

stream::iter(items)
    .for_each_concurrent(10, |item| async move {
        process(item).await;
    })
    .await;
```

### Tokio Runtime Tuning
```rust
// For I/O-heavy workloads
#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
async fn main() { }

// For CPU-heavy with some async I/O
let runtime = tokio::runtime::Builder::new_multi_thread()
    .worker_threads(num_cpus::get())
    .thread_name("orchestrator-worker")
    .enable_all()
    .build()?;

runtime.block_on(async {
    // Application logic
});
```

---

## Debugging Tools

### Tokio Console
```rust
// Add to Cargo.toml
// [dependencies]
// console-subscriber = "0.4"

// In main.rs
fn main() {
    console_subscriber::init();

    tokio::runtime::Runtime::new().unwrap().block_on(async {
        // Your async code
    });
}

// Run with: tokio-console
```

### Tracing with Filtering
```bash
# Set environment variable
export RUST_LOG=orchestrator_core=debug,tokio=info,warn

# Or in code
use tracing_subscriber::EnvFilter;

tracing_subscriber::fmt()
    .with_env_filter(
        EnvFilter::from_default_env()
            .add_directive("orchestrator_core=debug".parse()?)
    )
    .init();
```

---

## Quick Command Reference

### Build & Test
```bash
# Build workspace
cargo build --workspace

# Run tests
cargo test --workspace

# Run with tracing
RUST_LOG=debug cargo run

# Check without building
cargo check --workspace

# Format code
cargo fmt --all

# Lint
cargo clippy --all-targets --all-features
```

### Tokio Console
```bash
# Install
cargo install tokio-console

# Run application with console support
RUSTFLAGS="--cfg tokio_unstable" cargo run

# In another terminal
tokio-console
```

---

## Resources

### Documentation
- Tokio: https://tokio.rs
- async-trait: https://docs.rs/async-trait
- tracing: https://docs.rs/tracing
- kube-rs: https://kube.rs

### Tools
- tokio-console: Async runtime inspector
- cargo-watch: Auto-rebuild on changes
- cargo-nextest: Faster test runner

### Community
- Rust async working group: https://rust-lang.github.io/wg-async/
- Tokio Discord: https://discord.gg/tokio

---

**End of Quick Reference**
