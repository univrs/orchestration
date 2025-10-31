# Youki Runtime Integration - Implementation Guide
## RustOrchestration Platform
**Date**: 2025-10-31

---

## Overview

This document describes the Youki runtime integration - a critical component that enables **real container execution** in the RustOrchestration platform. Youki is a Rust-based OCI (Open Container Initiative) runtime that provides container lifecycle management.

---

## Architecture

### Component Structure

```
youki_runtime/
├── Cargo.toml          # Dependencies and features
└── src/
    └── lib.rs          # Youki runtime implementation
```

### Key Components

1. **YoukiRuntime** - Main struct implementing `ContainerRuntime` trait
2. **OCI Spec Generation** - Converts `ContainerConfig` to OCI runtime specification
3. **Bundle Management** - Creates container bundles with config.json and rootfs
4. **Container Lifecycle** - Create, start, stop, delete operations via Youki binary
5. **Status Parsing** - Extracts container state from Youki JSON output

---

## Implementation Details

### 1. Core Struct

```rust
pub struct YoukiRuntime {
    /// Path to the youki binary
    youki_binary: PathBuf,

    /// Root directory for container state
    state_dir: PathBuf,

    /// Root directory for container bundles
    bundle_dir: PathBuf,

    /// Per-node state tracking
    node_states: RwLock<HashMap<NodeId, NodeRuntimeState>>,
}
```

**Defaults**:
- Youki binary: `youki` (from PATH)
- State directory: `/var/run/orchestrator/youki`
- Bundle directory: `/var/lib/orchestrator/bundles`

### 2. OCI Specification Generation

Converts orchestrator's `ContainerConfig` into OCI-compliant runtime specification:

```rust
fn generate_oci_spec(&self, config: &ContainerConfig) -> Result<Spec>
```

**Generated Components**:
- **Process**: Command, args, environment variables, working directory
- **Root**: Filesystem path, read-only flag
- **Linux**: Linux-specific configuration (namespaces, cgroups, etc.)
- **Version**: OCI spec version 1.0.2

**Example OCI Spec**:
```json
{
  "ociVersion": "1.0.2",
  "process": {
    "args": ["/bin/sh", "-c", "echo hello"],
    "cwd": "/",
    "env": [
      "PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
      "FOO=bar"
    ]
  },
  "root": {
    "path": "rootfs",
    "readonly": false
  },
  "linux": {}
}
```

### 3. Container Bundle Creation

**Bundle Structure**:
```
/var/lib/orchestrator/bundles/{container-id}/
├── config.json         # OCI runtime spec
└── rootfs/             # Container filesystem
```

**Process**:
1. Create bundle directory
2. Generate OCI spec from `ContainerConfig`
3. Write `config.json`
4. Create `rootfs` directory
5. ⚠️  **TODO**: Extract container image to rootfs

### 4. Container Lifecycle Operations

#### Create & Start
```rust
async fn create_container(
    &self,
    config: &ContainerConfig,
    options: &CreateContainerOptions,
) -> Result<ContainerId>
```

**Workflow**:
1. Generate unique container ID: `container-{uuid}`
2. Create bundle with OCI spec
3. Execute: `youki --root {state_dir} create {id} --bundle {bundle_path}`
4. Execute: `youki --root {state_dir} start {id}`
5. Store container metadata in node state

#### Stop
```rust
async fn stop_container(&self, container_id: &ContainerId) -> Result<()>
```

**Workflow**:
1. Execute: `youki --root {state_dir} kill {id} SIGTERM`
2. Container gracefully terminates

#### Remove
```rust
async fn remove_container(&self, container_id: &ContainerId) -> Result<()>
```

**Workflow**:
1. Execute: `youki --root {state_dir} delete {id}`
2. Remove bundle directory
3. Clean up node state

#### Get Status
```rust
async fn get_container_status(&self, container_id: &ContainerId) -> Result<ContainerStatus>
```

**Workflow**:
1. Execute: `youki --root {state_dir} state {id}`
2. Parse JSON output
3. Map to `ContainerStatus`

### 5. Youki Command Execution

```rust
async fn run_youki_command(&self, args: &[&str]) -> Result<std::process::Output>
```

**Features**:
- Async execution via `tokio::process::Command`
- Captures stdout and stderr
- Error checking with detailed logging
- Returns parsed output or error

**Example Youki Commands**:
```bash
# Create container
youki --root /var/run/orchestrator/youki create container-123 \
    --bundle /var/lib/orchestrator/bundles/container-123

# Start container
youki --root /var/run/orchestrator/youki start container-123

# Get status
youki --root /var/run/orchestrator/youki state container-123

# Kill container
youki --root /var/run/orchestrator/youki kill container-123 SIGTERM

# Delete container
youki --root /var/run/orchestrator/youki delete container-123
```

---

## Container State Management

### Node State Tracking

```rust
struct NodeRuntimeState {
    containers: HashMap<ContainerId, ContainerMetadata>,
}

struct ContainerMetadata {
    bundle_path: PathBuf,
    container_id: ContainerId,
    node_id: NodeId,
    _temp_dir: Option<String>,
}
```

**Purpose**:
- Track containers per node
- Enable efficient container listing
- Maintain bundle paths for cleanup

### Container Status

```rust
pub struct ContainerStatus {
    pub id: ContainerId,
    pub state: String,        // "created", "running", "stopped"
    pub exit_code: Option<i32>,
    pub error_message: Option<String>,
}
```

**OCI Container States**:
- `creating` - Container is being created
- `created` - Runtime has finished create operation
- `running` - Container process is running
- `stopped` - Container process has exited

---

## Dependencies

### Core Dependencies

```toml
[dependencies]
# Orchestrator interfaces
orchestrator_shared_types = { path = "../orchestrator_shared_types" }
container_runtime_interface = { path = "../container_runtime_interface" }

# Workspace dependencies
tokio = { workspace = true }           # Async runtime
async-trait = { workspace = true }     # Async trait support
serde = { workspace = true }           # Serialization
serde_json = { workspace = true }      # JSON handling
tracing = { workspace = true }         # Logging
uuid = { workspace = true }            # ID generation

# External dependencies
oci-spec = "0.6"        # OCI runtime spec types
tempfile = "3.8"        # Temp directories for bundles
nix = "0.27"            # Process/signal management
```

---

## Testing

### Unit Tests

```rust
#[test]
fn test_oci_spec_generation() {
    let runtime = YoukiRuntime::new(None, None, None).unwrap();

    let config = ContainerConfig {
        name: "test-container".to_string(),
        image: "alpine:latest".to_string(),
        command: Some(vec!["/bin/sh".to_string()]),
        env_vars: HashMap::from([
            ("FOO".to_string(), "bar".to_string()),
        ]),
        ...
    };

    let spec = runtime.generate_oci_spec(&config).unwrap();

    assert_eq!(spec.version(), "1.0.2");
    assert!(spec.process().is_some());
    assert!(spec.root().is_some());
}
```

### Integration Tests (Planned)

```rust
#[tokio::test]
#[ignore] // Requires youki binary
async fn test_container_lifecycle() {
    let runtime = YoukiRuntime::new(None, None, None).unwrap();
    let node_id = Uuid::new_v4();

    runtime.init_node(node_id).await.unwrap();

    let config = ContainerConfig { /* ... */ };
    let options = CreateContainerOptions {
        workload_id: Uuid::new_v4(),
        node_id,
    };

    // Create and start
    let container_id = runtime.create_container(&config, &options).await.unwrap();

    // Check status
    let status = runtime.get_container_status(&container_id).await.unwrap();
    assert_eq!(status.state, "running");

    // Stop
    runtime.stop_container(&container_id).await.unwrap();

    // Remove
    runtime.remove_container(&container_id).await.unwrap();
}
```

---

## Limitations & TODOs

### 🔴 Critical

1. **Image Extraction** ⚠️  **REQUIRED FOR PRODUCTION**
   - Currently, `rootfs` directory is empty
   - Need to pull and extract container image
   - Options:
     - Integrate `containerd` for image management
     - Use `skopeo` to copy images to OCI layout
     - Use `podman` image extraction
     - Build custom image puller

   **Temporary Workaround**:
   ```bash
   # Manual image extraction (for testing)
   skopeo copy docker://alpine:latest oci:/tmp/alpine:latest
   umoci unpack --image /tmp/alpine:latest bundle
   cp -r bundle/rootfs /var/lib/orchestrator/bundles/{container-id}/rootfs
   ```

2. **Resource Limits** (cgroups)
   - OCI spec includes resource constraints:
     - CPU: `cpu.shares`, `cpu.quota`
     - Memory: `memory.limit_in_bytes`
     - I/O: `blkio.weight`
   - Need to populate `LinuxResources` from `ContainerConfig.resource_requests`

3. **Namespaces Configuration**
   - Network namespace setup
   - PID namespace isolation
   - Mount namespace for filesystem isolation
   - User namespace for rootless containers (optional)

### 🟡 Medium Priority

4. **Port Mapping**
   - Implement CNI (Container Network Interface) plugin support
   - Map `ContainerConfig.ports` to iptables/nftables rules
   - Handle host port conflicts

5. **Volume Mounts**
   - Support persistent volumes
   - Bind mounts from host
   - Temporary volumes (emptyDir)

6. **Health Checks**
   - Implement container health probes (exec, http, tcp)
   - Restart policy based on health status

7. **Logging**
   - Capture container stdout/stderr
   - Stream logs to orchestrator
   - Log rotation and retention

### 🟢 Nice to Have

8. **Security**
   - AppArmor/SELinux profiles
   - Seccomp filters
   - Capabilities management
   - Rootless containers

9. **Advanced Features**
   - Container checkpointing (CRIU)
   - Live migration
   - GPU support
   - RDMA support

---

## Usage Example

### Basic Container Creation

```rust
use std::collections::HashMap;
use youki_runtime::YoukiRuntime;
use orchestrator_shared_types::*;
use container_runtime_interface::*;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize runtime
    let runtime = YoukiRuntime::new(None, None, None)?;
    let node_id = Uuid::new_v4();

    runtime.init_node(node_id).await?;

    // Define container
    let config = ContainerConfig {
        name: "my-app".to_string(),
        image: "nginx:latest".to_string(),
        command: None, // Use image default
        args: None,
        env_vars: HashMap::from([
            ("ENV".to_string(), "production".to_string()),
        ]),
        ports: vec![
            PortMapping {
                container_port: 80,
                host_port: Some(8080),
                protocol: "tcp".to_string(),
            },
        ],
        resource_requests: NodeResources {
            cpu_cores: 0.5,
            memory_mb: 512,
            disk_mb: 0,
        },
    };

    let options = CreateContainerOptions {
        workload_id: Uuid::new_v4(),
        node_id,
    };

    // Create and start container
    let container_id = runtime.create_container(&config, &options).await?;
    println!("Container created: {}", container_id);

    // Get status
    let status = runtime.get_container_status(&container_id).await?;
    println!("Container status: {}", status.state);

    // List all containers on node
    let containers = runtime.list_containers(node_id).await?;
    println!("Running containers: {}", containers.len());

    Ok(())
}
```

---

## Integration with Orchestrator

### Orchestrator Core Usage

```rust
use youki_runtime::YoukiRuntime;
use orchestrator_core::start_orchestrator_service;

let state_store = Arc::new(InMemoryStateStore::new());
let runtime = Arc::new(YoukiRuntime::new(None, None, None)?);
let cluster_manager = Arc::new(MockClusterManager::new());
let scheduler = Arc::new(SimpleScheduler);

let workload_tx = start_orchestrator_service(
    state_store,
    runtime,           // <-- Youki runtime here
    cluster_manager,
    scheduler,
).await?;

// Submit workload
workload_tx.send(workload_def).await?;
```

---

## Deployment Considerations

### Prerequisites

1. **Youki Binary**:
   ```bash
   # Install Youki
   cargo install youki

   # Or download from releases
   wget https://github.com/containers/youki/releases/download/v0.3.0/youki
   chmod +x youki
   sudo mv youki /usr/local/bin/
   ```

2. **Directories**:
   ```bash
   sudo mkdir -p /var/run/orchestrator/youki
   sudo mkdir -p /var/lib/orchestrator/bundles
   sudo chown -R orchestrator:orchestrator /var/run/orchestrator
   sudo chown -R orchestrator:orchestrator /var/lib/orchestrator
   ```

3. **Permissions**:
   - Youki requires root or capabilities for namespaces
   - Consider rootless mode for enhanced security

### Configuration

```rust
// Production configuration
let runtime = YoukiRuntime::new(
    Some(PathBuf::from("/usr/local/bin/youki")),
    Some(PathBuf::from("/var/run/orchestrator/youki")),
    Some(PathBuf::from("/var/lib/orchestrator/bundles")),
)?;
```

### Monitoring

- Check Youki logs: `journalctl -u youki`
- Monitor container states: `youki list --root /var/run/orchestrator/youki`
- Track bundle directory size: `du -sh /var/lib/orchestrator/bundles`

---

## Performance Considerations

### Benchmark Results (Expected)

Based on Youki's published benchmarks:

| Operation | Youki | runc | Improvement |
|-----------|-------|------|-------------|
| Create    | 111ms | 224ms| 50% faster  |
| Delete    | 50ms  | 100ms| 50% faster  |
| Memory    | 6MB   | 12MB | 50% less    |

### Optimization Tips

1. **Bundle Caching**: Reuse rootfs for identical images
2. **Lazy Loading**: Extract images on-demand
3. **Parallel Creation**: Create multiple containers concurrently
4. **State Cleanup**: Regularly prune old bundles

---

## Troubleshooting

### Common Issues

**Issue**: `Failed to execute youki: No such file or directory`

**Solution**:
```bash
which youki
# If not found, install or specify full path
```

**Issue**: `Permission denied when creating containers`

**Solution**:
```bash
# Run with sudo or grant capabilities
sudo setcap cap_sys_admin+epi /usr/local/bin/youki
```

**Issue**: `Failed to create bundle dir: Permission denied`

**Solution**:
```bash
sudo chown -R $(whoami) /var/lib/orchestrator/bundles
```

**Issue**: `Container state: unknown`

**Solution**:
- Check Youki logs for errors
- Verify OCI spec is valid
- Ensure rootfs exists and has content

---

## Future Enhancements

### Roadmap

**Phase 1** (Current):
- ✅ Basic container lifecycle (create, start, stop, delete)
- ✅ OCI spec generation
- ✅ Status parsing

**Phase 2** (Next):
- [ ] Image pulling and extraction
- [ ] Resource limits (cgroups)
- [ ] Basic networking (CNI)

**Phase 3**:
- [ ] Volume mounts
- [ ] Health checks
- [ ] Log collection

**Phase 4**:
- [ ] Advanced security (AppArmor, Seccomp)
- [ ] GPU support
- [ ] Live migration

---

## References

- **Youki Documentation**: https://github.com/containers/youki
- **OCI Runtime Specification**: https://github.com/opencontainers/runtime-spec
- **oci-spec Rust Crate**: https://docs.rs/oci-spec/
- **Container Lifecycle**: https://github.com/opencontainers/runtime-spec/blob/main/runtime.md

---

**Document Version**: 1.0
**Author**: Claude (AI Assistant)
**Date**: 2025-10-31
**Status**: Youki runtime integration complete (Phase 1)
