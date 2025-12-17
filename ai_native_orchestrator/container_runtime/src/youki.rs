//! Youki container runtime adapter using libcontainer.
//!
//! This provides a real container runtime implementation that creates
//! OCI-compliant containers using the libcontainer library (Youki's core).
//!
//! # Architecture
//!
//! - Containers are created in a root directory per node
//! - Each container gets a bundle directory with OCI spec and rootfs
//! - libcontainer is synchronous, so we use `spawn_blocking` for async
//!
//! # Requirements
//!
//! - Linux with cgroups v2
//! - Root privileges (or appropriate capabilities)
//! - Container images must be pre-extracted (image pulling not implemented)

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use container_runtime_interface::{ContainerRuntime, ContainerStatus, CreateContainerOptions};
use orchestrator_shared_types::{
    ContainerConfig, ContainerId, NodeId, OrchestrationError, Result,
};

// Youki/libcontainer imports
use libcontainer::container::builder::ContainerBuilder;
use libcontainer::syscall::syscall::SyscallType;
use nix::sys::signal::Signal;
use oci_spec::runtime::{
    LinuxBuilder, LinuxNamespaceBuilder, LinuxNamespaceType, ProcessBuilder, RootBuilder, Spec,
    SpecBuilder, UserBuilder,
};

/// Metadata about a managed container
#[derive(Debug, Clone)]
struct ContainerInfo {
    _id: ContainerId,
    node_id: NodeId,
    bundle_path: PathBuf,
    state: String,
}

/// Configuration for the Youki runtime
#[derive(Debug, Clone)]
pub struct YoukiConfig {
    /// Base directory for container data (default: /var/lib/orchestrator)
    pub base_dir: PathBuf,
    /// Root directory for container state (default: /run/orchestrator)
    pub state_dir: PathBuf,
    /// Whether to run containers in detached mode
    pub detached: bool,
}

impl Default for YoukiConfig {
    fn default() -> Self {
        Self {
            base_dir: PathBuf::from("/var/lib/orchestrator"),
            state_dir: PathBuf::from("/run/orchestrator"),
            detached: true,
        }
    }
}

/// Youki-based container runtime using libcontainer.
pub struct YoukiRuntime {
    config: YoukiConfig,
    /// Track all containers by ID
    containers: Arc<RwLock<HashMap<ContainerId, ContainerInfo>>>,
    /// Track containers by node
    containers_by_node: Arc<RwLock<HashMap<NodeId, Vec<ContainerId>>>>,
}

impl YoukiRuntime {
    /// Create a new Youki runtime with default configuration.
    pub fn new() -> Self {
        Self::with_config(YoukiConfig::default())
    }

    /// Create a new Youki runtime with custom configuration.
    pub fn with_config(config: YoukiConfig) -> Self {
        Self {
            config,
            containers: Arc::new(RwLock::new(HashMap::new())),
            containers_by_node: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get the bundle directory path for a container on a node.
    fn bundle_path(&self, node_id: &NodeId, container_id: &str) -> PathBuf {
        self.config
            .base_dir
            .join("nodes")
            .join(node_id.to_string())
            .join("bundles")
            .join(container_id)
    }

    /// Get the root path for container state.
    fn root_path(&self, node_id: &NodeId) -> PathBuf {
        self.config.state_dir.join("nodes").join(node_id.to_string())
    }

    /// Get the full container state path (root + container_id).
    fn container_state_path(&self, node_id: &NodeId, container_id: &str) -> PathBuf {
        self.root_path(node_id).join(container_id)
    }

    /// Convert our ContainerConfig to an OCI runtime spec.
    fn create_oci_spec(&self, config: &ContainerConfig) -> Result<Spec> {
        // Build the process specification
        let mut process_builder = ProcessBuilder::default()
            .terminal(false)
            .user(
                UserBuilder::default()
                    .uid(0u32)
                    .gid(0u32)
                    .build()
                    .map_err(|e| OrchestrationError::RuntimeError(e.to_string()))?,
            )
            .cwd("/".to_string());

        // Set command and args
        let args: Vec<String> = if let Some(ref cmd) = config.command {
            let mut args = cmd.clone();
            if let Some(ref extra_args) = config.args {
                args.extend(extra_args.clone());
            }
            args
        } else {
            // Default to sh if no command specified
            vec!["/bin/sh".to_string()]
        };
        process_builder = process_builder.args(args);

        // Set environment variables
        let env: Vec<String> = config
            .env_vars
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .chain(std::iter::once("PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin".to_string()))
            .collect();
        process_builder = process_builder.env(env);

        let process = process_builder
            .build()
            .map_err(|e| OrchestrationError::RuntimeError(e.to_string()))?;

        // Build root filesystem specification
        let root = RootBuilder::default()
            .path("rootfs".to_string())
            .readonly(false)
            .build()
            .map_err(|e| OrchestrationError::RuntimeError(e.to_string()))?;

        // Build Linux namespaces
        let namespaces = vec![
            LinuxNamespaceBuilder::default()
                .typ(LinuxNamespaceType::Pid)
                .build()
                .map_err(|e| OrchestrationError::RuntimeError(e.to_string()))?,
            LinuxNamespaceBuilder::default()
                .typ(LinuxNamespaceType::Network)
                .build()
                .map_err(|e| OrchestrationError::RuntimeError(e.to_string()))?,
            LinuxNamespaceBuilder::default()
                .typ(LinuxNamespaceType::Ipc)
                .build()
                .map_err(|e| OrchestrationError::RuntimeError(e.to_string()))?,
            LinuxNamespaceBuilder::default()
                .typ(LinuxNamespaceType::Uts)
                .build()
                .map_err(|e| OrchestrationError::RuntimeError(e.to_string()))?,
            LinuxNamespaceBuilder::default()
                .typ(LinuxNamespaceType::Mount)
                .build()
                .map_err(|e| OrchestrationError::RuntimeError(e.to_string()))?,
        ];

        let linux = LinuxBuilder::default()
            .namespaces(namespaces)
            .build()
            .map_err(|e| OrchestrationError::RuntimeError(e.to_string()))?;

        // Build the full spec
        let spec = SpecBuilder::default()
            .version("1.0.2".to_string())
            .root(root)
            .process(process)
            .linux(linux)
            .hostname(config.name.clone())
            .build()
            .map_err(|e| OrchestrationError::RuntimeError(e.to_string()))?;

        Ok(spec)
    }

    /// Create the bundle directory structure for a container.
    async fn create_bundle(
        &self,
        bundle_path: &Path,
        config: &ContainerConfig,
    ) -> Result<()> {
        let bundle = bundle_path.to_path_buf();
        let rootfs_path = bundle.join("rootfs");
        let config_path = bundle.join("config.json");
        let container_config = config.clone();

        // Use spawn_blocking for filesystem operations
        let spec = self.create_oci_spec(&container_config)?;

        tokio::task::spawn_blocking(move || {
            // Create directories
            std::fs::create_dir_all(&rootfs_path).map_err(|e| {
                OrchestrationError::RuntimeError(format!(
                    "Failed to create rootfs directory: {}",
                    e
                ))
            })?;

            // Create minimal rootfs structure
            // In production, this would be populated by image extraction
            for dir in &["bin", "etc", "lib", "proc", "sys", "tmp", "var", "dev"] {
                std::fs::create_dir_all(rootfs_path.join(dir)).map_err(|e| {
                    OrchestrationError::RuntimeError(format!(
                        "Failed to create {} directory: {}",
                        dir, e
                    ))
                })?;
            }

            // Write OCI config.json
            let spec_json = serde_json::to_string_pretty(&spec).map_err(|e| {
                OrchestrationError::RuntimeError(format!("Failed to serialize OCI spec: {}", e))
            })?;

            std::fs::write(&config_path, spec_json).map_err(|e| {
                OrchestrationError::RuntimeError(format!("Failed to write config.json: {}", e))
            })?;

            Ok::<(), OrchestrationError>(())
        })
        .await
        .map_err(|e| OrchestrationError::RuntimeError(format!("Task join error: {}", e)))??;

        Ok(())
    }

    /// Clean up a container's bundle directory.
    async fn cleanup_bundle(&self, bundle_path: &Path) -> Result<()> {
        let bundle = bundle_path.to_path_buf();

        tokio::task::spawn_blocking(move || {
            if bundle.exists() {
                std::fs::remove_dir_all(&bundle).map_err(|e| {
                    OrchestrationError::RuntimeError(format!(
                        "Failed to remove bundle directory: {}",
                        e
                    ))
                })?;
            }
            Ok::<(), OrchestrationError>(())
        })
        .await
        .map_err(|e| OrchestrationError::RuntimeError(format!("Task join error: {}", e)))??;

        Ok(())
    }
}

impl Default for YoukiRuntime {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ContainerRuntime for YoukiRuntime {
    async fn init_node(&self, node_id: NodeId) -> Result<()> {
        info!("YoukiRuntime: Initializing node {}", node_id);

        let root_path = self.root_path(&node_id);
        let bundles_path = self.config.base_dir.join("nodes").join(node_id.to_string()).join("bundles");

        // Create directories in blocking task
        tokio::task::spawn_blocking(move || {
            std::fs::create_dir_all(&root_path).map_err(|e| {
                OrchestrationError::RuntimeError(format!(
                    "Failed to create state directory: {}",
                    e
                ))
            })?;
            std::fs::create_dir_all(&bundles_path).map_err(|e| {
                OrchestrationError::RuntimeError(format!(
                    "Failed to create bundles directory: {}",
                    e
                ))
            })?;
            Ok::<(), OrchestrationError>(())
        })
        .await
        .map_err(|e| OrchestrationError::RuntimeError(format!("Task join error: {}", e)))??;

        // Initialize node container list
        let mut by_node = self.containers_by_node.write().await;
        by_node.entry(node_id).or_insert_with(Vec::new);

        Ok(())
    }

    async fn create_container(
        &self,
        config: &ContainerConfig,
        options: &CreateContainerOptions,
    ) -> Result<ContainerId> {
        let container_id = format!("{}-{}", config.name, Uuid::new_v4());

        info!(
            "YoukiRuntime: Creating container {} for workload {} on node {}",
            container_id, options.workload_id, options.node_id
        );

        let bundle_path = self.bundle_path(&options.node_id, &container_id);
        let root_path = self.root_path(&options.node_id);

        // Create bundle with OCI spec
        self.create_bundle(&bundle_path, config).await?;

        // Create and start container using libcontainer
        let cid = container_id.clone();
        let bundle = bundle_path.clone();
        let root = root_path.clone();
        let detached = self.config.detached;

        let result = tokio::task::spawn_blocking(move || {
            // Build the container
            let container_result = ContainerBuilder::new(cid.clone(), SyscallType::default())
                .with_root_path(root)
                .map_err(|e| {
                    OrchestrationError::RuntimeError(format!("Failed to set root path: {}", e))
                })?
                .as_init(&bundle)
                .with_detach(detached)
                .build();

            match container_result {
                Ok(mut container) => {
                    // Start the container
                    container.start().map_err(|e| {
                        OrchestrationError::RuntimeError(format!(
                            "Failed to start container: {}",
                            e
                        ))
                    })?;
                    Ok(())
                }
                Err(e) => Err(OrchestrationError::RuntimeError(format!(
                    "Failed to build container: {}",
                    e
                ))),
            }
        })
        .await
        .map_err(|e| OrchestrationError::RuntimeError(format!("Task join error: {}", e)))?;

        if let Err(e) = result {
            // Clean up on failure
            warn!("Container creation failed, cleaning up bundle: {}", e);
            let _ = self.cleanup_bundle(&bundle_path).await;
            return Err(e);
        }

        // Track the container
        let container_info = ContainerInfo {
            _id: container_id.clone(),
            node_id: options.node_id,
            bundle_path,
            state: "running".to_string(),
        };

        self.containers
            .write()
            .await
            .insert(container_id.clone(), container_info);

        self.containers_by_node
            .write()
            .await
            .entry(options.node_id)
            .or_insert_with(Vec::new)
            .push(container_id.clone());

        Ok(container_id)
    }

    async fn stop_container(&self, container_id: &ContainerId) -> Result<()> {
        info!("YoukiRuntime: Stopping container {}", container_id);

        let containers = self.containers.read().await;
        let info = containers.get(container_id).ok_or_else(|| {
            OrchestrationError::RuntimeError(format!("Container not found: {}", container_id))
        })?;

        let container_state_path = self.container_state_path(&info.node_id, container_id);

        drop(containers);

        // Stop container using libcontainer
        tokio::task::spawn_blocking(move || {
            let mut container =
                libcontainer::container::Container::load(container_state_path).map_err(|e| {
                    OrchestrationError::RuntimeError(format!("Failed to load container: {}", e))
                })?;

            // Send SIGTERM first, then SIGKILL if needed
            if container.can_kill() {
                container.kill(Signal::SIGTERM, false).map_err(|e| {
                    OrchestrationError::RuntimeError(format!("Failed to kill container: {}", e))
                })?;
            }

            Ok::<(), OrchestrationError>(())
        })
        .await
        .map_err(|e| OrchestrationError::RuntimeError(format!("Task join error: {}", e)))??;

        // Update state
        let mut containers = self.containers.write().await;
        if let Some(info) = containers.get_mut(container_id) {
            info.state = "stopped".to_string();
        }

        Ok(())
    }

    async fn remove_container(&self, container_id: &ContainerId) -> Result<()> {
        info!("YoukiRuntime: Removing container {}", container_id);

        let containers = self.containers.read().await;
        let info = containers.get(container_id).ok_or_else(|| {
            OrchestrationError::RuntimeError(format!("Container not found: {}", container_id))
        })?;

        let container_state_path = self.container_state_path(&info.node_id, container_id);
        let bundle_path = info.bundle_path.clone();
        let node_id = info.node_id;

        drop(containers);

        // Delete container using libcontainer
        tokio::task::spawn_blocking(move || {
            let mut container =
                libcontainer::container::Container::load(container_state_path).map_err(|e| {
                    OrchestrationError::RuntimeError(format!("Failed to load container: {}", e))
                })?;

            if container.can_delete() {
                container.delete(true).map_err(|e| {
                    OrchestrationError::RuntimeError(format!("Failed to delete container: {}", e))
                })?;
            }

            Ok::<(), OrchestrationError>(())
        })
        .await
        .map_err(|e| OrchestrationError::RuntimeError(format!("Task join error: {}", e)))??;

        // Clean up bundle
        self.cleanup_bundle(&bundle_path).await?;

        // Remove from tracking
        self.containers.write().await.remove(container_id);

        let mut by_node = self.containers_by_node.write().await;
        if let Some(node_containers) = by_node.get_mut(&node_id) {
            node_containers.retain(|id| id != container_id);
        }

        Ok(())
    }

    async fn get_container_status(&self, container_id: &ContainerId) -> Result<ContainerStatus> {
        debug!("YoukiRuntime: Getting status for container {}", container_id);

        let containers = self.containers.read().await;
        let info = containers.get(container_id).ok_or_else(|| {
            OrchestrationError::RuntimeError(format!("Container not found: {}", container_id))
        })?;

        let container_state_path = self.container_state_path(&info.node_id, container_id);
        let cid = container_id.clone();

        drop(containers);

        // Get status from libcontainer
        let status = tokio::task::spawn_blocking(move || {
            match libcontainer::container::Container::load(container_state_path) {
                Ok(container) => {
                    let state = match container.status() {
                        libcontainer::container::ContainerStatus::Creating => "creating",
                        libcontainer::container::ContainerStatus::Created => "created",
                        libcontainer::container::ContainerStatus::Running => "running",
                        libcontainer::container::ContainerStatus::Stopped => "stopped",
                        libcontainer::container::ContainerStatus::Paused => "paused",
                    };

                    Ok::<ContainerStatus, OrchestrationError>(ContainerStatus {
                        id: cid,
                        state: state.to_string(),
                        exit_code: None, // Would need to read from container state
                        error_message: None,
                    })
                }
                Err(e) => {
                    error!("Failed to load container for status: {}", e);
                    Ok::<ContainerStatus, OrchestrationError>(ContainerStatus {
                        id: cid,
                        state: "unknown".to_string(),
                        exit_code: None,
                        error_message: Some(e.to_string()),
                    })
                }
            }
        })
        .await
        .map_err(|e| OrchestrationError::RuntimeError(format!("Task join error: {}", e)))??;

        Ok(status)
    }

    async fn list_containers(&self, node_id: NodeId) -> Result<Vec<ContainerStatus>> {
        debug!("YoukiRuntime: Listing containers for node {}", node_id);

        let by_node = self.containers_by_node.read().await;
        let container_ids = by_node.get(&node_id).cloned().unwrap_or_default();

        drop(by_node);

        let mut statuses = Vec::new();
        for container_id in container_ids {
            match self.get_container_status(&container_id).await {
                Ok(status) => statuses.push(status),
                Err(e) => {
                    warn!("Failed to get status for container {}: {}", container_id, e);
                    statuses.push(ContainerStatus {
                        id: container_id,
                        state: "unknown".to_string(),
                        exit_code: None,
                        error_message: Some(e.to_string()),
                    });
                }
            }
        }

        Ok(statuses)
    }
}

// Note: Unit tests for YoukiRuntime require root privileges and a proper
// Linux environment with cgroups. Integration tests should be run in a
// controlled environment (container, VM, or CI with appropriate setup).
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_youki_config_default() {
        let config = YoukiConfig::default();
        assert_eq!(config.base_dir, PathBuf::from("/var/lib/orchestrator"));
        assert_eq!(config.state_dir, PathBuf::from("/run/orchestrator"));
        assert!(config.detached);
    }

    #[test]
    fn test_bundle_path() {
        let runtime = YoukiRuntime::new();
        let node_id = Uuid::new_v4();
        let container_id = "test-container";

        let path = runtime.bundle_path(&node_id, container_id);
        assert!(path.to_string_lossy().contains(&node_id.to_string()));
        assert!(path.to_string_lossy().contains(container_id));
    }

    #[test]
    fn test_root_path() {
        let runtime = YoukiRuntime::new();
        let node_id = Uuid::new_v4();

        let path = runtime.root_path(&node_id);
        assert!(path.to_string_lossy().contains(&node_id.to_string()));
    }

    #[tokio::test]
    async fn test_create_oci_spec() {
        let runtime = YoukiRuntime::new();
        let config = ContainerConfig {
            name: "test".to_string(),
            image: "alpine:latest".to_string(),
            command: Some(vec!["/bin/sh".to_string()]),
            args: Some(vec!["-c".to_string(), "echo hello".to_string()]),
            env_vars: [("FOO".to_string(), "bar".to_string())]
                .into_iter()
                .collect(),
            ports: vec![],
            resource_requests: orchestrator_shared_types::NodeResources::default(),
        };

        let spec = runtime.create_oci_spec(&config).unwrap();
        assert_eq!(spec.version(), "1.0.2");
        assert!(spec.hostname().as_ref().map(|h| h == "test").unwrap_or(false));
    }
}
