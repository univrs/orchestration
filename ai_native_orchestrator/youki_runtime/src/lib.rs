use async_trait::async_trait;
use container_runtime_interface::{ContainerRuntime, ContainerStatus, CreateContainerOptions};
use orchestrator_shared_types::{ContainerId, NodeId, OrchestrationError, Result, ContainerConfig};
use oci_spec::runtime::{
    LinuxBuilder, ProcessBuilder, RootBuilder, Spec, SpecBuilder,
};
use serde_json;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tempfile::TempDir;
use tokio::fs;
use tokio::process::Command;
use tracing::{debug, error, info, warn};

/// Youki runtime implementation
///
/// This implementation provides container lifecycle management using Youki,
/// a Rust-based OCI runtime. It generates OCI-compliant runtime specifications
/// and manages container execution through the Youki binary.
pub struct YoukiRuntime {
    /// Path to the youki binary
    youki_binary: PathBuf,

    /// Root directory for container state
    state_dir: PathBuf,

    /// Root directory for container bundles
    bundle_dir: PathBuf,

    /// Per-node state tracking
    node_states: tokio::sync::RwLock<HashMap<NodeId, NodeRuntimeState>>,
}

/// Runtime state per node
#[derive(Debug, Default)]
struct NodeRuntimeState {
    /// Containers running on this node
    containers: HashMap<ContainerId, ContainerMetadata>,
}

/// Metadata for a running container
#[derive(Debug, Clone)]
struct ContainerMetadata {
    bundle_path: PathBuf,
    container_id: ContainerId,
    node_id: NodeId,
    _temp_dir: Option<String>, // Path to temp dir (kept for cleanup)
}

impl YoukiRuntime {
    /// Create a new Youki runtime instance
    ///
    /// # Arguments
    /// * `youki_binary` - Path to youki executable (defaults to "youki" in PATH)
    /// * `state_dir` - Directory for runtime state
    /// * `bundle_dir` - Directory for container bundles
    pub fn new(
        youki_binary: Option<PathBuf>,
        state_dir: Option<PathBuf>,
        bundle_dir: Option<PathBuf>,
    ) -> Result<Self> {
        let youki_binary = youki_binary.unwrap_or_else(|| PathBuf::from("youki"));
        let state_dir = state_dir.unwrap_or_else(|| PathBuf::from("/var/run/orchestrator/youki"));
        let bundle_dir = bundle_dir.unwrap_or_else(|| PathBuf::from("/var/lib/orchestrator/bundles"));

        Ok(Self {
            youki_binary,
            state_dir,
            bundle_dir,
            node_states: tokio::sync::RwLock::new(HashMap::new()),
        })
    }

    /// Generate OCI runtime specification from container config
    fn generate_oci_spec(&self, config: &ContainerConfig) -> Result<Spec> {
        // Build process configuration
        let args: Vec<String> = if let Some(cmd) = &config.command {
            cmd.clone()
        } else {
            // Default to sh if no command specified
            vec!["/bin/sh".to_string()]
        };

        let mut process_builder = ProcessBuilder::default()
            .args(args)
            .cwd("/".to_string());

        // Add environment variables
        let mut env_vars = vec![
            "PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin".to_string(),
        ];
        for (key, value) in &config.env_vars {
            env_vars.push(format!("{}={}", key, value));
        }
        process_builder = process_builder.env(env_vars);

        let process = process_builder
            .build()
            .map_err(|e| OrchestrationError::RuntimeError(format!("Failed to build process spec: {}", e)))?;

        // Build root filesystem configuration
        let root = RootBuilder::default()
            .path("rootfs")
            .readonly(false)
            .build()
            .map_err(|e| OrchestrationError::RuntimeError(format!("Failed to build root spec: {}", e)))?;

        // Build Linux-specific configuration
        let linux = LinuxBuilder::default()
            .build()
            .map_err(|e| OrchestrationError::RuntimeError(format!("Failed to build Linux spec: {}", e)))?;

        // Build complete OCI spec
        let spec = SpecBuilder::default()
            .version("1.0.2".to_string())
            .process(process)
            .root(root)
            .linux(linux)
            .build()
            .map_err(|e| OrchestrationError::RuntimeError(format!("Failed to build OCI spec: {}", e)))?;

        Ok(spec)
    }

    /// Create container bundle directory with config.json and rootfs
    async fn create_bundle(
        &self,
        container_id: &ContainerId,
        config: &ContainerConfig,
    ) -> Result<PathBuf> {
        // Create bundle directory
        let bundle_path = self.bundle_dir.join(container_id);
        fs::create_dir_all(&bundle_path)
            .await
            .map_err(|e| OrchestrationError::RuntimeError(format!("Failed to create bundle dir: {}", e)))?;

        // Generate OCI spec
        let spec = self.generate_oci_spec(config)?;

        // Write config.json
        let config_path = bundle_path.join("config.json");
        let config_json = serde_json::to_string_pretty(&spec)
            .map_err(|e| OrchestrationError::RuntimeError(format!("Failed to serialize OCI spec: {}", e)))?;

        fs::write(&config_path, config_json)
            .await
            .map_err(|e| OrchestrationError::RuntimeError(format!("Failed to write config.json: {}", e)))?;

        // Create rootfs directory
        let rootfs_path = bundle_path.join("rootfs");
        fs::create_dir_all(&rootfs_path)
            .await
            .map_err(|e| OrchestrationError::RuntimeError(format!("Failed to create rootfs dir: {}", e)))?;

        // TODO: Extract container image to rootfs
        // For now, we'll need to handle image pulling/extraction separately
        // This is a placeholder - in production, you'd use containerd/podman/skopeo
        // to extract the image specified in config.image to rootfs_path

        info!("Created bundle at {:?} for container {}", bundle_path, container_id);

        Ok(bundle_path)
    }

    /// Execute youki command
    async fn run_youki_command(&self, args: &[&str]) -> Result<std::process::Output> {
        debug!("Executing youki command: {:?} {:?}", self.youki_binary, args);

        let output = Command::new(&self.youki_binary)
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|e| OrchestrationError::RuntimeError(format!("Failed to execute youki: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            error!("Youki command failed: {}", stderr);
            return Err(OrchestrationError::RuntimeError(format!(
                "Youki command failed: {}",
                stderr
            )));
        }

        Ok(output)
    }

    /// Parse container state from youki state output
    fn parse_container_state(&self, state_json: &str) -> Result<ContainerStatus> {
        // Parse the JSON output from `youki state`
        let state: serde_json::Value = serde_json::from_str(state_json)
            .map_err(|e| OrchestrationError::RuntimeError(format!("Failed to parse state JSON: {}", e)))?;

        let id = state["id"]
            .as_str()
            .ok_or_else(|| OrchestrationError::RuntimeError("Missing container ID in state".to_string()))?
            .to_string();

        let status = state["status"]
            .as_str()
            .unwrap_or("unknown")
            .to_string();

        Ok(ContainerStatus {
            id,
            state: status,
            exit_code: None,
            error_message: None,
        })
    }
}

#[async_trait]
impl ContainerRuntime for YoukiRuntime {
    async fn init_node(&self, node_id: NodeId) -> Result<()> {
        info!("Initializing Youki runtime on node {}", node_id);

        // Ensure state directory exists
        fs::create_dir_all(&self.state_dir)
            .await
            .map_err(|e| OrchestrationError::RuntimeError(format!("Failed to create state dir: {}", e)))?;

        // Ensure bundle directory exists
        fs::create_dir_all(&self.bundle_dir)
            .await
            .map_err(|e| OrchestrationError::RuntimeError(format!("Failed to create bundle dir: {}", e)))?;

        // Initialize node state
        let mut node_states = self.node_states.write().await;
        node_states.insert(node_id, NodeRuntimeState::default());

        info!("Youki runtime initialized on node {}", node_id);
        Ok(())
    }

    async fn create_container(
        &self,
        config: &ContainerConfig,
        options: &CreateContainerOptions,
    ) -> Result<ContainerId> {
        let container_id = format!("container-{}", uuid::Uuid::new_v4());

        info!(
            "Creating container {} for workload {} on node {}",
            container_id, options.workload_id, options.node_id
        );

        // Create bundle
        let bundle_path = self.create_bundle(&container_id, config).await?;

        // Create container with youki
        let bundle_str = bundle_path.to_string_lossy();
        let root_str = self.state_dir.to_string_lossy();

        self.run_youki_command(&[
            "--root",
            &root_str,
            "create",
            &container_id,
            "--bundle",
            &bundle_str,
        ])
        .await?;

        info!("Container {} created successfully", container_id);

        // Start container
        self.run_youki_command(&[
            "--root",
            &root_str,
            "start",
            &container_id,
        ])
        .await?;

        info!("Container {} started successfully", container_id);

        // Store container metadata
        let mut node_states = self.node_states.write().await;
        if let Some(node_state) = node_states.get_mut(&options.node_id) {
            node_state.containers.insert(
                container_id.clone(),
                ContainerMetadata {
                    bundle_path: bundle_path.clone(),
                    container_id: container_id.clone(),
                    node_id: options.node_id,
                    _temp_dir: None,
                },
            );
        }

        Ok(container_id)
    }

    async fn stop_container(&self, container_id: &ContainerId) -> Result<()> {
        info!("Stopping container {}", container_id);

        let root_str = self.state_dir.to_string_lossy();

        // Kill the container (SIGTERM)
        self.run_youki_command(&[
            "--root",
            &root_str,
            "kill",
            container_id,
            "SIGTERM",
        ])
        .await?;

        info!("Container {} stopped successfully", container_id);
        Ok(())
    }

    async fn remove_container(&self, container_id: &ContainerId) -> Result<()> {
        info!("Removing container {}", container_id);

        let root_str = self.state_dir.to_string_lossy();

        // Delete the container
        self.run_youki_command(&[
            "--root",
            &root_str,
            "delete",
            container_id,
        ])
        .await?;

        // Clean up bundle directory
        let bundle_path = self.bundle_dir.join(container_id);
        if bundle_path.exists() {
            fs::remove_dir_all(&bundle_path)
                .await
                .map_err(|e| {
                    warn!("Failed to remove bundle directory: {}", e);
                    OrchestrationError::RuntimeError(format!("Failed to remove bundle: {}", e))
                })?;
        }

        // Remove from node state
        let mut node_states = self.node_states.write().await;
        for node_state in node_states.values_mut() {
            node_state.containers.remove(container_id);
        }

        info!("Container {} removed successfully", container_id);
        Ok(())
    }

    async fn get_container_status(&self, container_id: &ContainerId) -> Result<ContainerStatus> {
        debug!("Getting status for container {}", container_id);

        let root_str = self.state_dir.to_string_lossy();

        let output = self.run_youki_command(&[
            "--root",
            &root_str,
            "state",
            container_id,
        ])
        .await?;

        let state_json = String::from_utf8_lossy(&output.stdout);
        self.parse_container_state(&state_json)
    }

    async fn list_containers(&self, node_id: NodeId) -> Result<Vec<ContainerStatus>> {
        debug!("Listing containers on node {}", node_id);

        let node_states = self.node_states.read().await;
        let container_ids: Vec<ContainerId> = if let Some(node_state) = node_states.get(&node_id) {
            node_state.containers.keys().cloned().collect()
        } else {
            vec![]
        };

        let mut statuses = Vec::new();
        for container_id in container_ids {
            match self.get_container_status(&container_id).await {
                Ok(status) => statuses.push(status),
                Err(e) => {
                    warn!("Failed to get status for container {}: {:?}", container_id, e);
                }
            }
        }

        Ok(statuses)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use orchestrator_shared_types::{PortMapping, NodeResources};

    #[test]
    fn test_oci_spec_generation() {
        let runtime = YoukiRuntime::new(None, None, None).unwrap();

        let config = ContainerConfig {
            name: "test-container".to_string(),
            image: "alpine:latest".to_string(),
            command: Some(vec!["/bin/sh".to_string(), "-c".to_string(), "echo hello".to_string()]),
            args: None,
            env_vars: vec![
                ("FOO".to_string(), "bar".to_string()),
                ("BAZ".to_string(), "qux".to_string()),
            ]
            .into_iter()
            .collect(),
            ports: vec![],
            resource_requests: NodeResources::default(),
        };

        let spec = runtime.generate_oci_spec(&config).unwrap();

        assert_eq!(spec.version(), "1.0.2");
        assert!(spec.process().is_some());
        assert!(spec.root().is_some());
    }
}
