use async_trait::async_trait;
use etcd_client::{Client, GetOptions, Error as EtcdError};
use orchestrator_shared_types::{
    Node, NodeId, OrchestrationError, Result, WorkloadDefinition, WorkloadId, WorkloadInstance,
};
use serde_json;
use std::sync::Arc;

use crate::{StateStore, StateStoreError};

/// Etcd-backed implementation of StateStore
///
/// This implementation uses etcd for distributed, persistent state storage.
/// Suitable for production multi-node deployments with high availability.
pub struct EtcdStateStore {
    client: Arc<tokio::sync::Mutex<Client>>,
    prefix: String, // Key prefix for namespacing (e.g., "/orchestrator/")
}

impl EtcdStateStore {
    /// Create a new etcd-backed state store
    ///
    /// # Arguments
    /// * `endpoints` - List of etcd endpoints (e.g., ["127.0.0.1:2379"])
    pub async fn new(endpoints: Vec<String>) -> Result<Self> {
        let client = Client::connect(endpoints, None)
            .await
            .map_err(|e| StateStoreError::ConnectionError(format!("Failed to connect to etcd: {}", e)))?;

        Ok(Self {
            client: Arc::new(tokio::sync::Mutex::new(client)),
            prefix: "/orchestrator".to_string(),
        })
    }

    /// Create with custom prefix
    pub async fn with_prefix(endpoints: Vec<String>, prefix: String) -> Result<Self> {
        let client = Client::connect(endpoints, None)
            .await
            .map_err(|e| StateStoreError::ConnectionError(format!("Failed to connect to etcd: {}", e)))?;

        Ok(Self {
            client: Arc::new(tokio::sync::Mutex::new(client)),
            prefix,
        })
    }

    // Helper methods for key construction
    fn node_key(&self, node_id: &NodeId) -> String {
        format!("{}/nodes/{}", self.prefix, node_id)
    }

    fn nodes_prefix(&self) -> String {
        format!("{}/nodes/", self.prefix)
    }

    fn workload_key(&self, workload_id: &WorkloadId) -> String {
        format!("{}/workloads/{}", self.prefix, workload_id)
    }

    fn workloads_prefix(&self) -> String {
        format!("{}/workloads/", self.prefix)
    }

    fn instance_key(&self, instance_id: &str) -> String {
        format!("{}/instances/{}", self.prefix, instance_id)
    }

    fn instances_prefix(&self) -> String {
        format!("{}/instances/", self.prefix)
    }

    fn instances_for_workload_prefix(&self, workload_id: &WorkloadId) -> String {
        format!("{}/instances_by_workload/{}/", self.prefix, workload_id)
    }

    // Helper to serialize and put a value
    async fn put_value<T: serde::Serialize>(&self, key: String, value: &T) -> Result<()> {
        let json = serde_json::to_string(value)
            .map_err(|e| StateStoreError::SerializationError(e.to_string()))?;

        let mut client = self.client.lock().await;
        client
            .put(key, json, None)
            .await
            .map_err(|e| StateStoreError::InternalError(format!("etcd put failed: {}", e)))?;

        Ok(())
    }

    // Helper to get and deserialize a value
    async fn get_value<T: serde::de::DeserializeOwned>(&self, key: String) -> Result<Option<T>> {
        let mut client = self.client.lock().await;
        let response = client
            .get(key, None)
            .await
            .map_err(|e| StateStoreError::InternalError(format!("etcd get failed: {}", e)))?;

        if let Some(kv) = response.kvs().first() {
            let json = kv.value_str()
                .map_err(|e| StateStoreError::SerializationError(format!("Invalid UTF-8: {}", e)))?;

            let value: T = serde_json::from_str(json)
                .map_err(|e| StateStoreError::SerializationError(e.to_string()))?;

            Ok(Some(value))
        } else {
            Ok(None)
        }
    }

    // Helper to list all values with a prefix
    async fn list_with_prefix<T: serde::de::DeserializeOwned>(&self, prefix: String) -> Result<Vec<T>> {
        let mut client = self.client.lock().await;
        let get_options = GetOptions::new().with_prefix();

        let response = client
            .get(prefix, Some(get_options))
            .await
            .map_err(|e| StateStoreError::InternalError(format!("etcd list failed: {}", e)))?;

        let mut results = Vec::new();
        for kv in response.kvs() {
            let json = kv.value_str()
                .map_err(|e| StateStoreError::SerializationError(format!("Invalid UTF-8: {}", e)))?;

            let value: T = serde_json::from_str(json)
                .map_err(|e| StateStoreError::SerializationError(e.to_string()))?;

            results.push(value);
        }

        Ok(results)
    }

    // Helper to delete a key
    async fn delete_key(&self, key: String) -> Result<()> {
        let mut client = self.client.lock().await;
        client
            .delete(key, None)
            .await
            .map_err(|e| StateStoreError::InternalError(format!("etcd delete failed: {}", e)))?;

        Ok(())
    }
}

#[async_trait]
impl StateStore for EtcdStateStore {
    async fn initialize(&self) -> Result<()> {
        // Test connection by attempting a health check
        self.health_check().await?;
        Ok(())
    }

    async fn health_check(&self) -> Result<bool> {
        let mut client = self.client.lock().await;
        // Try a simple get operation to test connectivity
        match client.get(format!("{}/health", self.prefix), None).await {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    // ===== Node Operations =====

    async fn put_node(&self, node: Node) -> Result<()> {
        let key = self.node_key(&node.id);
        self.put_value(key, &node).await
    }

    async fn get_node(&self, node_id: &NodeId) -> Result<Option<Node>> {
        let key = self.node_key(node_id);
        self.get_value(key).await
    }

    async fn list_nodes(&self) -> Result<Vec<Node>> {
        let prefix = self.nodes_prefix();
        self.list_with_prefix(prefix).await
    }

    async fn delete_node(&self, node_id: &NodeId) -> Result<()> {
        let key = self.node_key(node_id);
        self.delete_key(key).await
    }

    // ===== Workload Operations =====

    async fn put_workload(&self, workload: WorkloadDefinition) -> Result<()> {
        let key = self.workload_key(&workload.id);
        self.put_value(key, &workload).await
    }

    async fn get_workload(&self, workload_id: &WorkloadId) -> Result<Option<WorkloadDefinition>> {
        let key = self.workload_key(workload_id);
        self.get_value(key).await
    }

    async fn list_workloads(&self) -> Result<Vec<WorkloadDefinition>> {
        let prefix = self.workloads_prefix();
        self.list_with_prefix(prefix).await
    }

    async fn delete_workload(&self, workload_id: &WorkloadId) -> Result<()> {
        let key = self.workload_key(workload_id);
        self.delete_key(key).await
    }

    // ===== Instance Operations =====

    async fn put_instance(&self, instance: WorkloadInstance) -> Result<()> {
        let key = self.instance_key(&instance.id.to_string());
        // Also store under workload-specific prefix for efficient lookups
        let workload_key = format!(
            "{}/instances_by_workload/{}/{}",
            self.prefix, instance.workload_id, instance.id
        );

        // Store in both locations
        self.put_value(key, &instance).await?;
        self.put_value(workload_key, &instance).await?;

        Ok(())
    }

    async fn get_instance(&self, instance_id: &str) -> Result<Option<WorkloadInstance>> {
        let key = self.instance_key(instance_id);
        self.get_value(key).await
    }

    async fn list_instances_for_workload(&self, workload_id: &WorkloadId) -> Result<Vec<WorkloadInstance>> {
        let prefix = self.instances_for_workload_prefix(workload_id);
        self.list_with_prefix(prefix).await
    }

    async fn list_all_instances(&self) -> Result<Vec<WorkloadInstance>> {
        let prefix = self.instances_prefix();
        self.list_with_prefix(prefix).await
    }

    async fn delete_instance(&self, instance_id: &str) -> Result<()> {
        // Need to delete from both locations
        // First get the instance to know its workload_id
        if let Some(instance) = self.get_instance(instance_id).await? {
            let key = self.instance_key(instance_id);
            let workload_key = format!(
                "{}/instances_by_workload/{}/{}",
                self.prefix, instance.workload_id, instance.id
            );

            self.delete_key(key).await?;
            self.delete_key(workload_key).await?;
        }

        Ok(())
    }

    async fn delete_instances_for_workload(&self, workload_id: &WorkloadId) -> Result<()> {
        // Get all instances for this workload
        let instances = self.list_instances_for_workload(workload_id).await?;

        // Delete each instance
        for instance in instances {
            self.delete_instance(&instance.id.to_string()).await?;
        }

        Ok(())
    }

    async fn put_instances_batch(&self, instances: Vec<WorkloadInstance>) -> Result<()> {
        // Etcd supports transactions, but for simplicity we'll do sequential writes
        // TODO: Optimize with etcd transactions
        for instance in instances {
            self.put_instance(instance).await?;
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

    // Note: These tests require a running etcd instance
    // Run with: docker run -d -p 2379:2379 --name etcd quay.io/coreos/etcd:latest

    async fn create_test_store() -> EtcdStateStore {
        EtcdStateStore::with_prefix(
            vec!["127.0.0.1:2379".to_string()],
            "/test_orchestrator".to_string(),
        )
        .await
        .expect("Failed to create etcd store")
    }

    #[tokio::test]
    #[ignore] // Ignore by default since it requires etcd
    async fn test_etcd_node_operations() {
        let store = create_test_store().await;

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

        // Clean up
        store.delete_node(&node_id).await.unwrap();
    }
}
