use async_trait::async_trait;
use orchestrator_shared_types::{ContainerId, NodeId, OrchestrationError};
use serde::{Deserialize, Serialize};
use std::time::SystemTime;
use uuid::Uuid;

/// Network resource allocation details.
///
/// Represents the network bandwidth, ports, and other networking resources
/// allocated to a container during the binding process.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NetworkAllocation {
    /// Allocated network bandwidth in Mbps
    pub bandwidth_mbps: u64,
    /// Reserved port ranges for the container
    pub port_range: Option<(u16, u16)>,
    /// Network namespace identifier
    pub network_namespace: Option<String>,
}

/// Resource commitment details for binding a container to a node.
///
/// Represents the permanent allocation of resources that will be committed
/// when a container is bound to a node. This converts temporary reservations
/// from the selection phase into permanent allocations.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResourceCommitment {
    /// CPU allocation in millicores (1000 = 1 core)
    pub cpu_allocation: u64,
    /// Memory allocation in bytes
    pub memory_allocation: u64,
    /// Storage allocation in bytes
    pub storage_allocation: u64,
    /// Optional network resource allocation
    pub network_allocation: Option<NetworkAllocation>,
}

/// Binding mode strategy for committing container placement.
///
/// The binding mode determines how the scheduler commits the placement decision,
/// trading off between latency, consistency guarantees, and fault tolerance.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum BindingMode {
    /// Optimistic binding strategy.
    ///
    /// Assumes the reservation is still valid and commits immediately with minimal
    /// validation. Provides the fastest binding path but includes conflict detection
    /// to handle race conditions. On conflict, triggers rollback and re-selection.
    ///
    /// Best for: Stable clusters with low contention and where fast container
    /// startup is critical.
    Optimistic {
        /// Whether to use fast commit path (skip extended validation)
        fast_commit: bool,
        /// Whether to perform conflict detection after commit
        conflict_detection: bool,
    },

    /// Pessimistic binding strategy.
    ///
    /// Acquires exclusive locks before committing and verifies all preconditions.
    /// Higher latency but provides guaranteed consistency with no rollback needed
    /// on successful lock acquisition.
    ///
    /// Best for: High-contention scenarios or critical workloads requiring
    /// strict consistency guarantees.
    Pessimistic {
        /// Whether to acquire locks before validation
        lock_acquisition: bool,
        /// Whether to verify all preconditions before commit
        verified_commit: bool,
    },

    /// Two-phase binding strategy.
    ///
    /// Implements a two-phase commit protocol with separate prepare and commit phases.
    /// Allows for distributed coordination, partial rollback, and is used in
    /// multi-cluster or federated scheduling scenarios.
    ///
    /// Best for: Distributed systems, multi-cluster scheduling, or scenarios
    /// requiring coordinated resource allocation across multiple nodes.
    TwoPhase {
        /// Prepare phase: validate and lock without committing
        prepare_phase: bool,
        /// Commit phase: finalize the binding atomically
        commit_phase: bool,
    },
}

impl Default for BindingMode {
    fn default() -> Self {
        // Default to optimistic binding for best performance in common case
        BindingMode::Optimistic {
            fast_commit: true,
            conflict_detection: true,
        }
    }
}

/// Request to bind a container to a target node.
///
/// Represents the complete binding operation, including container identity,
/// target placement, resource commitments, and operational parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BindRequest {
    /// Container to be bound
    pub container: ContainerId,
    /// Target node for placement
    pub target_node: NodeId,
    /// Reservation ID from the selection phase
    pub reservation_id: Uuid,
    /// Binding strategy to use
    pub binding_mode: BindingMode,
    /// Resources to commit on the target node
    pub resource_updates: ResourceCommitment,
}

/// Container state after binding operation.
///
/// Represents the state of a container in the scheduling pipeline,
/// tracking its progression from pending to bound or failed.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ContainerState {
    /// Container is waiting to be scheduled
    Pending,
    /// Container has been scheduled but not yet bound
    Scheduled,
    /// Container has been successfully bound to a node
    Bound,
    /// Container binding failed
    Failed,
    /// Container is in the process of being removed
    Terminating,
}

/// Result of a binding operation.
///
/// Contains the outcome of the binding attempt, including success status,
/// updated states, and emitted events for observability.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BindResult {
    /// Whether the binding operation succeeded
    pub success: bool,
    /// Updated container state after binding
    pub container_state: ContainerState,
    /// Whether the node state was successfully updated
    pub node_state_updated: bool,
    /// Event emitted for this binding operation
    pub event: BindEvent,
}

/// Events emitted during binding operations.
///
/// These events provide observability into the binding process and are used
/// for monitoring, auditing, and triggering downstream workflows.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BindEvent {
    /// Binding succeeded.
    ///
    /// Emitted when a container is successfully bound to a node with all
    /// resource commitments applied and state transitions completed.
    BindSuccess {
        /// Container that was bound
        container_id: ContainerId,
        /// Node where the container was bound
        node_id: NodeId,
        /// Timestamp of successful binding
        timestamp: SystemTime,
    },

    /// Binding failed.
    ///
    /// Emitted when binding fails for any reason, including reservation expiry,
    /// resource unavailability, network partitions, or constraint violations.
    BindFailure {
        /// Container that failed to bind
        container_id: ContainerId,
        /// Reason for binding failure
        reason: String,
        /// Timestamp of failure
        timestamp: SystemTime,
    },
}

/// Handler for rolling back failed binding operations.
///
/// Implements compensation logic to ensure system consistency when binding fails.
/// Rollback operations must be idempotent and fast to minimize inconsistency windows.
///
/// The rollback handler is responsible for:
/// - Releasing reservations to free locked resources
/// - Reverting any partial state changes
/// - Cleaning up allocated resources
/// - Potentially triggering re-scheduling or alerts
#[async_trait]
pub trait RollbackHandler: Send + Sync {
    /// Rollback a failed binding operation.
    ///
    /// This method is called when a binding operation fails and needs to be undone.
    /// Implementations must ensure that all resources are properly released and
    /// any partial state changes are reverted.
    ///
    /// # Arguments
    ///
    /// * `request` - The original bind request that failed
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if rollback completed successfully, or a `RollbackError`
    /// if the rollback itself failed (which may require manual intervention).
    ///
    /// # Implementation Notes
    ///
    /// - Rollback operations must be idempotent (safe to call multiple times)
    /// - Should complete quickly to minimize resource lock time
    /// - Must handle partial rollback scenarios gracefully
    /// - Should log detailed error information for debugging
    async fn rollback(&self, request: &BindRequest) -> std::result::Result<(), RollbackError>;
}

/// Trait for binding containers to nodes.
///
/// Implements the final stage of the scheduling pipeline, committing the placement
/// decision to the selected node and updating all relevant system state.
///
/// The binder is responsible for:
/// - Validating reservations from the selection phase
/// - Committing resources on the target node
/// - Updating container and node state atomically
/// - Emitting events for observability
/// - Coordinating rollback on failure
///
/// # Binding Process
///
/// 1. Validate the reservation is still valid and not expired
/// 2. Execute binding according to the specified binding mode
/// 3. Commit resource allocations on the target node
/// 4. Update container state (pending -> bound)
/// 5. Update node state (allocated resources, container registry)
/// 6. Emit success or failure events
/// 7. On failure, trigger rollback handler
///
/// # Error Handling
///
/// Binding can fail for several reasons:
/// - Reservation expired or invalid
/// - Node state changed since selection (resources unavailable)
/// - Network partition or node failure
/// - Resource constraint violation detected late
/// - Concurrent binding conflict (optimistic mode)
///
/// When binding fails, the implementation must:
/// - Return a BindResult with success=false
/// - Emit a BindFailure event with detailed reason
/// - Trigger the rollback handler to clean up
/// - Ensure no partial state remains in the system
#[async_trait]
pub trait Binder: Send + Sync {
    /// Bind a container to a node.
    ///
    /// Commits the placement decision by allocating resources, updating state,
    /// and emitting events. This is an atomic operation - either the binding
    /// fully succeeds or it is rolled back entirely.
    ///
    /// # Arguments
    ///
    /// * `request` - The bind request containing container, node, and resource details
    ///
    /// # Returns
    ///
    /// Returns a `BindResult` containing the outcome of the binding operation,
    /// including updated states and emitted events.
    ///
    /// # Errors
    ///
    /// Returns `BindError` if:
    /// - The reservation is invalid or expired
    /// - Resources are no longer available on the target node
    /// - Node state update fails
    /// - Network communication fails
    /// - Any other critical binding precondition is violated
    ///
    /// # Performance Considerations
    ///
    /// - Binding latency directly impacts container startup time
    /// - Optimistic mode minimizes latency for the common case
    /// - Event emission should be asynchronous to avoid blocking
    /// - Consider using connection pooling for state store access
    async fn bind(&self, request: BindRequest) -> std::result::Result<BindResult, BindError>;

    /// Validate that a reservation is still valid.
    ///
    /// Checks whether a reservation from the selection phase is still active
    /// and not expired. This is typically called before attempting to bind.
    ///
    /// # Arguments
    ///
    /// * `reservation_id` - The reservation UUID to validate
    ///
    /// # Returns
    ///
    /// Returns `true` if the reservation is valid and can be used for binding,
    /// `false` otherwise.
    fn validate_reservation(&self, reservation_id: Uuid) -> bool;
}

/// Errors that can occur during binding operations.
#[derive(Debug, thiserror::Error)]
pub enum BindError {
    /// Reservation is invalid or has expired
    #[error("Reservation {0} is invalid or expired")]
    InvalidReservation(Uuid),

    /// Target node is no longer available or has insufficient resources
    #[error("Target node {0} is unavailable or has insufficient resources: {1}")]
    NodeUnavailable(NodeId, String),

    /// Resource commitment failed
    #[error("Failed to commit resources on node {0}: {1}")]
    ResourceCommitmentFailed(NodeId, String),

    /// State update failed
    #[error("Failed to update system state: {0}")]
    StateUpdateFailed(String),

    /// Conflict detected during optimistic binding
    #[error("Binding conflict detected for container {0}: {1}")]
    BindingConflict(ContainerId, String),

    /// Lock acquisition failed (pessimistic mode)
    #[error("Failed to acquire lock for binding: {0}")]
    LockAcquisitionFailed(String),

    /// Two-phase commit failed
    #[error("Two-phase commit failed at {0} phase: {1}")]
    TwoPhaseCommitFailed(String, String),

    /// Network partition or communication failure
    #[error("Network communication failed: {0}")]
    NetworkError(String),

    /// General orchestration error
    #[error("Orchestration error: {0}")]
    OrchestrationError(#[from] OrchestrationError),

    /// Internal error
    #[error("Internal binding error: {0}")]
    InternalError(String),
}

/// Errors that can occur during rollback operations.
#[derive(Debug, thiserror::Error)]
pub enum RollbackError {
    /// Reservation release failed
    #[error("Failed to release reservation {0}: {1}")]
    ReservationReleaseFailed(Uuid, String),

    /// Resource cleanup failed
    #[error("Failed to clean up resources on node {0}: {1}")]
    ResourceCleanupFailed(NodeId, String),

    /// State reversion failed
    #[error("Failed to revert state changes: {0}")]
    StateReversionFailed(String),

    /// Rollback is incomplete and manual intervention may be required
    #[error("Rollback incomplete, manual intervention required: {0}")]
    IncompleteRollback(String),

    /// Internal error during rollback
    #[error("Internal rollback error: {0}")]
    InternalError(String),
}

// Convert BindError to OrchestrationError for compatibility
impl From<BindError> for OrchestrationError {
    fn from(err: BindError) -> Self {
        OrchestrationError::SchedulingError(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_binding_mode_default() {
        let mode = BindingMode::default();
        assert!(matches!(mode, BindingMode::Optimistic { .. }));
    }

    #[test]
    fn test_resource_commitment_creation() {
        let commitment = ResourceCommitment {
            cpu_allocation: 1000,
            memory_allocation: 1024 * 1024 * 512,   // 512 MB
            storage_allocation: 1024 * 1024 * 1024, // 1 GB
            network_allocation: Some(NetworkAllocation {
                bandwidth_mbps: 100,
                port_range: Some((8000, 8100)),
                network_namespace: Some("ns-1".to_string()),
            }),
        };

        assert_eq!(commitment.cpu_allocation, 1000);
        assert!(commitment.network_allocation.is_some());
    }

    fn generate_node_id() -> NodeId {
        orchestrator_shared_types::Keypair::generate().public_key()
    }

    #[test]
    fn test_bind_request_creation() {
        let request = BindRequest {
            container: "container-123".to_string(),
            target_node: generate_node_id(),
            reservation_id: Uuid::new_v4(),
            binding_mode: BindingMode::Optimistic {
                fast_commit: true,
                conflict_detection: true,
            },
            resource_updates: ResourceCommitment {
                cpu_allocation: 500,
                memory_allocation: 256 * 1024 * 1024,
                storage_allocation: 512 * 1024 * 1024,
                network_allocation: None,
            },
        };

        assert_eq!(request.container, "container-123");
        assert!(matches!(
            request.binding_mode,
            BindingMode::Optimistic { .. }
        ));
    }

    #[test]
    fn test_bind_event_success() {
        let event = BindEvent::BindSuccess {
            container_id: "container-456".to_string(),
            node_id: generate_node_id(),
            timestamp: SystemTime::now(),
        };

        assert!(matches!(event, BindEvent::BindSuccess { .. }));
    }

    #[test]
    fn test_bind_event_failure() {
        let event = BindEvent::BindFailure {
            container_id: "container-789".to_string(),
            reason: "Node unavailable".to_string(),
            timestamp: SystemTime::now(),
        };

        assert!(matches!(event, BindEvent::BindFailure { .. }));
    }

    #[test]
    fn test_container_state_transitions() {
        let state = ContainerState::Pending;
        assert_eq!(state, ContainerState::Pending);

        let state = ContainerState::Bound;
        assert_eq!(state, ContainerState::Bound);
    }
}
