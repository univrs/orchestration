# Youki Integration Design

**Real Container Runtime for Univrs Orchestrator**

Version: 0.1.0  
Date: December 18, 2025  
Status: Design Document

---

## Overview

This document outlines the integration of [Youki](https://github.com/containers/youki), a Rust-native OCI container runtime, into the Univrs orchestrator. This replaces the current `MockRuntime` with actual container execution capabilities.

### Goals

1. Execute real OCI containers on worker nodes
2. Manage complete container lifecycle (create, start, stop, kill, delete)
3. Pull and cache container images
4. Collect logs and metrics from running containers
5. Enforce resource limits via cgroups
6. Maintain the existing `ContainerRuntime` trait interface

### Non-Goals (This Phase)

- Windows container support
- GPU scheduling
- Custom CNI networking (use host networking initially)
- Persistent volumes (use ephemeral storage)

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────┐
│                         ORCHESTRATOR NODE                                │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│  ┌─────────────────┐                                                    │
│  │   Reconciler    │  "Desired state vs actual state"                   │
│  └────────┬────────┘                                                    │
│           │                                                              │
│           ▼                                                              │
│  ┌─────────────────┐                                                    │
│  │ContainerRuntime │  trait (interface)                                 │
│  │     Trait       │                                                    │
│  └────────┬────────┘                                                    │
│           │                                                              │
│           ▼                                                              │
│  ┌─────────────────┐     ┌─────────────────┐                           │
│  │  YoukiRuntime   │────▶│  Image Manager  │                           │
│  │  (new impl)     │     │  (pull, cache)  │                           │
│  └────────┬────────┘     └─────────────────┘                           │
│           │                                                              │
│           ▼                                                              │
│  ┌─────────────────┐     ┌─────────────────┐                           │
│  │   Youki CLI     │────▶│  OCI Bundle     │                           │
│  │   (libcontainer)│     │  (config.json)  │                           │
│  └────────┬────────┘     └─────────────────┘                           │
│           │                                                              │
│           ▼                                                              │
│  ┌─────────────────────────────────────────┐                           │
│  │              Linux Kernel                │                           │
│  │  ┌─────────┐ ┌─────────┐ ┌───────────┐  │                           │
│  │  │ cgroups │ │namespaces│ │ seccomp   │  │                           │
│  │  └─────────┘ └─────────┘ └───────────┘  │                           │
│  └─────────────────────────────────────────┘                           │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## Component Design

### 1. YoukiRuntime Struct

```rust
// container_runtime/src/youki.rs

use crate::{ContainerRuntime, ContainerConfig, ContainerStatus, RuntimeError};
use std::path::PathBuf;
use std::collections::HashMap;
use tokio::sync::RwLock;

pub struct YoukiRuntime {
    /// Root directory for container bundles
    bundle_root: PathBuf,
    
    /// Root directory for container state
    state_root: PathBuf,
    
    /// Image cache directory
    image_cache: PathBuf,
    
    /// Running container states
    containers: RwLock<HashMap<String, ContainerState>>,
    
    /// Path to youki binary (or use libcontainer directly)
    youki_path: PathBuf,
}

struct ContainerState {
    id: String,
    bundle_path: PathBuf,
    pid: Option<u32>,
    status: ContainerStatus,
    created_at: chrono::DateTime<chrono::Utc>,
    started_at: Option<chrono::DateTime<chrono::Utc>>,
    finished_at: Option<chrono::DateTime<chrono::Utc>>,
    exit_code: Option<i32>,
}

impl YoukiRuntime {
    pub fn new(config: YoukiConfig) -> Result<Self, RuntimeError> {
        // Verify youki is available
        // Create directory structure
        // Initialize state
    }
}
```

### 2. ContainerRuntime Trait Implementation

```rust
#[async_trait]
impl ContainerRuntime for YoukiRuntime {
    async fn create_container(
        &self,
        id: &str,
        config: &ContainerConfig,
    ) -> Result<(), RuntimeError> {
        // 1. Pull image if not cached
        self.ensure_image(&config.image).await?;
        
        // 2. Create OCI bundle directory
        let bundle_path = self.create_bundle(id, config).await?;
        
        // 3. Generate config.json (OCI spec)
        self.generate_oci_config(id, config, &bundle_path).await?;
        
        // 4. Call youki create
        self.youki_create(id, &bundle_path).await?;
        
        // 5. Update internal state
        self.containers.write().await.insert(id.to_string(), ContainerState {
            id: id.to_string(),
            bundle_path,
            pid: None,
            status: ContainerStatus::Created,
            created_at: chrono::Utc::now(),
            started_at: None,
            finished_at: None,
            exit_code: None,
        });
        
        Ok(())
    }

    async fn start_container(&self, id: &str) -> Result<(), RuntimeError> {
        // 1. Call youki start
        self.youki_start(id).await?;
        
        // 2. Get PID
        let pid = self.youki_state(id).await?.pid;
        
        // 3. Update state
        let mut containers = self.containers.write().await;
        if let Some(state) = containers.get_mut(id) {
            state.pid = Some(pid);
            state.status = ContainerStatus::Running;
            state.started_at = Some(chrono::Utc::now());
        }
        
        Ok(())
    }

    async fn stop_container(
        &self,
        id: &str,
        timeout: Option<Duration>,
    ) -> Result<(), RuntimeError> {
        // 1. Send SIGTERM via youki kill
        self.youki_kill(id, "SIGTERM").await?;
        
        // 2. Wait for graceful shutdown
        let timeout = timeout.unwrap_or(Duration::from_secs(30));
        if !self.wait_for_exit(id, timeout).await {
            // 3. Force kill if timeout
            self.youki_kill(id, "SIGKILL").await?;
        }
        
        // 4. Update state
        self.update_exit_state(id).await?;
        
        Ok(())
    }

    async fn remove_container(&self, id: &str) -> Result<(), RuntimeError> {
        // 1. Ensure stopped
        if self.get_status(id).await? == ContainerStatus::Running {
            self.stop_container(id, None).await?;
        }
        
        // 2. Call youki delete
        self.youki_delete(id).await?;
        
        // 3. Clean up bundle directory
        let state = self.containers.write().await.remove(id);
        if let Some(state) = state {
            tokio::fs::remove_dir_all(&state.bundle_path).await?;
        }
        
        Ok(())
    }

    async fn get_status(&self, id: &str) -> Result<ContainerStatus, RuntimeError> {
        // Query youki state and reconcile with internal state
        let youki_state = self.youki_state(id).await?;
        
        match youki_state.status.as_str() {
            "creating" => Ok(ContainerStatus::Creating),
            "created" => Ok(ContainerStatus::Created),
            "running" => Ok(ContainerStatus::Running),
            "stopped" => Ok(ContainerStatus::Stopped),
            _ => Ok(ContainerStatus::Unknown),
        }
    }

    async fn get_logs(
        &self,
        id: &str,
        tail: Option<usize>,
    ) -> Result<Vec<String>, RuntimeError> {
        // Read from container log file
        let log_path = self.state_root.join(id).join("container.log");
        // ... read and tail logs
    }

    async fn get_stats(&self, id: &str) -> Result<ContainerStats, RuntimeError> {
        // Read from cgroups
        let cgroup_path = format!("/sys/fs/cgroup/univrs/{}", id);
        // ... parse cgroup stats
    }
}
```

### 3. Image Manager

```rust
// container_runtime/src/image.rs

pub struct ImageManager {
    /// Local cache directory
    cache_dir: PathBuf,
    
    /// Registry credentials (optional)
    credentials: Option<RegistryCredentials>,
}

impl ImageManager {
    /// Pull an image from registry
    pub async fn pull(&self, image: &str) -> Result<ImageInfo, ImageError> {
        let (registry, repo, tag) = parse_image_ref(image)?;
        
        // Check if already cached
        if let Some(info) = self.get_cached(image).await? {
            return Ok(info);
        }
        
        // Pull manifest
        let manifest = self.fetch_manifest(&registry, &repo, &tag).await?;
        
        // Pull layers
        for layer in &manifest.layers {
            self.pull_layer(&registry, &repo, layer).await?;
        }
        
        // Extract to cache
        let rootfs = self.extract_layers(&manifest).await?;
        
        Ok(ImageInfo {
            id: manifest.config.digest.clone(),
            rootfs,
            config: manifest.config,
        })
    }
    
    /// Get rootfs path for an image
    pub async fn get_rootfs(&self, image: &str) -> Result<PathBuf, ImageError> {
        let info = self.pull(image).await?;
        Ok(info.rootfs)
    }
}
```

### 4. OCI Bundle Generation

```rust
// container_runtime/src/bundle.rs

use oci_spec::runtime::{Spec, Process, Root, Mount, LinuxBuilder, LinuxResourcesBuilder};

pub fn generate_oci_spec(config: &ContainerConfig) -> Result<Spec, BundleError> {
    let mut spec = Spec::default();
    
    // Process configuration
    let process = Process::default()
        .set_terminal(false)
        .set_user(oci_spec::runtime::User::default())
        .set_args(config.command.clone().unwrap_or_else(|| vec!["/bin/sh".into()]))
        .set_env(config.env_vars.iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect())
        .set_cwd("/".into());
    
    spec.set_process(Some(process));
    
    // Root filesystem
    spec.set_root(Some(Root::default()
        .set_path("rootfs".into())
        .set_readonly(false)));
    
    // Mounts
    let mounts = vec![
        Mount::default()
            .set_destination("/proc".into())
            .set_type(Some("proc".into()))
            .set_source(Some("proc".into())),
        Mount::default()
            .set_destination("/dev".into())
            .set_type(Some("tmpfs".into()))
            .set_source(Some("tmpfs".into())),
        // ... more standard mounts
    ];
    spec.set_mounts(Some(mounts));
    
    // Linux-specific (namespaces, cgroups)
    let linux = LinuxBuilder::default()
        .resources(LinuxResourcesBuilder::default()
            .cpu(oci_spec::runtime::LinuxCpuBuilder::default()
                .quota(Some((config.resources.cpu_cores * 100000.0) as i64))
                .period(Some(100000))
                .build()?)
            .memory(oci_spec::runtime::LinuxMemoryBuilder::default()
                .limit(Some((config.resources.memory_mb * 1024 * 1024) as i64))
                .build()?)
            .build()?)
        .namespaces(vec![
            oci_spec::runtime::LinuxNamespace { typ: "pid".into(), path: None },
            oci_spec::runtime::LinuxNamespace { typ: "mount".into(), path: None },
            oci_spec::runtime::LinuxNamespace { typ: "ipc".into(), path: None },
            oci_spec::runtime::LinuxNamespace { typ: "uts".into(), path: None },
            // Note: no network namespace for now (host networking)
        ])
        .build()?;
    
    spec.set_linux(Some(linux));
    
    Ok(spec)
}
```

---

## Integration Points

### 1. Feature Flag

```toml
# container_runtime/Cargo.toml

[features]
default = ["mock"]
mock = []
youki = ["oci-spec", "bollard", "flate2", "tar"]
```

### 2. Runtime Selection

```rust
// orchestrator_core/src/bin/node.rs

fn create_runtime(config: &NodeConfig) -> Arc<dyn ContainerRuntime> {
    #[cfg(feature = "youki")]
    {
        Arc::new(YoukiRuntime::new(YoukiConfig {
            bundle_root: config.data_dir.join("bundles"),
            state_root: config.data_dir.join("state"),
            image_cache: config.data_dir.join("images"),
        }).expect("Failed to initialize Youki runtime"))
    }
    
    #[cfg(not(feature = "youki"))]
    {
        Arc::new(MockRuntime::new())
    }
}
```

### 3. Reconciler Changes

The reconciler already calls `container_runtime.create_container()` and `start_container()`. No changes needed - just swap the implementation.

---

## Directory Structure

```
/var/lib/univrs/
├── bundles/                    # OCI bundles (one per container)
│   └── {container_id}/
│       ├── config.json         # OCI runtime spec
│       └── rootfs/             # Container filesystem
├── state/                      # Container state
│   └── {container_id}/
│       ├── state.json          # Runtime state
│       └── container.log       # Stdout/stderr logs
├── images/                     # Image cache
│   ├── layers/                 # Extracted layers
│   │   └── {digest}/
│   └── manifests/              # Image manifests
│       └── {image_ref}.json
└── tmp/                        # Temporary files
```

---

## Dependencies

```toml
[dependencies]
# OCI spec types
oci-spec = "0.6"

# Container image handling
bollard = "0.15"           # Docker registry client (for pulling)
flate2 = "1.0"             # Gzip decompression
tar = "0.4"                # Tar extraction

# Or use youki's libcontainer directly
libcontainer = { git = "https://github.com/containers/youki" }
```

---

## Implementation Phases

### Phase 1: Basic Execution (Week 1)

- [ ] YoukiRuntime struct with youki CLI wrapper
- [ ] OCI bundle generation from ContainerConfig
- [ ] create/start/stop/delete lifecycle
- [ ] Basic logging (stdout/stderr capture)

### Phase 2: Image Management (Week 1-2)

- [ ] Image reference parsing
- [ ] Pull from Docker Hub (public images)
- [ ] Layer extraction and caching
- [ ] Rootfs assembly

### Phase 3: Resource Enforcement (Week 2)

- [ ] CPU limits via cgroups
- [ ] Memory limits via cgroups
- [ ] Disk quotas (if supported)
- [ ] Container stats collection

### Phase 4: Production Hardening (Week 3)

- [ ] Seccomp profiles
- [ ] AppArmor/SELinux integration
- [ ] Private registry authentication
- [ ] Health check execution

---

## Testing Strategy

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_oci_spec_generation() {
        let config = ContainerConfig {
            image: "nginx:latest".into(),
            command: Some(vec!["nginx".into(), "-g".into(), "daemon off;".into()]),
            env_vars: HashMap::from([("PORT".into(), "8080".into())]),
            resources: ResourceRequirements {
                cpu_cores: 0.5,
                memory_mb: 256,
                disk_mb: 1024,
            },
            ..Default::default()
        };
        
        let spec = generate_oci_spec(&config).unwrap();
        
        assert_eq!(spec.process().as_ref().unwrap().args(), 
                   &Some(vec!["nginx".into(), "-g".into(), "daemon off;".into()]));
    }
}
```

### Integration Tests

```rust
#[tokio::test]
#[ignore] // Requires youki installed
async fn test_container_lifecycle() {
    let runtime = YoukiRuntime::new(YoukiConfig::default()).unwrap();
    
    // Create
    runtime.create_container("test-1", &ContainerConfig {
        image: "alpine:latest".into(),
        command: Some(vec!["sleep".into(), "30".into()]),
        ..Default::default()
    }).await.unwrap();
    
    // Start
    runtime.start_container("test-1").await.unwrap();
    assert_eq!(runtime.get_status("test-1").await.unwrap(), ContainerStatus::Running);
    
    // Stop
    runtime.stop_container("test-1", Some(Duration::from_secs(5))).await.unwrap();
    assert_eq!(runtime.get_status("test-1").await.unwrap(), ContainerStatus::Stopped);
    
    // Delete
    runtime.remove_container("test-1").await.unwrap();
}
```

---

## Risks & Mitigations

| Risk | Mitigation |
|------|------------|
| Youki API instability | Pin to specific version, abstract behind trait |
| Root privileges required | Document requirements, support rootless mode later |
| Image pull failures | Retry logic, fallback registries |
| Resource leak on crash | Cleanup on startup, periodic garbage collection |
| cgroup v1 vs v2 | Detect and handle both versions |

---

## Success Criteria

1. `orch deploy --image nginx:latest` creates a running nginx container
2. `curl localhost:<port>` returns nginx response
3. `orch logs <workload>` shows nginx access logs
4. `orch status` shows "Running" state
5. Container respects CPU/memory limits
6. Container is cleaned up on delete

---

## References

- [Youki GitHub](https://github.com/containers/youki)
- [OCI Runtime Spec](https://github.com/opencontainers/runtime-spec)
- [OCI Image Spec](https://github.com/opencontainers/image-spec)
- [libcontainer Documentation](https://docs.rs/libcontainer)

---

*Youki Integration Design v0.1.0*  
*Making containers real*
