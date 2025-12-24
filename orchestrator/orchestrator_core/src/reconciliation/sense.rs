//! Reconciliation Sense Module
//!
//! This module implements the **sense** phase of the reconciliation loop:
//! `sense -> compare -> plan -> actuate`
//!
//! The sense phase is responsible for observing the current state of the system
//! in a read-only, atomic manner. It provides mechanisms to query various entities
//! (containers, nodes, workloads, instances) and report their current state with
//! fidelity guarantees.
//!
//! # Reconciliation Loop Phases
//!
//! 1. **Sense** (this module): Observe current state of the system
//! 2. **Compare**: Diff current state against desired state
//! 3. **Plan**: Generate actions to reconcile differences
//! 4. **Actuate**: Execute planned actions to reach desired state
//!
//! # Key Concepts
//!
//! - **Atomicity**: All sense operations are atomic and read-only
//! - **Scope**: Define which entity types to observe
//! - **Fidelity**: Track accuracy and latency of observations
//! - **Completeness**: Report whether full or partial state was captured

use std::time::Instant;
use uuid::Uuid;

/// Represents the completeness of a state observation.
///
/// Indicates whether the sense operation successfully captured the full state
/// or only a partial view due to failures or constraints.
#[derive(Debug, Clone, PartialEq)]
pub enum Completeness {
    /// Full state successfully captured for all requested entity types.
    Full,

    /// Partial state captured; some entity types are missing.
    Partial {
        /// List of entity types that could not be observed.
        missing: Vec<EntityType>,
    },
}

/// Enumeration of entity types that can be observed in the system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EntityType {
    /// Container entity type.
    Container,

    /// Node entity type.
    Node,

    /// Workload entity type.
    Workload,

    /// Instance entity type.
    Instance,
}

/// Represents the current observed state of a container.
///
/// Containers are the fundamental runtime units in the system.
#[derive(Debug, Clone, PartialEq)]
pub struct ContainerState {
    /// Unique identifier for the container.
    pub id: String,

    /// Current status of the container (e.g., running, stopped, paused).
    pub status: String,

    /// Resource utilization metrics.
    pub resources: ResourceMetrics,

    /// Timestamp when this state was observed.
    pub observed_at: Instant,
}

/// Represents the current observed state of a node.
///
/// Nodes are the compute resources that host containers and workloads.
#[derive(Debug, Clone, PartialEq)]
pub struct NodeState {
    /// Unique identifier for the node.
    pub id: String,

    /// Node health status (e.g., healthy, degraded, unreachable).
    pub health: String,

    /// Available capacity on the node.
    pub capacity: ResourceMetrics,

    /// Timestamp when this state was observed.
    pub observed_at: Instant,
}

/// Represents the current observed state of a workload.
///
/// Workloads are logical groupings of work to be executed.
#[derive(Debug, Clone, PartialEq)]
pub struct WorkloadState {
    /// Unique identifier for the workload.
    pub id: String,

    /// Current phase of the workload lifecycle.
    pub phase: String,

    /// Number of desired replicas.
    pub desired_replicas: u32,

    /// Number of ready replicas.
    pub ready_replicas: u32,

    /// Timestamp when this state was observed.
    pub observed_at: Instant,
}

/// Represents the current observed state of an instance.
///
/// Instances are individual execution units of a workload.
#[derive(Debug, Clone, PartialEq)]
pub struct InstanceState {
    /// Unique identifier for the instance.
    pub id: String,

    /// Associated workload identifier.
    pub workload_id: String,

    /// Current status of the instance.
    pub status: String,

    /// Node where this instance is running.
    pub node_id: Option<String>,

    /// Timestamp when this state was observed.
    pub observed_at: Instant,
}

/// Resource metrics for capacity and utilization tracking.
#[derive(Debug, Clone, PartialEq)]
pub struct ResourceMetrics {
    /// CPU allocation/usage (in millicores).
    pub cpu_millicores: u64,

    /// Memory allocation/usage (in bytes).
    pub memory_bytes: u64,

    /// Storage allocation/usage (in bytes).
    pub storage_bytes: u64,
}

/// Complete snapshot of the current system state.
///
/// Represents the observed state of all requested entities at a specific point in time.
/// The completeness field indicates whether this is a full or partial view.
#[derive(Debug, Clone, PartialEq)]
pub struct CurrentState {
    /// Timestamp when this state snapshot was taken.
    pub timestamp: Instant,

    /// Indicates whether the state observation is complete or partial.
    pub completeness: Completeness,

    /// Observed container states.
    pub containers: Vec<ContainerState>,

    /// Observed node states.
    pub nodes: Vec<NodeState>,

    /// Observed workload states.
    pub workloads: Vec<WorkloadState>,

    /// Observed instance states.
    pub instances: Vec<InstanceState>,
}

/// Defines the scope of entities to observe during a sense operation.
///
/// Allows selective observation of entity types to optimize performance
/// and reduce overhead when only specific entity types are needed.
#[derive(Debug, Clone, PartialEq)]
pub struct SenseScope {
    /// Whether to observe containers.
    pub containers: bool,

    /// Whether to observe nodes.
    pub nodes: bool,

    /// Whether to observe workloads.
    pub workloads: bool,

    /// Whether to observe instances.
    pub instances: bool,
}

impl SenseScope {
    /// Creates a scope that observes all entity types.
    pub fn all() -> Self {
        Self {
            containers: true,
            nodes: true,
            workloads: true,
            instances: true,
        }
    }

    /// Creates a scope that observes no entity types.
    pub fn none() -> Self {
        Self {
            containers: false,
            nodes: false,
            workloads: false,
            instances: false,
        }
    }

    /// Creates a scope that observes only containers.
    pub fn containers_only() -> Self {
        Self {
            containers: true,
            nodes: false,
            workloads: false,
            instances: false,
        }
    }

    /// Creates a scope that observes only nodes.
    pub fn nodes_only() -> Self {
        Self {
            containers: false,
            nodes: true,
            workloads: false,
            instances: false,
        }
    }

    /// Creates a scope that observes only workloads.
    pub fn workloads_only() -> Self {
        Self {
            containers: false,
            nodes: false,
            workloads: true,
            instances: false,
        }
    }

    /// Creates a scope that observes only instances.
    pub fn instances_only() -> Self {
        Self {
            containers: false,
            nodes: false,
            workloads: false,
            instances: true,
        }
    }
}

/// Tracks the fidelity (accuracy and performance) of a sense operation.
///
/// Fidelity metrics help evaluate the quality of state observations and can be
/// used to detect degraded sensing capabilities.
#[derive(Debug, Clone, PartialEq)]
pub struct SenseFidelity {
    /// Accuracy of the observation (0.0 = completely inaccurate, 1.0 = perfect accuracy).
    ///
    /// Accuracy may be reduced due to stale data, sampling, or estimation techniques.
    pub accuracy: f64,

    /// Latency of the sense operation in milliseconds.
    ///
    /// Measures the time taken to complete the observation.
    pub latency_ms: u64,
}

impl SenseFidelity {
    /// Creates a new SenseFidelity with the given accuracy and latency.
    ///
    /// # Panics
    ///
    /// Panics if accuracy is not in the range [0.0, 1.0].
    pub fn new(accuracy: f64, latency_ms: u64) -> Self {
        assert!(
            (0.0..=1.0).contains(&accuracy),
            "accuracy must be between 0.0 and 1.0, got: {}",
            accuracy
        );
        Self {
            accuracy,
            latency_ms,
        }
    }
}

/// A completed sense operation with correlation tracking.
///
/// Encapsulates the result of a sense operation along with metadata for
/// tracing and debugging.
#[derive(Debug, Clone, PartialEq)]
pub struct SenseOperation {
    /// Unique correlation identifier for this operation.
    ///
    /// Used for distributed tracing and correlating sense operations with
    /// subsequent reconciliation phases.
    pub correlation_id: Uuid,

    /// The scope that was requested for this operation.
    pub scope: SenseScope,

    /// The observed current state.
    pub result: CurrentState,

    /// Fidelity metrics for this operation.
    pub fidelity: SenseFidelity,
}

/// Errors that can occur during sense operations.
#[derive(Debug, Clone, PartialEq)]
pub enum SenseError {
    /// The underlying query to observe state failed.
    QueryFailed(String),

    /// Partial state was read; some entity types failed to be observed.
    PartialRead {
        /// Entity types that were successfully observed.
        observed: Vec<EntityType>,

        /// Entity types that failed to be observed.
        failed: Vec<EntityType>,
    },

    /// The sense operation timed out before completing.
    Timeout,
}

impl std::fmt::Display for SenseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SenseError::QueryFailed(msg) => write!(f, "Query failed: {}", msg),
            SenseError::PartialRead { observed, failed } => {
                write!(
                    f,
                    "Partial read: observed {:?}, failed {:?}",
                    observed, failed
                )
            }
            SenseError::Timeout => write!(f, "Sense operation timed out"),
        }
    }
}

impl std::error::Error for SenseError {}

/// Trait for implementing sensors that observe system state.
///
/// Sensors are responsible for querying the current state of the system
/// in an atomic, read-only manner. Implementations must guarantee:
///
/// - **Atomicity**: Each sense operation provides a consistent snapshot
/// - **Read-only**: No mutations to system state during observation
/// - **Bounded latency**: Operations should complete within reasonable time
///
/// # Examples
///
/// ```ignore
/// use orchestrator_core::reconciliation::sense::{Sensor, SenseScope, SenseOperation, SenseError};
///
/// struct KubernetesSensor {
///     // ... k8s client
/// }
///
/// #[async_trait::async_trait]
/// impl Sensor for KubernetesSensor {
///     async fn sense(&self, scope: &SenseScope) -> Result<SenseOperation, SenseError> {
///         // Query Kubernetes API
///         // Build CurrentState from API responses
///         // Return SenseOperation
///         todo!()
///     }
/// }
/// ```
#[async_trait::async_trait]
pub trait Sensor: Send + Sync {
    /// Observes the current state of the system within the given scope.
    ///
    /// This operation must be:
    /// - **Atomic**: Provide a consistent snapshot at a point in time
    /// - **Read-only**: Must not modify any system state
    ///
    /// # Arguments
    ///
    /// * `scope` - Defines which entity types to observe
    ///
    /// # Returns
    ///
    /// Returns a `SenseOperation` containing the observed state and metadata,
    /// or a `SenseError` if the operation failed.
    ///
    /// # Errors
    ///
    /// - `SenseError::QueryFailed`: Underlying queries failed completely
    /// - `SenseError::PartialRead`: Some entities observed, others failed
    /// - `SenseError::Timeout`: Operation exceeded timeout threshold
    async fn sense(&self, scope: &SenseScope) -> Result<SenseOperation, SenseError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sense_scope_all() {
        let scope = SenseScope::all();
        assert!(scope.containers);
        assert!(scope.nodes);
        assert!(scope.workloads);
        assert!(scope.instances);
    }

    #[test]
    fn test_sense_scope_none() {
        let scope = SenseScope::none();
        assert!(!scope.containers);
        assert!(!scope.nodes);
        assert!(!scope.workloads);
        assert!(!scope.instances);
    }

    #[test]
    fn test_sense_fidelity_valid() {
        let fidelity = SenseFidelity::new(0.95, 100);
        assert_eq!(fidelity.accuracy, 0.95);
        assert_eq!(fidelity.latency_ms, 100);
    }

    #[test]
    #[should_panic(expected = "accuracy must be between 0.0 and 1.0")]
    fn test_sense_fidelity_invalid_accuracy() {
        SenseFidelity::new(1.5, 100);
    }

    #[test]
    fn test_completeness_full() {
        let completeness = Completeness::Full;
        assert_eq!(completeness, Completeness::Full);
    }

    #[test]
    fn test_completeness_partial() {
        let completeness = Completeness::Partial {
            missing: vec![EntityType::Container, EntityType::Node],
        };
        match completeness {
            Completeness::Partial { missing } => {
                assert_eq!(missing.len(), 2);
                assert!(missing.contains(&EntityType::Container));
                assert!(missing.contains(&EntityType::Node));
            }
            _ => panic!("Expected Completeness::Partial"),
        }
    }

    #[test]
    fn test_sense_error_display() {
        let err = SenseError::QueryFailed("connection refused".to_string());
        assert_eq!(err.to_string(), "Query failed: connection refused");

        let err = SenseError::Timeout;
        assert_eq!(err.to_string(), "Sense operation timed out");

        let err = SenseError::PartialRead {
            observed: vec![EntityType::Container],
            failed: vec![EntityType::Node],
        };
        assert!(err.to_string().contains("Partial read"));
    }
}
