# Consensus Design

**OpenRaft Integration for Distributed State**

Version: 0.1.0  
Date: December 18, 2025  
Status: Design Document (Phase 2)

---

## Overview

This document outlines the integration of [OpenRaft](https://github.com/databendlabs/openraft), a Rust implementation of the Raft consensus algorithm, into the Univrs orchestrator. This provides:

- **Leader election** - Automatic failover when nodes die
- **Log replication** - All state changes replicated to followers
- **Consistency** - Linearizable reads and writes
- **Persistence** - State survives node restarts

### Current State (Gossip Only)

```
┌─────────────────────────────────────────────────────────────────────────┐
│                      CURRENT: GOSSIP-ONLY CLUSTER                        │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│   Node A              Node B              Node C                        │
│   ┌─────────┐        ┌─────────┐        ┌─────────┐                    │
│   │ State   │        │ State   │        │ State   │                    │
│   │ (local) │        │ (local) │        │ (local) │                    │
│   └────┬────┘        └────┬────┘        └────┬────┘                    │
│        │                  │                  │                          │
│        └──────────────────┴──────────────────┘                          │
│                    Chitchat Gossip                                       │
│                    (membership only)                                     │
│                                                                          │
│   Problems:                                                             │
│   - No leader election                                                  │
│   - State divergence possible                                           │
│   - Lost on restart                                                     │
│   - Split-brain undetected                                              │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

### Target State (Raft + Gossip)

```
┌─────────────────────────────────────────────────────────────────────────┐
│                      TARGET: RAFT + GOSSIP CLUSTER                       │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│   Node A (Leader)     Node B (Follower)   Node C (Follower)            │
│   ┌─────────────┐    ┌─────────────┐    ┌─────────────┐               │
│   │ Raft State  │    │ Raft State  │    │ Raft State  │               │
│   │ Machine     │    │ Machine     │    │ Machine     │               │
│   ├─────────────┤    ├─────────────┤    ├─────────────┤               │
│   │ Log Store   │    │ Log Store   │    │ Log Store   │               │
│   │ (RocksDB)   │    │ (RocksDB)   │    │ (RocksDB)   │               │
│   └──────┬──────┘    └──────┬──────┘    └──────┬──────┘               │
│          │                  │                  │                        │
│          ├──────────────────┴──────────────────┤                        │
│          │          Raft RPC (gRPC)            │                        │
│          │     (AppendEntries, Vote, etc)      │                        │
│          └─────────────────────────────────────┘                        │
│                              │                                          │
│                    Chitchat Gossip                                       │
│                    (discovery, health)                                   │
│                                                                          │
│   Benefits:                                                             │
│   - Automatic leader election                                           │
│   - Consistent state replication                                        │
│   - Survives node failures                                              │
│   - Persisted to disk                                                   │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## Architecture

### Component Layers

```
┌─────────────────────────────────────────────────────────────────────────┐
│                         RAFT INTEGRATION                                 │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│  ┌─────────────────────────────────────────────────────────────────┐    │
│  │                    Application Layer                             │    │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────────────┐  │    │
│  │  │ REST API     │  │ MCP Server   │  │ Reconciler           │  │    │
│  │  └──────┬───────┘  └──────┬───────┘  └──────────┬───────────┘  │    │
│  │         │                 │                      │              │    │
│  │         └─────────────────┴──────────────────────┘              │    │
│  │                           │                                      │    │
│  │                           ▼                                      │    │
│  │  ┌─────────────────────────────────────────────────────────┐    │    │
│  │  │                  RaftStateStore                          │    │    │
│  │  │         (implements StateStore trait)                    │    │    │
│  │  │                                                          │    │    │
│  │  │   write() ──► propose_to_raft() ──► commit() ──► apply() │    │    │
│  │  │   read()  ──► read_from_state_machine()                  │    │    │
│  │  └─────────────────────────┬───────────────────────────────┘    │    │
│  └────────────────────────────┼────────────────────────────────────┘    │
│                               │                                          │
│  ┌────────────────────────────┼────────────────────────────────────┐    │
│  │                    OpenRaft Layer                                │    │
│  │                            │                                     │    │
│  │  ┌─────────────────────────▼───────────────────────────────┐    │    │
│  │  │                    Raft<TypeConfig>                      │    │    │
│  │  │                                                          │    │    │
│  │  │   ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  │    │    │
│  │  │   │ LogStore     │  │ StateMachine │  │ Network      │  │    │    │
│  │  │   │ (RocksDB)    │  │ (in-memory)  │  │ (gRPC)       │  │    │    │
│  │  │   └──────────────┘  └──────────────┘  └──────────────┘  │    │    │
│  │  └─────────────────────────────────────────────────────────┘    │    │
│  └──────────────────────────────────────────────────────────────────┘    │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## Data Model

### What Gets Replicated

```rust
/// Commands that modify cluster state
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum ClusterCommand {
    // Workload operations
    CreateWorkload(WorkloadSpec),
    UpdateWorkload { id: WorkloadId, spec: WorkloadSpec },
    DeleteWorkload(WorkloadId),
    ScaleWorkload { id: WorkloadId, replicas: u32 },
    
    // Instance operations
    CreateInstance(InstanceSpec),
    UpdateInstanceStatus { id: InstanceId, status: InstanceStatus },
    DeleteInstance(InstanceId),
    
    // Node operations
    RegisterNode(NodeInfo),
    UpdateNodeStatus { id: NodeId, status: NodeStatus },
    DeregisterNode(NodeId),
    
    // Credit operations (Phase 3)
    TransferCredits { from: PeerId, to: PeerId, amount: u64 },
    MintCredits { to: PeerId, amount: u64, reason: String },
}

/// Response from state machine after applying command
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum ClusterResponse {
    Success,
    WorkloadCreated(WorkloadId),
    InstanceCreated(InstanceId),
    Error(String),
}
```

### State Machine

```rust
/// The replicated state machine
pub struct ClusterStateMachine {
    /// All workloads
    workloads: HashMap<WorkloadId, Workload>,
    
    /// All instances
    instances: HashMap<InstanceId, Instance>,
    
    /// All nodes
    nodes: HashMap<NodeId, NodeInfo>,
    
    /// Credit balances (Phase 3)
    credits: HashMap<PeerId, u64>,
    
    /// Last applied log index
    last_applied: LogId,
}

impl ClusterStateMachine {
    pub fn apply(&mut self, command: ClusterCommand) -> ClusterResponse {
        match command {
            ClusterCommand::CreateWorkload(spec) => {
                let id = WorkloadId::new();
                let workload = Workload::from_spec(id.clone(), spec);
                self.workloads.insert(id.clone(), workload);
                ClusterResponse::WorkloadCreated(id)
            }
            ClusterCommand::DeleteWorkload(id) => {
                self.workloads.remove(&id);
                // Also remove associated instances
                self.instances.retain(|_, i| i.workload_id != id);
                ClusterResponse::Success
            }
            // ... other commands
        }
    }
}
```

---

## OpenRaft Integration

### Type Configuration

```rust
// consensus/src/types.rs

use openraft::{Config, Raft, BasicNode};

/// Our Raft type configuration
#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Ord, PartialOrd)]
pub struct TypeConfig;

impl openraft::RaftTypeConfig for TypeConfig {
    type D = ClusterCommand;      // Log entry data
    type R = ClusterResponse;     // Response type
    type NodeId = NodeId;         // Node identifier
    type Node = BasicNode;        // Node metadata
    type Entry = openraft::Entry<TypeConfig>;
    type SnapshotData = Cursor<Vec<u8>>;
    type AsyncRuntime = openraft::TokioRuntime;
}

pub type ClusterRaft = Raft<TypeConfig>;
```

### Log Store (RocksDB)

```rust
// consensus/src/log_store.rs

use rocksdb::{DB, Options};
use openraft::storage::{LogState, RaftLogStorage};

pub struct RocksLogStore {
    db: DB,
}

impl RocksLogStore {
    pub fn new(path: &Path) -> Result<Self, StorageError> {
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.set_write_buffer_size(64 * 1024 * 1024); // 64MB
        
        let db = DB::open(&opts, path)?;
        Ok(Self { db })
    }
}

impl RaftLogStorage<TypeConfig> for RocksLogStore {
    async fn get_log_state(&mut self) -> Result<LogState<TypeConfig>, StorageError> {
        // Read from RocksDB
    }
    
    async fn save_vote(&mut self, vote: &Vote<NodeId>) -> Result<(), StorageError> {
        let key = b"vote";
        let value = serde_json::to_vec(vote)?;
        self.db.put(key, value)?;
        Ok(())
    }
    
    async fn read_vote(&mut self) -> Result<Option<Vote<NodeId>>, StorageError> {
        match self.db.get(b"vote")? {
            Some(bytes) => Ok(Some(serde_json::from_slice(&bytes)?)),
            None => Ok(None),
        }
    }
    
    async fn append<I>(&mut self, entries: I, callback: LogFlushed<TypeConfig>) 
    -> Result<(), StorageError>
    where I: IntoIterator<Item = Entry<TypeConfig>> {
        let mut batch = rocksdb::WriteBatch::default();
        
        for entry in entries {
            let key = format!("log:{}", entry.log_id.index);
            let value = serde_json::to_vec(&entry)?;
            batch.put(key.as_bytes(), value);
        }
        
        self.db.write(batch)?;
        callback.log_io_completed(Ok(()));
        Ok(())
    }
    
    // ... other methods
}
```

### Network Layer (gRPC)

```rust
// consensus/src/network.rs

use openraft::network::{RaftNetwork, RaftNetworkFactory};
use tonic::{transport::Channel, Request};

pub struct GrpcNetwork {
    connections: RwLock<HashMap<NodeId, RaftServiceClient<Channel>>>,
}

impl RaftNetworkFactory<TypeConfig> for GrpcNetwork {
    type Network = GrpcNetworkConnection;
    
    async fn new_client(&mut self, target: NodeId, node: &BasicNode) -> Self::Network {
        let addr = node.addr.clone();
        let channel = Channel::from_shared(format!("http://{}", addr))
            .unwrap()
            .connect()
            .await
            .unwrap();
            
        GrpcNetworkConnection {
            target,
            client: RaftServiceClient::new(channel),
        }
    }
}

pub struct GrpcNetworkConnection {
    target: NodeId,
    client: RaftServiceClient<Channel>,
}

impl RaftNetwork<TypeConfig> for GrpcNetworkConnection {
    async fn append_entries(
        &mut self,
        req: AppendEntriesRequest<TypeConfig>,
    ) -> Result<AppendEntriesResponse<NodeId>, RPCError<NodeId>> {
        let request = Request::new(req.into());
        let response = self.client.append_entries(request).await?;
        Ok(response.into_inner().into())
    }
    
    async fn vote(
        &mut self,
        req: VoteRequest<NodeId>,
    ) -> Result<VoteResponse<NodeId>, RPCError<NodeId>> {
        let request = Request::new(req.into());
        let response = self.client.vote(request).await?;
        Ok(response.into_inner().into())
    }
    
    async fn install_snapshot(
        &mut self,
        req: InstallSnapshotRequest<TypeConfig>,
    ) -> Result<InstallSnapshotResponse<NodeId>, RPCError<NodeId>> {
        // ... snapshot installation
    }
}
```

### gRPC Service Definition

```protobuf
// proto/raft.proto

syntax = "proto3";
package univrs.raft;

service RaftService {
    rpc AppendEntries(AppendEntriesRequest) returns (AppendEntriesResponse);
    rpc Vote(VoteRequest) returns (VoteResponse);
    rpc InstallSnapshot(stream InstallSnapshotRequest) returns (InstallSnapshotResponse);
}

message AppendEntriesRequest {
    uint64 term = 1;
    uint64 leader_id = 2;
    uint64 prev_log_index = 3;
    uint64 prev_log_term = 4;
    repeated LogEntry entries = 5;
    uint64 leader_commit = 6;
}

message AppendEntriesResponse {
    uint64 term = 1;
    bool success = 2;
    uint64 conflict_index = 3;
}

// ... other messages
```

---

## Integration with Existing Code

### RaftStateStore (Replaces InMemoryStateStore)

```rust
// consensus/src/state_store.rs

use state_store_interface::{StateStore, StateStoreError};

pub struct RaftStateStore {
    raft: Arc<ClusterRaft>,
    state_machine: Arc<RwLock<ClusterStateMachine>>,
}

#[async_trait]
impl StateStore for RaftStateStore {
    async fn create_workload(&self, spec: WorkloadSpec) -> Result<Workload, StateStoreError> {
        // Only leader can accept writes
        if !self.is_leader().await {
            return Err(StateStoreError::NotLeader(self.get_leader().await));
        }
        
        // Propose to Raft
        let command = ClusterCommand::CreateWorkload(spec);
        let response = self.raft.client_write(command).await
            .map_err(|e| StateStoreError::RaftError(e.to_string()))?;
        
        match response.data {
            ClusterResponse::WorkloadCreated(id) => {
                let state = self.state_machine.read().await;
                Ok(state.workloads.get(&id).unwrap().clone())
            }
            ClusterResponse::Error(e) => Err(StateStoreError::Internal(e)),
            _ => Err(StateStoreError::Internal("Unexpected response".into())),
        }
    }
    
    async fn get_workload(&self, id: &WorkloadId) -> Result<Option<Workload>, StateStoreError> {
        // Reads can go to any node (eventually consistent)
        // For linearizable reads, use read_linearizable()
        let state = self.state_machine.read().await;
        Ok(state.workloads.get(id).cloned())
    }
    
    async fn list_workloads(&self) -> Result<Vec<Workload>, StateStoreError> {
        let state = self.state_machine.read().await;
        Ok(state.workloads.values().cloned().collect())
    }
    
    // ... other methods follow same pattern
}
```

### Leader Forwarding

```rust
impl RaftStateStore {
    async fn forward_to_leader<T>(&self, request: T) -> Result<Response, StateStoreError> {
        let leader = self.get_leader().await
            .ok_or(StateStoreError::NoLeader)?;
        
        // Forward via gRPC
        let mut client = self.get_client(&leader).await?;
        client.forward(request).await
    }
    
    async fn is_leader(&self) -> bool {
        self.raft.current_leader().await == Some(self.node_id)
    }
    
    async fn get_leader(&self) -> Option<NodeId> {
        self.raft.current_leader().await
    }
}
```

---

## Cluster Lifecycle

### Bootstrap (First Node)

```rust
pub async fn bootstrap_cluster(raft: &ClusterRaft, node_id: NodeId) -> Result<(), RaftError> {
    // Initialize as single-node cluster
    let members = btreemap! {
        node_id => BasicNode { addr: "localhost:7000".into() }
    };
    
    raft.initialize(members).await?;
    Ok(())
}
```

### Join Existing Cluster

```rust
pub async fn join_cluster(
    raft: &ClusterRaft, 
    node_id: NodeId,
    bootstrap_addr: &str,
) -> Result<(), RaftError> {
    // Connect to existing leader
    let mut client = RaftServiceClient::connect(format!("http://{}", bootstrap_addr)).await?;
    
    // Request to join
    let response = client.add_learner(AddLearnerRequest {
        node_id: node_id.into(),
        node: BasicNode { addr: our_addr.into() }.into(),
    }).await?;
    
    // Wait for catch-up, then promote to voter
    // ...
    
    Ok(())
}
```

### Node Removal

```rust
pub async fn leave_cluster(raft: &ClusterRaft, node_id: NodeId) -> Result<(), RaftError> {
    raft.change_membership(
        ChangeMembers::Remove(btreeset! { node_id }),
        false, // don't turn off replication
    ).await?;
    
    Ok(())
}
```

---

## Configuration

```toml
# Node configuration

[raft]
# Raft cluster configuration
node_id = "node-1"
data_dir = "/var/lib/univrs/raft"

# Timeouts
heartbeat_interval_ms = 150
election_timeout_min_ms = 300
election_timeout_max_ms = 500

# Snapshotting
snapshot_threshold = 10000  # Entries before snapshot
snapshot_max_chunk_size = 1048576  # 1MB

# Network
raft_port = 7000
max_payload_size = 4194304  # 4MB

[raft.bootstrap]
# Bootstrap nodes for cluster formation
nodes = [
    { id = "node-1", addr = "192.168.1.10:7000" },
    { id = "node-2", addr = "192.168.1.11:7000" },
    { id = "node-3", addr = "192.168.1.12:7000" },
]
```

---

## Implementation Phases

### Phase 1: Foundation (Week 1)

- [ ] Add OpenRaft dependency
- [ ] Define TypeConfig and command types
- [ ] Implement RocksDB LogStore
- [ ] Basic state machine (workloads only)

### Phase 2: Networking (Week 2)

- [ ] gRPC service definition
- [ ] Network layer implementation
- [ ] Leader election testing
- [ ] Log replication testing

### Phase 3: Integration (Week 2-3)

- [ ] RaftStateStore implementing StateStore trait
- [ ] Leader forwarding
- [ ] Cluster bootstrap/join logic
- [ ] Replace InMemoryStateStore

### Phase 4: Production (Week 3-4)

- [ ] Snapshotting
- [ ] Membership changes
- [ ] Metrics and monitoring
- [ ] Chaos testing

---

## Testing Strategy

### Unit Tests

```rust
#[tokio::test]
async fn test_state_machine_apply() {
    let mut sm = ClusterStateMachine::default();
    
    let response = sm.apply(ClusterCommand::CreateWorkload(WorkloadSpec {
        name: "test".into(),
        image: "nginx".into(),
        replicas: 3,
    }));
    
    assert!(matches!(response, ClusterResponse::WorkloadCreated(_)));
    assert_eq!(sm.workloads.len(), 1);
}
```

### Integration Tests

```rust
#[tokio::test]
async fn test_three_node_cluster() {
    // Start 3 nodes
    let nodes = start_cluster(3).await;
    
    // Wait for leader election
    tokio::time::sleep(Duration::from_secs(2)).await;
    
    // Find leader
    let leader = nodes.iter().find(|n| n.is_leader()).unwrap();
    
    // Write to leader
    leader.create_workload(spec).await.unwrap();
    
    // Read from follower (eventual consistency)
    tokio::time::sleep(Duration::from_millis(100)).await;
    let follower = nodes.iter().find(|n| !n.is_leader()).unwrap();
    let workload = follower.get_workload(&id).await.unwrap();
    assert!(workload.is_some());
}
```

### Chaos Tests

```rust
#[tokio::test]
async fn test_leader_failure() {
    let nodes = start_cluster(3).await;
    let leader = find_leader(&nodes).await;
    
    // Kill leader
    leader.shutdown().await;
    
    // Wait for new election
    tokio::time::sleep(Duration::from_secs(3)).await;
    
    // Verify new leader elected
    let remaining: Vec<_> = nodes.iter().filter(|n| n.is_running()).collect();
    let new_leader = find_leader(&remaining).await;
    assert!(new_leader.is_some());
    
    // Verify writes still work
    new_leader.create_workload(spec).await.unwrap();
}
```

---

## Dependencies

```toml
[dependencies]
openraft = { version = "0.9", features = ["serde"] }
rocksdb = "0.21"
tonic = "0.11"
prost = "0.12"

[build-dependencies]
tonic-build = "0.11"
```

---

## Risks & Mitigations

| Risk | Mitigation |
|------|------------|
| Split-brain | Require majority quorum (3 of 5, 2 of 3) |
| Slow elections | Tune timeouts for network conditions |
| Large snapshots | Incremental snapshotting, compression |
| Network partitions | Proper timeout configuration, monitoring |
| Data corruption | Checksums, replication provides redundancy |

---

## Success Criteria

1. 3-node cluster elects leader within 5 seconds
2. Writes replicate to all nodes within 100ms
3. Cluster survives single node failure
4. State persists across full cluster restart
5. New nodes can join running cluster
6. Linearizable reads available on leader

---

## References

- [OpenRaft Documentation](https://datafuselabs.github.io/openraft/)
- [OpenRaft Examples](https://github.com/datafuselabs/openraft/tree/main/examples)
- [Raft Paper](https://raft.github.io/raft.pdf)
- [RocksDB Documentation](https://rocksdb.org/docs/)

---

*Consensus Design v0.1.0*  
*Distributed agreement for reliable orchestration*
