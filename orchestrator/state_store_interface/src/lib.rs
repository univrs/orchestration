use async_trait::async_trait;
use orchestrator_shared_types::{
    Node, NodeId, OrchestrationError, Result, WorkloadDefinition, WorkloadId, WorkloadInstance,
};
use std::sync::Arc;
use thiserror::Error;

/// Errors specific to state store operations
#[derive(Debug, Error)]
pub enum StateStoreError {
    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Already exists: {0}")]
    AlreadyExists(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Connection error: {0}")]
    ConnectionError(String),

    #[error("Transaction error: {0}")]
    TransactionError(String),

    #[error("Internal error: {0}")]
    InternalError(String),
}

impl From<StateStoreError> for OrchestrationError {
    fn from(err: StateStoreError) -> Self {
        OrchestrationError::StateError(err.to_string())
    }
}

/// Core trait for persistent state management
///
/// This trait provides the abstraction layer for storing and retrieving
/// orchestration state. Implementations can be in-memory (for testing),
/// etcd-backed (for production), or any other distributed key-value store.
#[async_trait]
pub trait StateStore: Send + Sync {
    // ===== Initialization =====

    /// Initialize the state store connection
    async fn initialize(&self) -> Result<()>;

    /// Check if the state store is healthy
    async fn health_check(&self) -> Result<bool>;

    // ===== Node Operations =====

    /// Store or update a node
    async fn put_node(&self, node: Node) -> Result<()>;

    /// Get a node by ID
    async fn get_node(&self, node_id: &NodeId) -> Result<Option<Node>>;

    /// List all nodes
    async fn list_nodes(&self) -> Result<Vec<Node>>;

    /// Delete a node by ID
    async fn delete_node(&self, node_id: &NodeId) -> Result<()>;

    // ===== Workload Operations =====

    /// Store or update a workload definition
    async fn put_workload(&self, workload: WorkloadDefinition) -> Result<()>;

    /// Get a workload definition by ID
    async fn get_workload(&self, workload_id: &WorkloadId) -> Result<Option<WorkloadDefinition>>;

    /// List all workload definitions
    async fn list_workloads(&self) -> Result<Vec<WorkloadDefinition>>;

    /// Delete a workload definition
    async fn delete_workload(&self, workload_id: &WorkloadId) -> Result<()>;

    // ===== Instance Operations =====

    /// Store or update a workload instance
    async fn put_instance(&self, instance: WorkloadInstance) -> Result<()>;

    /// Get a specific instance by ID
    async fn get_instance(&self, instance_id: &str) -> Result<Option<WorkloadInstance>>;

    /// List all instances for a specific workload
    async fn list_instances_for_workload(
        &self,
        workload_id: &WorkloadId,
    ) -> Result<Vec<WorkloadInstance>>;

    /// List all instances across all workloads
    async fn list_all_instances(&self) -> Result<Vec<WorkloadInstance>>;

    /// Delete a workload instance
    async fn delete_instance(&self, instance_id: &str) -> Result<()>;

    /// Delete all instances for a workload (bulk operation)
    async fn delete_instances_for_workload(&self, workload_id: &WorkloadId) -> Result<()>;

    // ===== Batch Operations (for efficiency) =====

    /// Batch put operations for multiple instances
    async fn put_instances_batch(&self, instances: Vec<WorkloadInstance>) -> Result<()> {
        // Default implementation calls put_instance sequentially
        // Implementations can override for better performance
        for instance in instances {
            self.put_instance(instance).await?;
        }
        Ok(())
    }

    // ===== Watch/Subscribe (optional, for future event-driven updates) =====
    // These can be no-ops for simple implementations

    /// Subscribe to node changes (optional)
    async fn watch_nodes(&self) -> Result<tokio::sync::mpsc::Receiver<Node>> {
        Err(OrchestrationError::NotImplemented(
            "watch_nodes not implemented".to_string(),
        ))
    }

    /// Subscribe to workload changes (optional)
    async fn watch_workloads(&self) -> Result<tokio::sync::mpsc::Receiver<WorkloadDefinition>> {
        Err(OrchestrationError::NotImplemented(
            "watch_workloads not implemented".to_string(),
        ))
    }
}

// Re-export implementations based on features
#[cfg(feature = "sqlite")]
pub mod sqlite_store;

#[cfg(feature = "in-memory")]
pub mod in_memory;

#[cfg(feature = "etcd")]
pub mod etcd_store;

// Re-export SQLite store as the default when enabled
#[cfg(feature = "sqlite")]
pub use sqlite_store::SqliteStateStore;

// Helper function to create appropriate store based on config (sync version for non-SQLite)
pub fn create_state_store(config: StateStoreConfig) -> Result<Arc<dyn StateStore>> {
    match config {
        #[cfg(feature = "sqlite")]
        StateStoreConfig::Sqlite { .. } => Err(OrchestrationError::ConfigError(
            "SQLite store requires async initialization. Use create_state_store_async instead."
                .to_string(),
        )),

        #[cfg(feature = "in-memory")]
        StateStoreConfig::InMemory => Ok(Arc::new(in_memory::InMemoryStateStore::new())),

        #[cfg(feature = "etcd")]
        StateStoreConfig::Etcd { endpoints } => {
            // Will be implemented in etcd_store module
            etcd_store::EtcdStateStore::new(endpoints)
                .map(|store| Arc::new(store) as Arc<dyn StateStore>)
        }

        #[allow(unreachable_patterns)]
        _ => Err(OrchestrationError::ConfigError(
            "State store configuration not supported with current features".to_string(),
        )),
    }
}

/// Async helper function to create appropriate store based on config
/// Required for SQLite backend which needs async initialization
pub async fn create_state_store_async(config: StateStoreConfig) -> Result<Arc<dyn StateStore>> {
    match config {
        #[cfg(feature = "sqlite")]
        StateStoreConfig::Sqlite { path } => {
            let store = sqlite_store::SqliteStateStore::open(path).await?;
            Ok(Arc::new(store) as Arc<dyn StateStore>)
        }

        #[cfg(feature = "in-memory")]
        StateStoreConfig::InMemory => Ok(Arc::new(in_memory::InMemoryStateStore::new())),

        #[cfg(feature = "etcd")]
        StateStoreConfig::Etcd { endpoints } => etcd_store::EtcdStateStore::new(endpoints)
            .map(|store| Arc::new(store) as Arc<dyn StateStore>),

        #[allow(unreachable_patterns)]
        _ => Err(OrchestrationError::ConfigError(
            "State store configuration not supported with current features".to_string(),
        )),
    }
}

/// Configuration for state store backends
#[derive(Debug, Clone)]
pub enum StateStoreConfig {
    /// SQLite backend (default, production-ready)
    /// Uses univrs-state SqliteStore for durable persistence
    #[cfg(feature = "sqlite")]
    Sqlite {
        /// Path to the SQLite database file
        path: std::path::PathBuf,
    },

    #[cfg(feature = "in-memory")]
    InMemory,

    #[cfg(feature = "etcd")]
    Etcd { endpoints: Vec<String> },
}
