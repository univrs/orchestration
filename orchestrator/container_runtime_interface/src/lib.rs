use async_trait::async_trait;
use orchestrator_shared_types::{
    ContainerConfig, ContainerId, NodeId, OrchestrationError, Result, WorkloadId,
};
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

/// Options for retrieving container logs.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LogOptions {
    /// Return only the last N lines.
    pub tail: Option<usize>,
    /// Include timestamps in output.
    pub timestamps: bool,
    /// Only return logs since this timestamp (RFC3339).
    pub since: Option<String>,
    /// Only return logs until this timestamp (RFC3339).
    pub until: Option<String>,
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

    /// Gets container logs.
    /// Returns the log content as a string.
    async fn get_container_logs(
        &self,
        container_id: &ContainerId,
        options: &LogOptions,
    ) -> Result<String> {
        // Default implementation - logs not supported
        let _ = (container_id, options);
        Err(OrchestrationError::RuntimeError(
            "Log retrieval not supported by this runtime".to_string(),
        ))
    }

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
