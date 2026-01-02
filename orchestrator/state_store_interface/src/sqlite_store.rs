//! SQLite implementation of StateStore using univrs-state.
//!
//! This is the default, production-ready storage backend.
//! Uses SQLite via univrs-state for persistence.

use async_trait::async_trait;
use orchestrator_shared_types::{
    Node, NodeId, Result, OrchestrationError, WorkloadDefinition, WorkloadId, WorkloadInstance,
};
use std::path::Path;
use std::sync::Arc;
use univrs_state::{SqliteStore, StateStore as UnivrsStateStore};

use crate::StateStore;

/// SQLite-backed implementation of StateStore.
///
/// Uses univrs-state SqliteStore for the underlying storage,
/// providing durable persistence with ACID transactions.
#[derive(Clone)]
pub struct SqliteStateStore {
    store: Arc<SqliteStore>,
}

impl SqliteStateStore {
    /// Open or create a SQLite state store at the given path.
    pub async fn open(path: impl AsRef<Path>) -> Result<Self> {
        let store = SqliteStore::open(path)
            .await
            .map_err(|e| OrchestrationError::StateError(e.to_string()))?;
        Ok(Self {
            store: Arc::new(store),
        })
    }

    /// Create an in-memory SQLite store (for testing).
    pub async fn in_memory() -> Result<Self> {
        let store = SqliteStore::in_memory()
            .await
            .map_err(|e| OrchestrationError::StateError(e.to_string()))?;
        Ok(Self {
            store: Arc::new(store),
        })
    }

    fn node_key(node_id: &NodeId) -> String {
        format!("/nodes/{}", node_id.to_base58())
    }

    fn workload_key(workload_id: &WorkloadId) -> String {
        format!("/workloads/{}", workload_id)
    }

    fn instance_key(instance_id: &str) -> String {
        format!("/instances/{}", instance_id)
    }
}

#[async_trait]
impl StateStore for SqliteStateStore {
    async fn initialize(&self) -> Result<()> {
        // Schema is auto-created by univrs-state
        Ok(())
    }

    async fn health_check(&self) -> Result<bool> {
        // Try a simple operation to verify connectivity
        match self.store.list("/").await {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    // ===== Node Operations =====

    async fn put_node(&self, node: Node) -> Result<()> {
        let key = Self::node_key(&node.id);
        self.store
            .set_json(&key, &node)
            .await
            .map_err(|e| OrchestrationError::StateError(e.to_string()))?;
        Ok(())
    }

    async fn get_node(&self, node_id: &NodeId) -> Result<Option<Node>> {
        let key = Self::node_key(node_id);
        self.store
            .get_json(&key)
            .await
            .map_err(|e| OrchestrationError::StateError(e.to_string()))
    }

    async fn list_nodes(&self) -> Result<Vec<Node>> {
        let keys = self
            .store
            .list("/nodes/")
            .await
            .map_err(|e| OrchestrationError::StateError(e.to_string()))?;

        let mut nodes = Vec::new();
        for key in keys {
            if let Some(node) = self
                .store
                .get_json::<Node>(&key)
                .await
                .map_err(|e| OrchestrationError::StateError(e.to_string()))?
            {
                nodes.push(node);
            }
        }
        Ok(nodes)
    }

    async fn delete_node(&self, node_id: &NodeId) -> Result<()> {
        let key = Self::node_key(node_id);
        self.store
            .delete(&key)
            .await
            .map_err(|e| OrchestrationError::StateError(e.to_string()))
    }

    // ===== Workload Operations =====

    async fn put_workload(&self, workload: WorkloadDefinition) -> Result<()> {
        let key = Self::workload_key(&workload.id);
        self.store
            .set_json(&key, &workload)
            .await
            .map_err(|e| OrchestrationError::StateError(e.to_string()))?;
        Ok(())
    }

    async fn get_workload(&self, workload_id: &WorkloadId) -> Result<Option<WorkloadDefinition>> {
        let key = Self::workload_key(workload_id);
        self.store
            .get_json(&key)
            .await
            .map_err(|e| OrchestrationError::StateError(e.to_string()))
    }

    async fn list_workloads(&self) -> Result<Vec<WorkloadDefinition>> {
        let keys = self
            .store
            .list("/workloads/")
            .await
            .map_err(|e| OrchestrationError::StateError(e.to_string()))?;

        let mut workloads = Vec::new();
        for key in keys {
            if let Some(workload) = self
                .store
                .get_json::<WorkloadDefinition>(&key)
                .await
                .map_err(|e| OrchestrationError::StateError(e.to_string()))?
            {
                workloads.push(workload);
            }
        }
        Ok(workloads)
    }

    async fn delete_workload(&self, workload_id: &WorkloadId) -> Result<()> {
        let key = Self::workload_key(workload_id);
        self.store
            .delete(&key)
            .await
            .map_err(|e| OrchestrationError::StateError(e.to_string()))
    }

    // ===== Instance Operations =====

    async fn put_instance(&self, instance: WorkloadInstance) -> Result<()> {
        let key = Self::instance_key(&instance.id.to_string());
        self.store
            .set_json(&key, &instance)
            .await
            .map_err(|e| OrchestrationError::StateError(e.to_string()))?;
        Ok(())
    }

    async fn get_instance(&self, instance_id: &str) -> Result<Option<WorkloadInstance>> {
        let key = Self::instance_key(instance_id);
        self.store
            .get_json(&key)
            .await
            .map_err(|e| OrchestrationError::StateError(e.to_string()))
    }

    async fn list_instances_for_workload(
        &self,
        workload_id: &WorkloadId,
    ) -> Result<Vec<WorkloadInstance>> {
        let all_instances = self.list_all_instances().await?;
        Ok(all_instances
            .into_iter()
            .filter(|inst| &inst.workload_id == workload_id)
            .collect())
    }

    async fn list_all_instances(&self) -> Result<Vec<WorkloadInstance>> {
        let keys = self
            .store
            .list("/instances/")
            .await
            .map_err(|e| OrchestrationError::StateError(e.to_string()))?;

        let mut instances = Vec::new();
        for key in keys {
            if let Some(instance) = self
                .store
                .get_json::<WorkloadInstance>(&key)
                .await
                .map_err(|e| OrchestrationError::StateError(e.to_string()))?
            {
                instances.push(instance);
            }
        }
        Ok(instances)
    }

    async fn delete_instance(&self, instance_id: &str) -> Result<()> {
        let key = Self::instance_key(instance_id);
        self.store
            .delete(&key)
            .await
            .map_err(|e| OrchestrationError::StateError(e.to_string()))
    }

    async fn delete_instances_for_workload(&self, workload_id: &WorkloadId) -> Result<()> {
        let instances = self.list_instances_for_workload(workload_id).await?;
        for instance in instances {
            self.delete_instance(&instance.id.to_string()).await?;
        }
        Ok(())
    }

    async fn put_instances_batch(&self, instances: Vec<WorkloadInstance>) -> Result<()> {
        for instance in instances {
            self.put_instance(instance).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use orchestrator_shared_types::{NodeResources, NodeStatus, WorkloadInstanceStatus, Keypair};
    use std::collections::HashMap;
    use uuid::Uuid;

    fn generate_node_id() -> NodeId {
        Keypair::generate().public_key()
    }

    #[tokio::test]
    async fn test_sqlite_node_operations() {
        let store = SqliteStateStore::in_memory().await.unwrap();
        store.initialize().await.unwrap();

        let node_id = generate_node_id();
        let node = Node {
            id: node_id,
            address: "10.0.0.1:8080".to_string(),
            status: NodeStatus::Ready,
            labels: HashMap::new(),
            resources_capacity: NodeResources {
                cpu_cores: 4.0,
                memory_mb: 8192,
                disk_mb: 100000,
            },
            resources_allocatable: NodeResources {
                cpu_cores: 3.8,
                memory_mb: 7000,
                disk_mb: 90000,
            },
        };

        // Put node
        store.put_node(node.clone()).await.unwrap();

        // Get node
        let retrieved = store.get_node(&node_id).await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().address, "10.0.0.1:8080");

        // List nodes
        let nodes = store.list_nodes().await.unwrap();
        assert_eq!(nodes.len(), 1);

        // Delete node
        store.delete_node(&node_id).await.unwrap();
        let retrieved = store.get_node(&node_id).await.unwrap();
        assert!(retrieved.is_none());
    }

    #[tokio::test]
    async fn test_sqlite_workload_operations() {
        let store = SqliteStateStore::in_memory().await.unwrap();

        let workload_id = Uuid::new_v4();
        let workload = WorkloadDefinition {
            id: workload_id,
            name: "test-workload".to_string(),
            containers: vec![],
            replicas: 3,
            labels: HashMap::new(),
        };

        store.put_workload(workload.clone()).await.unwrap();

        let retrieved = store.get_workload(&workload_id).await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().name, "test-workload");

        let workloads = store.list_workloads().await.unwrap();
        assert_eq!(workloads.len(), 1);

        store.delete_workload(&workload_id).await.unwrap();
        assert!(store.get_workload(&workload_id).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_sqlite_instance_operations() {
        let store = SqliteStateStore::in_memory().await.unwrap();

        let workload_id = Uuid::new_v4();
        let node_id = generate_node_id();
        let instance = WorkloadInstance {
            id: Uuid::new_v4(),
            workload_id,
            node_id,
            container_ids: vec!["container-123".to_string()],
            status: WorkloadInstanceStatus::Running,
        };

        let instance_id = instance.id.to_string();

        store.put_instance(instance.clone()).await.unwrap();

        let retrieved = store.get_instance(&instance_id).await.unwrap();
        assert!(retrieved.is_some());

        let instances = store.list_instances_for_workload(&workload_id).await.unwrap();
        assert_eq!(instances.len(), 1);

        store.delete_instance(&instance_id).await.unwrap();
        assert!(store.get_instance(&instance_id).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_sqlite_health_check() {
        let store = SqliteStateStore::in_memory().await.unwrap();
        assert!(store.health_check().await.unwrap());
    }
}
