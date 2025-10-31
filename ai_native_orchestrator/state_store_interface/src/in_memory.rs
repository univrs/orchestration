use async_trait::async_trait;
use orchestrator_shared_types::{
    Node, NodeId, Result, WorkloadDefinition, WorkloadId, WorkloadInstance,
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::StateStore;

/// In-memory implementation of StateStore
///
/// This implementation uses RwLock-protected HashMaps for thread-safe
/// in-memory storage. Suitable for testing, development, and single-node
/// deployments where persistence across restarts is not required.
pub struct InMemoryStateStore {
    nodes: Arc<RwLock<HashMap<NodeId, Node>>>,
    workloads: Arc<RwLock<HashMap<WorkloadId, WorkloadDefinition>>>,
    instances: Arc<RwLock<HashMap<String, WorkloadInstance>>>, // Key: instance.id.to_string()
}

impl InMemoryStateStore {
    /// Create a new in-memory state store
    pub fn new() -> Self {
        Self {
            nodes: Arc::new(RwLock::new(HashMap::new())),
            workloads: Arc::new(RwLock::new(HashMap::new())),
            instances: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl Default for InMemoryStateStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl StateStore for InMemoryStateStore {
    async fn initialize(&self) -> Result<()> {
        // No initialization needed for in-memory store
        Ok(())
    }

    async fn health_check(&self) -> Result<bool> {
        // In-memory store is always healthy
        Ok(true)
    }

    // ===== Node Operations =====

    async fn put_node(&self, node: Node) -> Result<()> {
        let mut nodes = self.nodes.write().await;
        nodes.insert(node.id, node);
        Ok(())
    }

    async fn get_node(&self, node_id: &NodeId) -> Result<Option<Node>> {
        let nodes = self.nodes.read().await;
        Ok(nodes.get(node_id).cloned())
    }

    async fn list_nodes(&self) -> Result<Vec<Node>> {
        let nodes = self.nodes.read().await;
        Ok(nodes.values().cloned().collect())
    }

    async fn delete_node(&self, node_id: &NodeId) -> Result<()> {
        let mut nodes = self.nodes.write().await;
        nodes.remove(node_id);
        Ok(())
    }

    // ===== Workload Operations =====

    async fn put_workload(&self, workload: WorkloadDefinition) -> Result<()> {
        let mut workloads = self.workloads.write().await;
        workloads.insert(workload.id, workload);
        Ok(())
    }

    async fn get_workload(&self, workload_id: &WorkloadId) -> Result<Option<WorkloadDefinition>> {
        let workloads = self.workloads.read().await;
        Ok(workloads.get(workload_id).cloned())
    }

    async fn list_workloads(&self) -> Result<Vec<WorkloadDefinition>> {
        let workloads = self.workloads.read().await;
        Ok(workloads.values().cloned().collect())
    }

    async fn delete_workload(&self, workload_id: &WorkloadId) -> Result<()> {
        let mut workloads = self.workloads.write().await;
        workloads.remove(workload_id);
        Ok(())
    }

    // ===== Instance Operations =====

    async fn put_instance(&self, instance: WorkloadInstance) -> Result<()> {
        let mut instances = self.instances.write().await;
        let key = instance.id.to_string();
        instances.insert(key, instance);
        Ok(())
    }

    async fn get_instance(&self, instance_id: &str) -> Result<Option<WorkloadInstance>> {
        let instances = self.instances.read().await;
        Ok(instances.get(instance_id).cloned())
    }

    async fn list_instances_for_workload(&self, workload_id: &WorkloadId) -> Result<Vec<WorkloadInstance>> {
        let instances = self.instances.read().await;
        Ok(instances
            .values()
            .filter(|inst| &inst.workload_id == workload_id)
            .cloned()
            .collect())
    }

    async fn list_all_instances(&self) -> Result<Vec<WorkloadInstance>> {
        let instances = self.instances.read().await;
        Ok(instances.values().cloned().collect())
    }

    async fn delete_instance(&self, instance_id: &str) -> Result<()> {
        let mut instances = self.instances.write().await;
        instances.remove(instance_id);
        Ok(())
    }

    async fn delete_instances_for_workload(&self, workload_id: &WorkloadId) -> Result<()> {
        let mut instances = self.instances.write().await;
        instances.retain(|_, inst| &inst.workload_id != workload_id);
        Ok(())
    }

    async fn put_instances_batch(&self, instances_batch: Vec<WorkloadInstance>) -> Result<()> {
        let mut instances = self.instances.write().await;
        for instance in instances_batch {
            let key = instance.id.to_string();
            instances.insert(key, instance);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use orchestrator_shared_types::{NodeStatus, NodeResources, WorkloadInstanceStatus};
    use std::collections::HashMap;
    use uuid::Uuid;

    #[tokio::test]
    async fn test_node_operations() {
        let store = InMemoryStateStore::new();

        let node_id = Uuid::new_v4();
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
        assert_eq!(retrieved.unwrap().id, node_id);

        // List nodes
        let nodes = store.list_nodes().await.unwrap();
        assert_eq!(nodes.len(), 1);

        // Delete node
        store.delete_node(&node_id).await.unwrap();
        let retrieved = store.get_node(&node_id).await.unwrap();
        assert!(retrieved.is_none());
    }

    #[tokio::test]
    async fn test_workload_operations() {
        let store = InMemoryStateStore::new();

        let workload_id = Uuid::new_v4();
        let workload = WorkloadDefinition {
            id: workload_id,
            name: "test-workload".to_string(),
            containers: vec![],
            replicas: 3,
            labels: HashMap::new(),
        };

        // Put workload
        store.put_workload(workload.clone()).await.unwrap();

        // Get workload
        let retrieved = store.get_workload(&workload_id).await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().name, "test-workload");

        // List workloads
        let workloads = store.list_workloads().await.unwrap();
        assert_eq!(workloads.len(), 1);

        // Delete workload
        store.delete_workload(&workload_id).await.unwrap();
        let retrieved = store.get_workload(&workload_id).await.unwrap();
        assert!(retrieved.is_none());
    }

    #[tokio::test]
    async fn test_instance_operations() {
        let store = InMemoryStateStore::new();

        let workload_id = Uuid::new_v4();
        let instance = WorkloadInstance {
            id: Uuid::new_v4(),
            workload_id,
            node_id: Uuid::new_v4(),
            container_ids: vec!["container-123".to_string()],
            status: WorkloadInstanceStatus::Running,
        };

        let instance_id = instance.id.to_string();

        // Put instance
        store.put_instance(instance.clone()).await.unwrap();

        // Get instance
        let retrieved = store.get_instance(&instance_id).await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().id, instance.id);

        // List instances for workload
        let instances = store.list_instances_for_workload(&workload_id).await.unwrap();
        assert_eq!(instances.len(), 1);

        // Delete instance
        store.delete_instance(&instance_id).await.unwrap();
        let retrieved = store.get_instance(&instance_id).await.unwrap();
        assert!(retrieved.is_none());
    }
}
