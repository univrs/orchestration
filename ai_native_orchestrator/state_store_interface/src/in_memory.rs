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
#[derive(Clone)]
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

    // ===== Comprehensive Test Suite =====

    // --- Initialization and Health Check Tests ---

    #[tokio::test]
    async fn test_initialize_and_health_check() {
        let store = InMemoryStateStore::new();

        // Initialize should succeed
        assert!(store.initialize().await.is_ok());

        // Health check should return true
        let health = store.health_check().await.unwrap();
        assert!(health);
    }

    #[tokio::test]
    async fn test_default_trait_implementation() {
        let store = InMemoryStateStore::default();

        // Should work the same as new()
        assert!(store.health_check().await.unwrap());
    }

    // --- Edge Case Tests ---

    #[tokio::test]
    async fn test_get_nonexistent_node() {
        let store = InMemoryStateStore::new();
        let nonexistent_id = Uuid::new_v4();

        let result = store.get_node(&nonexistent_id).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_get_nonexistent_workload() {
        let store = InMemoryStateStore::new();
        let nonexistent_id = Uuid::new_v4();

        let result = store.get_workload(&nonexistent_id).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_get_nonexistent_instance() {
        let store = InMemoryStateStore::new();
        let nonexistent_id = "nonexistent-instance-id";

        let result = store.get_instance(nonexistent_id).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_delete_nonexistent_node() {
        let store = InMemoryStateStore::new();
        let nonexistent_id = Uuid::new_v4();

        // Should not error when deleting non-existent
        assert!(store.delete_node(&nonexistent_id).await.is_ok());
    }

    #[tokio::test]
    async fn test_delete_nonexistent_workload() {
        let store = InMemoryStateStore::new();
        let nonexistent_id = Uuid::new_v4();

        assert!(store.delete_workload(&nonexistent_id).await.is_ok());
    }

    #[tokio::test]
    async fn test_delete_nonexistent_instance() {
        let store = InMemoryStateStore::new();

        assert!(store.delete_instance("nonexistent").await.is_ok());
    }

    #[tokio::test]
    async fn test_empty_store_lists() {
        let store = InMemoryStateStore::new();

        assert!(store.list_nodes().await.unwrap().is_empty());
        assert!(store.list_workloads().await.unwrap().is_empty());
        assert!(store.list_all_instances().await.unwrap().is_empty());
    }

    // --- Update (Upsert) Tests ---

    #[tokio::test]
    async fn test_node_update_overwrites() {
        let store = InMemoryStateStore::new();
        let node_id = Uuid::new_v4();

        let node_v1 = Node {
            id: node_id,
            address: "10.0.0.1:8080".to_string(),
            status: NodeStatus::Ready,
            labels: HashMap::new(),
            resources_capacity: NodeResources { cpu_cores: 4.0, memory_mb: 8192, disk_mb: 100000 },
            resources_allocatable: NodeResources { cpu_cores: 4.0, memory_mb: 8192, disk_mb: 100000 },
        };

        let node_v2 = Node {
            id: node_id,
            address: "10.0.0.2:9090".to_string(), // Changed address
            status: NodeStatus::NotReady,          // Changed status
            labels: HashMap::new(),
            resources_capacity: NodeResources { cpu_cores: 8.0, memory_mb: 16384, disk_mb: 200000 },
            resources_allocatable: NodeResources { cpu_cores: 7.5, memory_mb: 15000, disk_mb: 180000 },
        };

        store.put_node(node_v1).await.unwrap();
        store.put_node(node_v2.clone()).await.unwrap();

        let retrieved = store.get_node(&node_id).await.unwrap().unwrap();
        assert_eq!(retrieved.address, "10.0.0.2:9090");
        assert_eq!(retrieved.status, NodeStatus::NotReady);
        assert_eq!(retrieved.resources_capacity.cpu_cores, 8.0);

        // Should still be only 1 node
        assert_eq!(store.list_nodes().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_workload_update_overwrites() {
        let store = InMemoryStateStore::new();
        let workload_id = Uuid::new_v4();

        let workload_v1 = WorkloadDefinition {
            id: workload_id,
            name: "workload-v1".to_string(),
            containers: vec![],
            replicas: 1,
            labels: HashMap::new(),
        };

        let workload_v2 = WorkloadDefinition {
            id: workload_id,
            name: "workload-v2".to_string(),
            containers: vec![],
            replicas: 5,
            labels: HashMap::new(),
        };

        store.put_workload(workload_v1).await.unwrap();
        store.put_workload(workload_v2).await.unwrap();

        let retrieved = store.get_workload(&workload_id).await.unwrap().unwrap();
        assert_eq!(retrieved.name, "workload-v2");
        assert_eq!(retrieved.replicas, 5);
        assert_eq!(store.list_workloads().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_instance_update_overwrites() {
        let store = InMemoryStateStore::new();
        let instance_id = Uuid::new_v4();
        let workload_id = Uuid::new_v4();

        let instance_v1 = WorkloadInstance {
            id: instance_id,
            workload_id,
            node_id: Uuid::new_v4(),
            container_ids: vec!["container-1".to_string()],
            status: WorkloadInstanceStatus::Pending,
        };

        let instance_v2 = WorkloadInstance {
            id: instance_id,
            workload_id,
            node_id: Uuid::new_v4(),
            container_ids: vec!["container-2".to_string(), "container-3".to_string()],
            status: WorkloadInstanceStatus::Running,
        };

        store.put_instance(instance_v1).await.unwrap();
        store.put_instance(instance_v2).await.unwrap();

        let retrieved = store.get_instance(&instance_id.to_string()).await.unwrap().unwrap();
        assert_eq!(retrieved.status, WorkloadInstanceStatus::Running);
        assert_eq!(retrieved.container_ids.len(), 2);
        assert_eq!(store.list_all_instances().await.unwrap().len(), 1);
    }

    // --- Multiple Items Tests ---

    #[tokio::test]
    async fn test_multiple_nodes() {
        let store = InMemoryStateStore::new();

        for i in 0..5 {
            let node = Node {
                id: Uuid::new_v4(),
                address: format!("10.0.0.{}:8080", i),
                status: NodeStatus::Ready,
                labels: HashMap::new(),
                resources_capacity: NodeResources { cpu_cores: 4.0, memory_mb: 8192, disk_mb: 100000 },
                resources_allocatable: NodeResources { cpu_cores: 4.0, memory_mb: 8192, disk_mb: 100000 },
            };
            store.put_node(node).await.unwrap();
        }

        let nodes = store.list_nodes().await.unwrap();
        assert_eq!(nodes.len(), 5);
    }

    #[tokio::test]
    async fn test_multiple_workloads() {
        let store = InMemoryStateStore::new();

        for i in 0..10 {
            let workload = WorkloadDefinition {
                id: Uuid::new_v4(),
                name: format!("workload-{}", i),
                containers: vec![],
                replicas: i as u32,
                labels: HashMap::new(),
            };
            store.put_workload(workload).await.unwrap();
        }

        let workloads = store.list_workloads().await.unwrap();
        assert_eq!(workloads.len(), 10);
    }

    #[tokio::test]
    async fn test_instances_for_multiple_workloads() {
        let store = InMemoryStateStore::new();

        let workload_1_id = Uuid::new_v4();
        let workload_2_id = Uuid::new_v4();

        // Create 3 instances for workload 1
        for _ in 0..3 {
            let instance = WorkloadInstance {
                id: Uuid::new_v4(),
                workload_id: workload_1_id,
                node_id: Uuid::new_v4(),
                container_ids: vec!["c1".to_string()],
                status: WorkloadInstanceStatus::Running,
            };
            store.put_instance(instance).await.unwrap();
        }

        // Create 2 instances for workload 2
        for _ in 0..2 {
            let instance = WorkloadInstance {
                id: Uuid::new_v4(),
                workload_id: workload_2_id,
                node_id: Uuid::new_v4(),
                container_ids: vec!["c2".to_string()],
                status: WorkloadInstanceStatus::Pending,
            };
            store.put_instance(instance).await.unwrap();
        }

        // Verify filtering by workload
        let w1_instances = store.list_instances_for_workload(&workload_1_id).await.unwrap();
        assert_eq!(w1_instances.len(), 3);
        assert!(w1_instances.iter().all(|i| i.workload_id == workload_1_id));

        let w2_instances = store.list_instances_for_workload(&workload_2_id).await.unwrap();
        assert_eq!(w2_instances.len(), 2);
        assert!(w2_instances.iter().all(|i| i.workload_id == workload_2_id));

        // Verify total count
        let all_instances = store.list_all_instances().await.unwrap();
        assert_eq!(all_instances.len(), 5);
    }

    // --- Batch Operations Tests ---

    #[tokio::test]
    async fn test_put_instances_batch() {
        let store = InMemoryStateStore::new();
        let workload_id = Uuid::new_v4();

        let instances: Vec<WorkloadInstance> = (0..5)
            .map(|i| WorkloadInstance {
                id: Uuid::new_v4(),
                workload_id,
                node_id: Uuid::new_v4(),
                container_ids: vec![format!("container-{}", i)],
                status: WorkloadInstanceStatus::Running,
            })
            .collect();

        store.put_instances_batch(instances.clone()).await.unwrap();

        let retrieved = store.list_all_instances().await.unwrap();
        assert_eq!(retrieved.len(), 5);

        // Verify each instance exists
        for instance in &instances {
            let found = store.get_instance(&instance.id.to_string()).await.unwrap();
            assert!(found.is_some());
        }
    }

    #[tokio::test]
    async fn test_put_instances_batch_empty() {
        let store = InMemoryStateStore::new();

        // Empty batch should succeed
        store.put_instances_batch(vec![]).await.unwrap();

        assert!(store.list_all_instances().await.unwrap().is_empty());
    }

    // --- Delete Instances for Workload Tests ---

    #[tokio::test]
    async fn test_delete_instances_for_workload() {
        let store = InMemoryStateStore::new();

        let workload_to_delete = Uuid::new_v4();
        let workload_to_keep = Uuid::new_v4();

        // Create instances for both workloads
        for _ in 0..3 {
            store.put_instance(WorkloadInstance {
                id: Uuid::new_v4(),
                workload_id: workload_to_delete,
                node_id: Uuid::new_v4(),
                container_ids: vec![],
                status: WorkloadInstanceStatus::Running,
            }).await.unwrap();
        }

        for _ in 0..2 {
            store.put_instance(WorkloadInstance {
                id: Uuid::new_v4(),
                workload_id: workload_to_keep,
                node_id: Uuid::new_v4(),
                container_ids: vec![],
                status: WorkloadInstanceStatus::Running,
            }).await.unwrap();
        }

        assert_eq!(store.list_all_instances().await.unwrap().len(), 5);

        // Delete instances for one workload
        store.delete_instances_for_workload(&workload_to_delete).await.unwrap();

        // Only instances from workload_to_keep should remain
        let remaining = store.list_all_instances().await.unwrap();
        assert_eq!(remaining.len(), 2);
        assert!(remaining.iter().all(|i| i.workload_id == workload_to_keep));

        // Deleting again should be idempotent
        store.delete_instances_for_workload(&workload_to_delete).await.unwrap();
        assert_eq!(store.list_all_instances().await.unwrap().len(), 2);
    }

    // --- Concurrent Access Tests ---

    #[tokio::test]
    async fn test_concurrent_reads() {
        use std::sync::Arc;

        let store = Arc::new(InMemoryStateStore::new());

        // Pre-populate with data
        for i in 0..10 {
            store.put_node(Node {
                id: Uuid::new_v4(),
                address: format!("10.0.0.{}:8080", i),
                status: NodeStatus::Ready,
                labels: HashMap::new(),
                resources_capacity: NodeResources { cpu_cores: 4.0, memory_mb: 8192, disk_mb: 100000 },
                resources_allocatable: NodeResources { cpu_cores: 4.0, memory_mb: 8192, disk_mb: 100000 },
            }).await.unwrap();
        }

        // Spawn multiple concurrent readers
        let mut handles = vec![];
        for _ in 0..10 {
            let store_clone = Arc::clone(&store);
            handles.push(tokio::spawn(async move {
                for _ in 0..100 {
                    let nodes = store_clone.list_nodes().await.unwrap();
                    assert_eq!(nodes.len(), 10);
                }
            }));
        }

        for handle in handles {
            handle.await.unwrap();
        }
    }

    #[tokio::test]
    async fn test_concurrent_writes() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicUsize, Ordering};

        let store = Arc::new(InMemoryStateStore::new());
        let counter = Arc::new(AtomicUsize::new(0));

        // Spawn multiple concurrent writers
        let mut handles = vec![];
        for _ in 0..10 {
            let store_clone = Arc::clone(&store);
            let counter_clone = Arc::clone(&counter);
            handles.push(tokio::spawn(async move {
                for _ in 0..10 {
                    let node = Node {
                        id: Uuid::new_v4(),
                        address: "10.0.0.1:8080".to_string(),
                        status: NodeStatus::Ready,
                        labels: HashMap::new(),
                        resources_capacity: NodeResources { cpu_cores: 4.0, memory_mb: 8192, disk_mb: 100000 },
                        resources_allocatable: NodeResources { cpu_cores: 4.0, memory_mb: 8192, disk_mb: 100000 },
                    };
                    store_clone.put_node(node).await.unwrap();
                    counter_clone.fetch_add(1, Ordering::SeqCst);
                }
            }));
        }

        for handle in handles {
            handle.await.unwrap();
        }

        // All writes should have completed
        assert_eq!(counter.load(Ordering::SeqCst), 100);

        // All nodes should be stored
        let nodes = store.list_nodes().await.unwrap();
        assert_eq!(nodes.len(), 100);
    }

    #[tokio::test]
    async fn test_concurrent_read_write() {
        use std::sync::Arc;

        let store = Arc::new(InMemoryStateStore::new());

        // Pre-populate
        let initial_node_id = Uuid::new_v4();
        store.put_node(Node {
            id: initial_node_id,
            address: "initial".to_string(),
            status: NodeStatus::Ready,
            labels: HashMap::new(),
            resources_capacity: NodeResources { cpu_cores: 4.0, memory_mb: 8192, disk_mb: 100000 },
            resources_allocatable: NodeResources { cpu_cores: 4.0, memory_mb: 8192, disk_mb: 100000 },
        }).await.unwrap();

        let mut handles = vec![];

        // Reader tasks
        for _ in 0..5 {
            let store_clone = Arc::clone(&store);
            let node_id = initial_node_id;
            handles.push(tokio::spawn(async move {
                for _ in 0..100 {
                    let node = store_clone.get_node(&node_id).await.unwrap();
                    assert!(node.is_some());
                }
            }));
        }

        // Writer tasks (adding new nodes)
        for _ in 0..5 {
            let store_clone = Arc::clone(&store);
            handles.push(tokio::spawn(async move {
                for _ in 0..20 {
                    store_clone.put_node(Node {
                        id: Uuid::new_v4(),
                        address: "new".to_string(),
                        status: NodeStatus::Ready,
                        labels: HashMap::new(),
                        resources_capacity: NodeResources { cpu_cores: 4.0, memory_mb: 8192, disk_mb: 100000 },
                        resources_allocatable: NodeResources { cpu_cores: 4.0, memory_mb: 8192, disk_mb: 100000 },
                    }).await.unwrap();
                }
            }));
        }

        for handle in handles {
            handle.await.unwrap();
        }

        // 1 initial + 100 new nodes
        let nodes = store.list_nodes().await.unwrap();
        assert_eq!(nodes.len(), 101);
    }

    // --- Data Integrity Tests ---

    #[tokio::test]
    async fn test_node_with_labels() {
        let store = InMemoryStateStore::new();

        let mut labels = HashMap::new();
        labels.insert("env".to_string(), "production".to_string());
        labels.insert("region".to_string(), "us-west-2".to_string());
        labels.insert("tier".to_string(), "compute".to_string());

        let node = Node {
            id: Uuid::new_v4(),
            address: "10.0.0.1:8080".to_string(),
            status: NodeStatus::Ready,
            labels: labels.clone(),
            resources_capacity: NodeResources { cpu_cores: 16.0, memory_mb: 65536, disk_mb: 1000000 },
            resources_allocatable: NodeResources { cpu_cores: 15.0, memory_mb: 60000, disk_mb: 900000 },
        };

        store.put_node(node.clone()).await.unwrap();

        let retrieved = store.get_node(&node.id).await.unwrap().unwrap();
        assert_eq!(retrieved.labels.len(), 3);
        assert_eq!(retrieved.labels.get("env"), Some(&"production".to_string()));
        assert_eq!(retrieved.labels.get("region"), Some(&"us-west-2".to_string()));
    }

    #[tokio::test]
    async fn test_instance_status_transitions() {
        let store = InMemoryStateStore::new();
        let instance_id = Uuid::new_v4();
        let workload_id = Uuid::new_v4();

        // Pending -> Running -> Failed -> Terminating
        let statuses = vec![
            WorkloadInstanceStatus::Pending,
            WorkloadInstanceStatus::Running,
            WorkloadInstanceStatus::Failed,
            WorkloadInstanceStatus::Terminating,
        ];

        for status in statuses {
            let instance = WorkloadInstance {
                id: instance_id,
                workload_id,
                node_id: Uuid::new_v4(),
                container_ids: vec!["container-1".to_string()],
                status: status.clone(),
            };

            store.put_instance(instance).await.unwrap();

            let retrieved = store.get_instance(&instance_id.to_string()).await.unwrap().unwrap();
            assert_eq!(retrieved.status, status);
        }

        // Still only 1 instance
        assert_eq!(store.list_all_instances().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_list_instances_empty_workload() {
        let store = InMemoryStateStore::new();
        let empty_workload_id = Uuid::new_v4();

        // Add instances for a different workload
        store.put_instance(WorkloadInstance {
            id: Uuid::new_v4(),
            workload_id: Uuid::new_v4(), // Different workload
            node_id: Uuid::new_v4(),
            container_ids: vec![],
            status: WorkloadInstanceStatus::Running,
        }).await.unwrap();

        // Query for empty workload should return empty list
        let instances = store.list_instances_for_workload(&empty_workload_id).await.unwrap();
        assert!(instances.is_empty());
    }
}
