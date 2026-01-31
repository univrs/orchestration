//! Reconciliation planning phase - Phase 3 of the reconciliation loop.
//!
//! The plan module translates the diff from the compare phase into a concrete sequence
//! of actions that will transform current state into desired state.
//!
//! Planning is not simply executing all diffs in order. It requires:
//!
//! 1. **Dependency ordering**: Some actions must complete before others (e.g.,
//!    create network before creating container that uses it).
//!
//! 2. **Rollback points**: Marking safe states to return to if actions fail,
//!    enabling partial rollback rather than full reversion.
//!
//! 3. **Preconditions**: Each action may have requirements that must be met,
//!    either from other actions completing or external state checks.
//!
//! 4. **Optimization**: Multiple strategies exist for sequencing:
//!    - Minimal disruption: Batch similar actions, use rolling updates
//!    - Resource efficiency: Consider capacity during transitions
//!    - Failure isolation: Group actions so failures don't cascade
//!
//! # Example
//!
//! ```ignore
//! use orchestrator_core::reconciliation::plan::{Planner, ReconciliationPlan, PlanError};
//! use orchestrator_core::reconciliation::compare::Diff;
//!
//! async fn plan_reconciliation(planner: &impl Planner, diff: &Diff) -> Result<ReconciliationPlan, PlanError> {
//!     // Generate the plan from the diff
//!     let plan = planner.plan(diff).await?;
//!
//!     // Validate the plan before execution
//!     planner.validate(&plan).await?;
//!
//!     Ok(plan)
//! }
//! ```

use super::compare::{Diff, EntityId};
use serde::{Deserialize, Serialize};
use serde_json;
use std::collections::HashMap;
use std::time::Duration;
use thiserror::Error;
use uuid::Uuid;

/// Unique identifier for an action within a plan.
pub type ActionId = Uuid;

/// Type of action to perform during reconciliation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ActionType {
    /// Create a new entity
    Create,
    /// Update an existing entity
    Update,
    /// Delete an existing entity
    Delete,
    /// Migrate an entity from one location to another
    Migrate,
    /// Scale an entity up or down
    Scale,
}

/// Risk level associated with an action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum RiskLevel {
    /// Low risk - unlikely to cause issues
    Low,
    /// Medium risk - may require attention
    Medium,
    /// High risk - requires careful execution and monitoring
    High,
}

/// Indicates whether an action can be reversed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Reversibility {
    /// Action can be fully reversed
    Reversible,
    /// Action can be partially reversed
    PartiallyReversible,
    /// Action cannot be reversed
    Irreversible,
}

/// Reference to an entity being reconciled, including its current and desired states.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityReference {
    /// Unique identifier for the entity
    pub entity_id: EntityId,
    /// Current state of the entity (if it exists)
    pub current_state: Option<serde_json::Value>,
    /// Desired state of the entity
    pub desired_state: Option<serde_json::Value>,
}

impl EntityReference {
    /// Create a new entity reference.
    pub fn new(
        entity_id: EntityId,
        current_state: Option<serde_json::Value>,
        desired_state: Option<serde_json::Value>,
    ) -> Self {
        Self {
            entity_id,
            current_state,
            desired_state,
        }
    }
}

/// Precondition that must be satisfied before an action can execute.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Precondition {
    /// Human-readable description of the precondition
    pub description: String,
    /// Type of precondition check
    pub check_type: PreconditionType,
}

/// Types of preconditions that can be checked.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PreconditionType {
    /// Another action must complete successfully
    ActionCompleted(ActionId),
    /// An entity must exist
    EntityExists(EntityId),
    /// An entity must be in a specific state
    EntityState {
        entity_id: EntityId,
        required_state: serde_json::Value,
    },
    /// A resource must be available
    ResourceAvailable {
        resource_type: String,
        minimum_amount: f64,
    },
    /// Custom precondition check
    Custom(String),
}

impl Precondition {
    /// Create a new precondition.
    pub fn new(description: String, check_type: PreconditionType) -> Self {
        Self {
            description,
            check_type,
        }
    }

    /// Create a precondition that requires an action to complete.
    pub fn action_completed(action_id: ActionId, description: String) -> Self {
        Self {
            description,
            check_type: PreconditionType::ActionCompleted(action_id),
        }
    }

    /// Create a precondition that requires an entity to exist.
    pub fn entity_exists(entity_id: EntityId, description: String) -> Self {
        Self {
            description,
            check_type: PreconditionType::EntityExists(entity_id),
        }
    }
}

/// A single action to be performed during reconciliation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Action {
    /// Unique identifier for this action
    pub id: ActionId,
    /// Type of action to perform
    pub action_type: ActionType,
    /// Entity being acted upon
    pub target: EntityReference,
    /// Preconditions that must be satisfied before execution
    pub preconditions: Vec<Precondition>,
    /// Estimated time this action will take to complete
    pub estimated_duration: Duration,
    /// Risk level associated with this action
    pub risk_level: RiskLevel,
    /// Whether this action can be reversed
    pub reversibility: Reversibility,
    /// Optional description of the action
    pub description: Option<String>,
}

impl Action {
    /// Create a new action.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        action_type: ActionType,
        target: EntityReference,
        preconditions: Vec<Precondition>,
        estimated_duration: Duration,
        risk_level: RiskLevel,
        reversibility: Reversibility,
        description: Option<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            action_type,
            target,
            preconditions,
            estimated_duration,
            risk_level,
            reversibility,
            description,
        }
    }

    /// Check if this action has any preconditions.
    pub fn has_preconditions(&self) -> bool {
        !self.preconditions.is_empty()
    }

    /// Get the total estimated duration of this action.
    pub fn duration(&self) -> Duration {
        self.estimated_duration
    }

    /// Check if this action is reversible.
    pub fn is_reversible(&self) -> bool {
        matches!(self.reversibility, Reversibility::Reversible)
    }
}

/// Snapshot of system state at a particular point in time.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StateSnapshot {
    /// Timestamp when the snapshot was taken
    pub timestamp: std::time::SystemTime,
    /// Serialized state data
    pub state_data: serde_json::Value,
    /// Description of what state was captured
    pub description: Option<String>,
}

impl StateSnapshot {
    /// Create a new state snapshot.
    pub fn new(state_data: serde_json::Value, description: Option<String>) -> Self {
        Self {
            timestamp: std::time::SystemTime::now(),
            state_data,
            description,
        }
    }
}

/// A point in the action sequence where the system can safely rollback to.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RollbackPoint {
    /// The action after which this rollback point is established
    pub action_id: ActionId,
    /// Snapshot of the system state at this point
    pub snapshot: StateSnapshot,
    /// Description of this rollback point
    pub description: Option<String>,
}

impl RollbackPoint {
    /// Create a new rollback point.
    pub fn new(action_id: ActionId, snapshot: StateSnapshot, description: Option<String>) -> Self {
        Self {
            action_id,
            snapshot,
            description,
        }
    }
}

/// A sequence of ordered actions with dependencies and rollback points.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionSequence {
    /// Actions in the order they should be executed
    pub ordered_actions: Vec<Action>,
    /// Dependencies between actions (maps action ID to IDs of actions it depends on)
    pub dependencies: HashMap<ActionId, Vec<ActionId>>,
    /// Rollback points established during the sequence
    pub rollback_points: Vec<RollbackPoint>,
}

impl ActionSequence {
    /// Create a new empty action sequence.
    pub fn new() -> Self {
        Self {
            ordered_actions: Vec::new(),
            dependencies: HashMap::new(),
            rollback_points: Vec::new(),
        }
    }

    /// Add an action to the sequence.
    pub fn add_action(&mut self, action: Action) {
        self.ordered_actions.push(action);
    }

    /// Add a dependency between two actions.
    pub fn add_dependency(&mut self, action_id: ActionId, depends_on: ActionId) {
        self.dependencies
            .entry(action_id)
            .or_insert_with(Vec::new)
            .push(depends_on);
    }

    /// Add a rollback point to the sequence.
    pub fn add_rollback_point(&mut self, rollback_point: RollbackPoint) {
        self.rollback_points.push(rollback_point);
    }

    /// Get the total number of actions in the sequence.
    pub fn action_count(&self) -> usize {
        self.ordered_actions.len()
    }

    /// Check if the sequence is empty.
    pub fn is_empty(&self) -> bool {
        self.ordered_actions.is_empty()
    }

    /// Get the total estimated duration for all actions.
    pub fn total_estimated_duration(&self) -> Duration {
        self.ordered_actions
            .iter()
            .map(|a| a.estimated_duration)
            .sum()
    }

    /// Get all actions with a specific risk level.
    pub fn actions_by_risk(&self, risk_level: RiskLevel) -> Vec<&Action> {
        self.ordered_actions
            .iter()
            .filter(|a| a.risk_level == risk_level)
            .collect()
    }

    /// Validate that all dependency references are valid.
    pub fn validate_dependencies(&self) -> Result<(), ValidationError> {
        let action_ids: HashMap<ActionId, ()> =
            self.ordered_actions.iter().map(|a| (a.id, ())).collect();

        for (action_id, deps) in &self.dependencies {
            // Check that the action exists
            if !action_ids.contains_key(action_id) {
                return Err(ValidationError::InvalidDependency(format!(
                    "Dependency references non-existent action: {}",
                    action_id
                )));
            }

            // Check that all dependencies exist
            for dep_id in deps {
                if !action_ids.contains_key(dep_id) {
                    return Err(ValidationError::InvalidDependency(format!(
                        "Action {} depends on non-existent action: {}",
                        action_id, dep_id
                    )));
                }
            }
        }

        Ok(())
    }

    /// Check for circular dependencies in the action sequence.
    pub fn has_circular_dependencies(&self) -> bool {
        // Simple cycle detection using DFS
        let mut visited = HashMap::new();
        let mut rec_stack = HashMap::new();

        for action in &self.ordered_actions {
            if self.has_cycle_util(action.id, &mut visited, &mut rec_stack) {
                return true;
            }
        }

        false
    }

    fn has_cycle_util(
        &self,
        action_id: ActionId,
        visited: &mut HashMap<ActionId, bool>,
        rec_stack: &mut HashMap<ActionId, bool>,
    ) -> bool {
        if rec_stack.get(&action_id) == Some(&true) {
            return true;
        }

        if visited.get(&action_id) == Some(&true) {
            return false;
        }

        visited.insert(action_id, true);
        rec_stack.insert(action_id, true);

        if let Some(deps) = self.dependencies.get(&action_id) {
            for &dep_id in deps {
                if self.has_cycle_util(dep_id, visited, rec_stack) {
                    return true;
                }
            }
        }

        rec_stack.insert(action_id, false);
        false
    }
}

impl Default for ActionSequence {
    fn default() -> Self {
        Self::new()
    }
}

/// A complete reconciliation plan with validated action sequence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReconciliationPlan {
    /// Unique identifier for this plan
    pub plan_id: Uuid,
    /// The diff that this plan addresses
    pub diff: Diff,
    /// Sequence of actions to execute
    pub action_sequence: ActionSequence,
    /// Whether this plan has been validated
    pub validated: bool,
    /// Timestamp when the plan was created
    pub created_at: std::time::SystemTime,
    /// Optional description of the plan
    pub description: Option<String>,
}

impl ReconciliationPlan {
    /// Create a new reconciliation plan.
    pub fn new(diff: Diff, action_sequence: ActionSequence) -> Self {
        Self {
            plan_id: Uuid::new_v4(),
            diff,
            action_sequence,
            validated: false,
            created_at: std::time::SystemTime::now(),
            description: None,
        }
    }

    /// Mark this plan as validated.
    pub fn mark_validated(&mut self) {
        self.validated = true;
    }

    /// Check if the plan is empty (no actions).
    pub fn is_empty(&self) -> bool {
        self.action_sequence.is_empty()
    }

    /// Get the total number of actions in the plan.
    pub fn action_count(&self) -> usize {
        self.action_sequence.action_count()
    }

    /// Set a description for the plan.
    pub fn with_description(mut self, description: String) -> Self {
        self.description = Some(description);
        self
    }
}

/// Errors that can occur during planning.
#[derive(Debug, Error)]
pub enum PlanError {
    /// The diff is invalid or malformed
    #[error("Invalid diff: {0}")]
    InvalidDiff(String),

    /// Unable to generate a valid action sequence
    #[error("Failed to generate action sequence: {0}")]
    ActionGenerationFailed(String),

    /// Action ordering failed due to circular dependencies
    #[error("Circular dependency detected: {0}")]
    CircularDependency(String),

    /// Resource constraints prevent plan generation
    #[error("Insufficient resources: {0}")]
    InsufficientResources(String),

    /// Planning timeout
    #[error("Planning timeout exceeded")]
    Timeout,

    /// Internal planning error
    #[error("Internal planning error: {0}")]
    Internal(String),
}

/// Errors that can occur during plan validation.
#[derive(Debug, Error)]
pub enum ValidationError {
    /// Plan contains invalid actions
    #[error("Invalid action: {0}")]
    InvalidAction(String),

    /// Action sequence has invalid dependencies
    #[error("Invalid dependency: {0}")]
    InvalidDependency(String),

    /// Plan has circular dependencies
    #[error("Circular dependencies detected")]
    CircularDependencies,

    /// Preconditions cannot be satisfied
    #[error("Unsatisfiable precondition: {0}")]
    UnsatisfiablePrecondition(String),

    /// Plan violates safety constraints
    #[error("Safety constraint violated: {0}")]
    SafetyViolation(String),

    /// Plan is too risky to execute
    #[error("Plan risk exceeds acceptable threshold: {0}")]
    ExcessiveRisk(String),

    /// Validation timeout
    #[error("Validation timeout exceeded")]
    Timeout,

    /// Internal validation error
    #[error("Internal validation error: {0}")]
    Internal(String),
}

/// Trait for components that generate reconciliation plans from diffs.
///
/// Implementors of this trait take a diff (from the compare phase) and generate
/// a concrete, ordered sequence of actions that will reconcile the system state.
#[async_trait::async_trait]
pub trait Planner: Send + Sync {
    /// Generate a reconciliation plan from a diff.
    ///
    /// This method analyzes the diff and produces an ordered sequence of actions
    /// that will transform the current state into the desired state.
    ///
    /// # Arguments
    ///
    /// * `diff` - The difference between current and desired state
    ///
    /// # Returns
    ///
    /// A reconciliation plan with an ordered, dependency-aware action sequence.
    ///
    /// # Errors
    ///
    /// Returns `PlanError` if the plan cannot be generated (e.g., circular
    /// dependencies, insufficient resources, etc.).
    async fn plan(&self, diff: &Diff) -> Result<ReconciliationPlan, PlanError>;

    /// Validate a reconciliation plan before execution.
    ///
    /// This method performs a "dry-run" validation of the plan to ensure:
    /// - All dependencies are valid
    /// - No circular dependencies exist
    /// - Preconditions can be satisfied
    /// - Safety constraints are met
    /// - Risk levels are acceptable
    ///
    /// # Arguments
    ///
    /// * `plan` - The plan to validate
    ///
    /// # Errors
    ///
    /// Returns `ValidationError` if the plan is invalid or unsafe to execute.
    async fn validate(&self, plan: &ReconciliationPlan) -> Result<(), ValidationError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entity_reference_creation() {
        let entity_id = EntityId::Workload(Uuid::new_v4());
        let entity_ref = EntityReference::new(
            entity_id.clone(),
            None,
            Some(serde_json::json!({"replicas": 3})),
        );

        assert_eq!(entity_ref.entity_id, entity_id);
        assert!(entity_ref.current_state.is_none());
        assert!(entity_ref.desired_state.is_some());
    }

    #[test]
    fn test_action_sequence_empty() {
        let sequence = ActionSequence::new();
        assert!(sequence.is_empty());
        assert_eq!(sequence.action_count(), 0);
    }

    #[test]
    fn test_action_sequence_add_action() {
        let mut sequence = ActionSequence::new();
        let action = Action::new(
            ActionType::Create,
            EntityReference::new(
                EntityId::Workload(Uuid::new_v4()),
                None,
                Some(serde_json::json!({"replicas": 3})),
            ),
            Vec::new(),
            Duration::from_secs(30),
            RiskLevel::Low,
            Reversibility::Reversible,
            Some("Create workload".to_string()),
        );

        sequence.add_action(action);
        assert_eq!(sequence.action_count(), 1);
        assert!(!sequence.is_empty());
    }

    #[test]
    fn test_action_sequence_circular_dependency_detection() {
        let mut sequence = ActionSequence::new();

        let action1_id = Uuid::new_v4();
        let action2_id = Uuid::new_v4();

        let mut action1 = Action::new(
            ActionType::Create,
            EntityReference::new(EntityId::Workload(Uuid::new_v4()), None, None),
            Vec::new(),
            Duration::from_secs(10),
            RiskLevel::Low,
            Reversibility::Reversible,
            None,
        );
        action1.id = action1_id;

        let mut action2 = Action::new(
            ActionType::Create,
            EntityReference::new(EntityId::Workload(Uuid::new_v4()), None, None),
            Vec::new(),
            Duration::from_secs(10),
            RiskLevel::Low,
            Reversibility::Reversible,
            None,
        );
        action2.id = action2_id;

        sequence.add_action(action1);
        sequence.add_action(action2);

        // Create circular dependency: action1 -> action2 -> action1
        sequence.add_dependency(action1_id, action2_id);
        sequence.add_dependency(action2_id, action1_id);

        assert!(sequence.has_circular_dependencies());
    }

    #[test]
    fn test_reconciliation_plan_creation() {
        let diff = Diff::empty();
        let sequence = ActionSequence::new();
        let plan = ReconciliationPlan::new(diff, sequence);

        assert!(!plan.validated);
        assert!(plan.is_empty());
    }

    #[test]
    fn test_action_reversibility() {
        let action = Action::new(
            ActionType::Delete,
            EntityReference::new(EntityId::Container("test123".to_string()), None, None),
            Vec::new(),
            Duration::from_secs(5),
            RiskLevel::High,
            Reversibility::Irreversible,
            None,
        );

        assert!(!action.is_reversible());
        assert_eq!(action.risk_level, RiskLevel::High);
    }

    #[test]
    fn test_precondition_creation() {
        let action_id = Uuid::new_v4();
        let precondition =
            Precondition::action_completed(action_id, "Wait for network creation".to_string());

        assert_eq!(precondition.description, "Wait for network creation");
        assert!(matches!(
            precondition.check_type,
            PreconditionType::ActionCompleted(_)
        ));
    }

    #[test]
    fn test_state_snapshot_creation() {
        let state_data = serde_json::json!({
            "workloads": 5,
            "nodes": 3
        });

        let snapshot =
            StateSnapshot::new(state_data.clone(), Some("Pre-update snapshot".to_string()));

        assert_eq!(snapshot.state_data, state_data);
        assert_eq!(
            snapshot.description,
            Some("Pre-update snapshot".to_string())
        );
    }
}
