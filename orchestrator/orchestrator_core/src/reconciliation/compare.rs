//! Reconciliation Compare Module
//!
//! This module implements the `reconciliation.compare` trait from the DOL specification.
//! It represents **Phase 2** of the reconciliation control loop:
//! **sense -> compare -> plan -> actuate**
//!
//! The compare phase determines the difference between desired state (what users declared)
//! and current state (what the system sensed), categorizing changes and quantifying drift.
//!
//! # DOL Specification
//!
//! From `docs/ontology/prospective/reconciliation/traits/compare.dol`:
//!
//! ```dol
//! trait reconciliation.compare {
//!   uses reconciliation.sense
//!
//!   compare has desired_state
//!   compare has current_state
//!   compare has diff
//!
//!   desired_state derives from workload_definitions
//!   desired_state is declarative
//!   desired_state is versioned
//!
//!   current_state derives from sense
//!
//!   diff has additions
//!   diff has modifications
//!   diff has deletions
//!   diff has unchanged
//!
//!   drift has magnitude
//!   drift has urgency
//!
//!   each comparison is deterministic
//!   each comparison is idempotent
//! }
//! ```
//!
//! # Key Invariants
//!
//! - **Deterministic**: Same inputs always produce same outputs
//! - **Idempotent**: Running comparison twice gives the same result
//! - **Complete**: All entities are categorized (add/modify/delete/unchanged)
//!
//! # Dependencies
//!
//! This module depends on the `reconciliation.sense` gene for current state observation.

use orchestrator_shared_types::{
    ContainerId, Node, NodeId, WorkloadDefinition, WorkloadId, WorkloadInstance,
};
use serde::{Deserialize, Serialize};
use serde_json;
use std::collections::HashMap;
use std::time::Instant;
use uuid::Uuid;

// Helper function for Instant default during deserialization
fn instant_now() -> Instant {
    Instant::now()
}

// ================================================================================================
// CURRENT STATE (from reconciliation.sense)
// ================================================================================================

/// Represents the current state of the cluster as observed by the sense gene.
///
/// This is the foundation for comparison - it captures what is **actually running**
/// in the cluster at a specific point in time.
///
/// # DOL Properties
///
/// From `reconciliation.sense`:
/// - `current_state derives from cluster_query`
/// - `current_state has timestamp`
/// - `current_state has completeness`
/// - `scope has containers, nodes, workloads, instances`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurrentState {
    /// All nodes currently in the cluster
    pub nodes: HashMap<NodeId, Node>,

    /// All workload definitions currently stored
    pub workloads: HashMap<WorkloadId, WorkloadDefinition>,

    /// All workload instances currently running or tracked
    pub instances: HashMap<Uuid, WorkloadInstance>,

    /// Container ID to Instance ID mapping for quick lookups
    pub containers: HashMap<ContainerId, Uuid>,

    /// When this state snapshot was captured
    #[serde(skip, default = "instant_now")]
    pub timestamp: Instant,

    /// Whether this is a complete snapshot (true) or partial read (false)
    pub completeness: bool,

    /// Correlation ID for tracing this observation through the reconciliation cycle
    pub correlation_id: Uuid,
}

impl CurrentState {
    /// Creates a new current state snapshot
    pub fn new(correlation_id: Uuid) -> Self {
        Self {
            nodes: HashMap::new(),
            workloads: HashMap::new(),
            instances: HashMap::new(),
            containers: HashMap::new(),
            timestamp: Instant::now(),
            completeness: true,
            correlation_id,
        }
    }

    /// Creates an empty current state for testing or initial bootstrap
    pub fn empty() -> Self {
        Self::new(Uuid::new_v4())
    }
}

// ================================================================================================
// DESIRED STATE
// ================================================================================================

/// Represents the desired state of the cluster as declared by users.
///
/// This is what **should be running** according to workload definitions.
///
/// # DOL Properties
///
/// From `reconciliation.compare`:
/// - `desired_state derives from workload_definitions`
/// - `desired_state is declarative`
/// - `desired_state is versioned`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesiredState {
    /// The workload definitions that declare what should be running
    pub workload_definitions: Vec<WorkloadDefinition>,

    /// Version number for this desired state (increments on changes)
    pub version: u64,

    /// When this desired state was established
    #[serde(skip, default = "instant_now")]
    pub timestamp: Instant,
}

impl DesiredState {
    /// Creates a new desired state from workload definitions
    pub fn new(workload_definitions: Vec<WorkloadDefinition>, version: u64) -> Self {
        Self {
            workload_definitions,
            version,
            timestamp: Instant::now(),
        }
    }

    /// Creates an empty desired state for testing
    pub fn empty() -> Self {
        Self::new(Vec::new(), 0)
    }
}

// ================================================================================================
// ENTITY IDENTIFICATION
// ================================================================================================

/// Unique identifier for any entity in the system
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EntityId {
    /// A container entity
    Container(ContainerId),

    /// A node entity
    Node(NodeId),

    /// A workload definition entity
    Workload(WorkloadId),

    /// A workload instance entity
    Instance(Uuid),
}

/// Type of entity being changed
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EntityType {
    /// Container entity type
    Container,

    /// Node entity type
    Node,

    /// Workload definition entity type
    Workload,

    /// Workload instance entity type
    Instance,
}

// ================================================================================================
// CHANGE TRACKING
// ================================================================================================

/// Type of change detected for an entity
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChangeType {
    /// Entity needs to be created (in desired but not current)
    Create,

    /// Entity needs to be updated (in both but differs)
    Update,

    /// Entity needs to be deleted (in current but not desired)
    Delete,
}

/// Represents a change detected for a specific entity.
///
/// This captures both the current and desired states for the entity,
/// allowing downstream components (plan, actuate) to determine
/// what actions to take.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityChange {
    /// Unique identifier for this entity
    pub entity_id: EntityId,

    /// Type of entity
    pub entity_type: EntityType,

    /// Current state (None if entity doesn't exist yet)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_state: Option<serde_json::Value>,

    /// Desired state (None if entity should be deleted)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub desired_state: Option<serde_json::Value>,

    /// Type of change required
    pub change_type: ChangeType,
}

impl EntityChange {
    /// Creates a new entity change for creation
    pub fn create(
        entity_id: EntityId,
        entity_type: EntityType,
        desired_state: serde_json::Value,
    ) -> Self {
        Self {
            entity_id,
            entity_type,
            current_state: None,
            desired_state: Some(desired_state),
            change_type: ChangeType::Create,
        }
    }

    /// Creates a new entity change for update
    pub fn update(
        entity_id: EntityId,
        entity_type: EntityType,
        current_state: serde_json::Value,
        desired_state: serde_json::Value,
    ) -> Self {
        Self {
            entity_id,
            entity_type,
            current_state: Some(current_state),
            desired_state: Some(desired_state),
            change_type: ChangeType::Update,
        }
    }

    /// Creates a new entity change for deletion
    pub fn delete(
        entity_id: EntityId,
        entity_type: EntityType,
        current_state: serde_json::Value,
    ) -> Self {
        Self {
            entity_id,
            entity_type,
            current_state: Some(current_state),
            desired_state: None,
            change_type: ChangeType::Delete,
        }
    }
}

// ================================================================================================
// DIFF RESULT
// ================================================================================================

/// The diff produced by comparison, categorizing all entities.
///
/// # DOL Properties
///
/// From `reconciliation.compare`:
/// - `diff has additions` - Entities in desired but not current (need to create)
/// - `diff has modifications` - Entities in both but with differences (need to update)
/// - `diff has deletions` - Entities in current but not desired (need to remove)
/// - `diff has unchanged` - Entities matching in both (no action needed)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Diff {
    /// Entities that need to be created
    pub additions: Vec<EntityChange>,

    /// Entities that need to be updated
    pub modifications: Vec<EntityChange>,

    /// Entities that need to be deleted
    pub deletions: Vec<EntityChange>,

    /// Entities that are already in sync (no action needed)
    pub unchanged: Vec<EntityId>,
}

impl Diff {
    /// Creates an empty diff
    pub fn empty() -> Self {
        Self {
            additions: Vec::new(),
            modifications: Vec::new(),
            deletions: Vec::new(),
            unchanged: Vec::new(),
        }
    }

    /// Returns the total number of changes (additions + modifications + deletions)
    pub fn total_changes(&self) -> usize {
        self.additions.len() + self.modifications.len() + self.deletions.len()
    }

    /// Returns the total number of entities processed
    pub fn total_entities(&self) -> usize {
        self.total_changes() + self.unchanged.len()
    }

    /// Returns true if there are any changes
    pub fn has_changes(&self) -> bool {
        self.total_changes() > 0
    }
}

// ================================================================================================
// DRIFT QUANTIFICATION
// ================================================================================================

/// Quantifies how much the system has drifted from desired state.
///
/// Magnitude can be measured in different ways:
/// - Count: Simple count of entities out of sync
/// - Percentage: Percentage of total entities that have drifted
/// - Weighted: Weighted score based on entity importance/impact
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DriftMagnitude {
    /// Simple count of drifted entities
    Count(usize),

    /// Percentage of entities that have drifted (0.0 to 100.0)
    Percentage(f64),

    /// Weighted drift score (higher = more severe)
    Weighted(f64),
}

/// Indicates how critical it is to reconcile the drift.
///
/// Urgency is typically based on:
/// - SLA requirements
/// - Priority levels
/// - Time since drift detected
/// - Impact on user workloads
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum DriftUrgency {
    /// Low urgency - can wait for next periodic reconciliation
    Low,

    /// Medium urgency - should reconcile soon
    Medium,

    /// High urgency - should reconcile immediately
    High,

    /// Critical urgency - system stability at risk, reconcile now
    Critical,
}

/// Drift quantification providing magnitude and urgency metrics.
///
/// # DOL Properties
///
/// From `reconciliation.compare`:
/// - `drift has magnitude` - How much has drifted
/// - `drift has urgency` - How critical is it to reconcile
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Drift {
    /// How much has drifted
    pub magnitude: DriftMagnitude,

    /// How critical is it to reconcile
    pub urgency: DriftUrgency,
}

impl Drift {
    /// Creates a new drift measurement from a diff
    ///
    /// This is a simple implementation that uses count-based magnitude
    /// and urgency based on the number of changes.
    pub fn from_diff(diff: &Diff) -> Self {
        let change_count = diff.total_changes();
        let total_entities = diff.total_entities();

        // Calculate percentage if there are entities
        let percentage = if total_entities > 0 {
            (change_count as f64 / total_entities as f64) * 100.0
        } else {
            0.0
        };

        // Determine urgency based on percentage drift
        let urgency = match percentage {
            p if p >= 50.0 => DriftUrgency::Critical,
            p if p >= 25.0 => DriftUrgency::High,
            p if p >= 10.0 => DriftUrgency::Medium,
            _ => DriftUrgency::Low,
        };

        Self {
            magnitude: DriftMagnitude::Percentage(percentage),
            urgency,
        }
    }

    /// Creates a zero drift (no changes)
    pub fn zero() -> Self {
        Self {
            magnitude: DriftMagnitude::Count(0),
            urgency: DriftUrgency::Low,
        }
    }
}

// ================================================================================================
// COMPARISON RESULT
// ================================================================================================

/// The complete result of a comparison operation.
///
/// This includes:
/// - The diff categorizing all entity changes
/// - The drift quantification
/// - The timestamp when comparison was performed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompareResult {
    /// The diff showing all categorized changes
    pub diff: Diff,

    /// The drift quantification
    pub drift: Drift,

    /// When this comparison was performed
    #[serde(skip, default = "instant_now")]
    pub comparison_timestamp: Instant,
}

impl CompareResult {
    /// Creates a new comparison result
    pub fn new(diff: Diff, drift: Drift) -> Self {
        Self {
            diff,
            drift,
            comparison_timestamp: Instant::now(),
        }
    }

    /// Creates an empty comparison result (no changes)
    pub fn empty() -> Self {
        Self::new(Diff::empty(), Drift::zero())
    }
}

// ================================================================================================
// COMPARATOR TRAIT
// ================================================================================================

/// The Comparator trait defines the interface for comparing desired and current state.
///
/// # DOL Requirements
///
/// From `reconciliation.compare`:
/// - `each comparison is deterministic` - Same inputs produce same outputs
/// - `each comparison is idempotent` - Running twice gives same result
///
/// # Implementations
///
/// Implementations must ensure:
/// 1. **Determinism**: Given the same desired and current state, always produce
///    the same diff and drift metrics
/// 2. **Idempotency**: Calling compare multiple times with the same inputs
///    produces identical results
/// 3. **Completeness**: All entities must be categorized (no entities missed)
pub trait Comparator {
    /// Compares desired state against current state to produce a diff and drift metrics.
    ///
    /// # Arguments
    ///
    /// * `desired` - The desired state (what should be running)
    /// * `current` - The current state (what is actually running)
    ///
    /// # Returns
    ///
    /// A `CompareResult` containing:
    /// - `diff` - Categorized changes (additions, modifications, deletions, unchanged)
    /// - `drift` - Quantified drift magnitude and urgency
    /// - `comparison_timestamp` - When the comparison was performed
    ///
    /// # Invariants
    ///
    /// - **Deterministic**: f(desired, current) = f(desired, current)
    /// - **Idempotent**: compare(d, c) = compare(d, c) = compare(d, c)
    /// - **Complete**: All entities in desired ∪ current are categorized
    fn compare(&self, desired: &DesiredState, current: &CurrentState) -> CompareResult;
}

// ================================================================================================
// TESTS
// ================================================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_states() {
        let desired = DesiredState::empty();
        let current = CurrentState::empty();

        assert_eq!(desired.workload_definitions.len(), 0);
        assert_eq!(current.nodes.len(), 0);
        assert_eq!(current.workloads.len(), 0);
        assert_eq!(current.instances.len(), 0);
    }

    #[test]
    fn test_diff_metrics() {
        let mut diff = Diff::empty();
        assert_eq!(diff.total_changes(), 0);
        assert_eq!(diff.total_entities(), 0);
        assert!(!diff.has_changes());

        // Add some changes
        diff.additions.push(EntityChange::create(
            EntityId::Workload(Uuid::new_v4()),
            EntityType::Workload,
            serde_json::json!({}),
        ));

        assert_eq!(diff.total_changes(), 1);
        assert_eq!(diff.total_entities(), 1);
        assert!(diff.has_changes());
    }

    #[test]
    fn test_drift_calculation() {
        let mut diff = Diff::empty();

        // Add 5 changes out of 10 total entities
        for _ in 0..5 {
            diff.additions.push(EntityChange::create(
                EntityId::Workload(Uuid::new_v4()),
                EntityType::Workload,
                serde_json::json!({}),
            ));
        }

        for _ in 0..5 {
            diff.unchanged.push(EntityId::Workload(Uuid::new_v4()));
        }

        let drift = Drift::from_diff(&diff);

        match drift.magnitude {
            DriftMagnitude::Percentage(p) => assert_eq!(p, 50.0),
            _ => panic!("Expected percentage magnitude"),
        }

        assert_eq!(drift.urgency, DriftUrgency::Critical);
    }

    #[test]
    fn test_drift_urgency_levels() {
        // Low urgency: < 10% drift
        let mut diff_low = Diff::empty();
        diff_low.additions.push(EntityChange::create(
            EntityId::Workload(Uuid::new_v4()),
            EntityType::Workload,
            serde_json::json!({}),
        ));
        for _ in 0..99 {
            diff_low.unchanged.push(EntityId::Workload(Uuid::new_v4()));
        }
        assert_eq!(Drift::from_diff(&diff_low).urgency, DriftUrgency::Low);

        // Medium urgency: 10-25% drift
        let mut diff_medium = Diff::empty();
        for _ in 0..15 {
            diff_medium.additions.push(EntityChange::create(
                EntityId::Workload(Uuid::new_v4()),
                EntityType::Workload,
                serde_json::json!({}),
            ));
        }
        for _ in 0..85 {
            diff_medium
                .unchanged
                .push(EntityId::Workload(Uuid::new_v4()));
        }
        assert_eq!(Drift::from_diff(&diff_medium).urgency, DriftUrgency::Medium);

        // High urgency: 25-50% drift
        let mut diff_high = Diff::empty();
        for _ in 0..30 {
            diff_high.additions.push(EntityChange::create(
                EntityId::Workload(Uuid::new_v4()),
                EntityType::Workload,
                serde_json::json!({}),
            ));
        }
        for _ in 0..70 {
            diff_high.unchanged.push(EntityId::Workload(Uuid::new_v4()));
        }
        assert_eq!(Drift::from_diff(&diff_high).urgency, DriftUrgency::High);

        // Critical urgency: >= 50% drift
        let mut diff_critical = Diff::empty();
        for _ in 0..50 {
            diff_critical.additions.push(EntityChange::create(
                EntityId::Workload(Uuid::new_v4()),
                EntityType::Workload,
                serde_json::json!({}),
            ));
        }
        for _ in 0..50 {
            diff_critical
                .unchanged
                .push(EntityId::Workload(Uuid::new_v4()));
        }
        assert_eq!(
            Drift::from_diff(&diff_critical).urgency,
            DriftUrgency::Critical
        );
    }

    #[test]
    fn test_entity_change_creation() {
        let entity_id = EntityId::Workload(Uuid::new_v4());
        let desired_state = serde_json::json!({"replicas": 3});

        let change = EntityChange::create(
            entity_id.clone(),
            EntityType::Workload,
            desired_state.clone(),
        );

        assert_eq!(change.entity_id, entity_id);
        assert_eq!(change.entity_type, EntityType::Workload);
        assert_eq!(change.change_type, ChangeType::Create);
        assert!(change.current_state.is_none());
        assert_eq!(change.desired_state, Some(desired_state));
    }

    #[test]
    fn test_entity_change_update() {
        let entity_id = EntityId::Workload(Uuid::new_v4());
        let current_state = serde_json::json!({"replicas": 2});
        let desired_state = serde_json::json!({"replicas": 3});

        let change = EntityChange::update(
            entity_id.clone(),
            EntityType::Workload,
            current_state.clone(),
            desired_state.clone(),
        );

        assert_eq!(change.change_type, ChangeType::Update);
        assert_eq!(change.current_state, Some(current_state));
        assert_eq!(change.desired_state, Some(desired_state));
    }

    #[test]
    fn test_entity_change_deletion() {
        let entity_id = EntityId::Workload(Uuid::new_v4());
        let current_state = serde_json::json!({"replicas": 2});

        let change = EntityChange::delete(
            entity_id.clone(),
            EntityType::Workload,
            current_state.clone(),
        );

        assert_eq!(change.change_type, ChangeType::Delete);
        assert_eq!(change.current_state, Some(current_state));
        assert!(change.desired_state.is_none());
    }
}
