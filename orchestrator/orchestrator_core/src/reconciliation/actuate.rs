//! Reconciliation Actuate Module
//!
//! This module implements the **actuate** phase of the reconciliation loop:
//! `sense -> compare -> plan -> actuate`
//!
//! The actuate phase is the fourth and final phase, responsible for executing
//! the planned actions to transform the cluster from its current state toward
//! the desired state. This phase handles action execution, concurrency control,
//! retry logic, failure handling, and rollback capabilities.
//!
//! # Reconciliation Loop Phases
//!
//! 1. **Sense**: Observe current state of the system
//! 2. **Compare**: Diff current state against desired state
//! 3. **Plan**: Generate actions to reconcile differences
//! 4. **Actuate** (this module): Execute planned actions to reach desired state
//!
//! # Key Concepts
//!
//! - **Concurrency Control**: Limits parallel action execution to prevent cluster overload
//! - **Dependency Respect**: Ensures actions wait for their dependencies before executing
//! - **Progress Tracking**: Provides visibility into execution state and progress
//! - **Failure Handling**: Implements retry policies with backoff strategies
//! - **Rollback Support**: Enables reverting to known good states on persistent failures
//! - **Event Emission**: Emits events for observability, monitoring, and debugging

use std::time::{Duration, Instant};
use uuid::Uuid;

use super::sense::CurrentState;
pub use super::plan::{ReconciliationPlan, Action, ActionSequence, RollbackPoint};

/// Unique identifier for an action in the reconciliation plan.
pub type ActionId = Uuid;

/// Represents the result state of an individual action execution.
///
/// Actions follow a state machine progression:
/// `Pending -> Running -> (Succeeded | Failed | Skipped)`
#[derive(Debug, Clone, PartialEq)]
pub enum ActionResult {
    /// Action has not yet started execution.
    Pending,

    /// Action is currently being executed.
    Running,

    /// Action completed successfully.
    Succeeded,

    /// Action failed with the given error message.
    Failed(String),

    /// Action was skipped due to dependency failure or condition not met.
    Skipped(String),
}

/// Backoff strategy for retry attempts.
///
/// Determines how the delay between retry attempts increases.
#[derive(Debug, Clone, PartialEq)]
pub enum BackoffStrategy {
    /// Fixed delay between all retry attempts.
    Fixed,

    /// Exponentially increasing delay (e.g., 1s, 2s, 4s, 8s).
    Exponential,

    /// Linearly increasing delay (e.g., 1s, 2s, 3s, 4s).
    Linear,
}

/// Policy for retrying failed actions.
///
/// Defines how many times to retry and with what delay pattern.
#[derive(Debug, Clone, PartialEq)]
pub struct RetryPolicy {
    /// Maximum number of retry attempts before giving up.
    pub max_attempts: u32,

    /// Strategy for calculating delay between retries.
    pub backoff_strategy: BackoffStrategy,

    /// Initial delay before the first retry attempt.
    pub initial_delay: Duration,
}

impl RetryPolicy {
    /// Creates a new retry policy with the given parameters.
    pub fn new(max_attempts: u32, backoff_strategy: BackoffStrategy, initial_delay: Duration) -> Self {
        Self {
            max_attempts,
            backoff_strategy,
            initial_delay,
        }
    }

    /// Creates a retry policy with no retries.
    pub fn no_retry() -> Self {
        Self {
            max_attempts: 0,
            backoff_strategy: BackoffStrategy::Fixed,
            initial_delay: Duration::from_secs(0),
        }
    }

    /// Creates a retry policy with fixed backoff.
    pub fn fixed(max_attempts: u32, delay: Duration) -> Self {
        Self {
            max_attempts,
            backoff_strategy: BackoffStrategy::Fixed,
            initial_delay: delay,
        }
    }

    /// Creates a retry policy with exponential backoff.
    pub fn exponential(max_attempts: u32, initial_delay: Duration) -> Self {
        Self {
            max_attempts,
            backoff_strategy: BackoffStrategy::Exponential,
            initial_delay,
        }
    }

    /// Calculates the delay for a specific retry attempt number.
    ///
    /// # Arguments
    ///
    /// * `attempt` - The retry attempt number (0-indexed)
    pub fn calculate_delay(&self, attempt: u32) -> Duration {
        match self.backoff_strategy {
            BackoffStrategy::Fixed => self.initial_delay,
            BackoffStrategy::Exponential => {
                let multiplier = 2_u32.pow(attempt);
                self.initial_delay * multiplier
            }
            BackoffStrategy::Linear => {
                let multiplier = attempt + 1;
                self.initial_delay * multiplier
            }
        }
    }
}

/// Tracks the progress of action execution during reconciliation.
///
/// Provides real-time visibility into the state of the reconciliation cycle.
#[derive(Debug, Clone, PartialEq)]
pub struct ExecutionProgress {
    /// Total number of actions in the plan.
    pub total_actions: usize,

    /// Number of actions that have completed successfully.
    pub completed: usize,

    /// Number of actions currently running.
    pub running: usize,

    /// Number of actions that have failed.
    pub failed: usize,

    /// Number of actions that were skipped.
    pub skipped: usize,
}

impl ExecutionProgress {
    /// Creates a new progress tracker initialized to zero for all counters.
    pub fn new(total_actions: usize) -> Self {
        Self {
            total_actions,
            completed: 0,
            running: 0,
            failed: 0,
            skipped: 0,
        }
    }

    /// Returns the number of pending (not yet started) actions.
    pub fn pending(&self) -> usize {
        self.total_actions
            .saturating_sub(self.completed + self.running + self.failed + self.skipped)
    }

    /// Returns true if all actions have completed (succeeded, failed, or skipped).
    pub fn is_complete(&self) -> bool {
        self.pending() == 0 && self.running == 0
    }

    /// Returns the completion percentage as a value between 0.0 and 1.0.
    pub fn completion_ratio(&self) -> f64 {
        if self.total_actions == 0 {
            1.0
        } else {
            (self.completed + self.failed + self.skipped) as f64 / self.total_actions as f64
        }
    }
}

/// The outcome of executing a reconciliation plan.
///
/// Captures the results of all actions along with the final system state.
#[derive(Debug, Clone, PartialEq)]
pub enum ExecutionOutcome {
    /// No actions were required; system is already in desired state.
    NoActionRequired,

    /// Actions were executed with the following results.
    ActionsExecuted {
        /// List of action IDs that succeeded.
        actions_succeeded: Vec<ActionId>,

        /// List of action IDs that failed with their error messages.
        actions_failed: Vec<(ActionId, String)>,

        /// The final state of the system after execution.
        final_state: CurrentState,

        /// Correlation ID for tracking this execution across logs and traces.
        correlation_id: Uuid,
    },

    /// Rollback was triggered due to failures.
    RolledBack {
        /// The rollback point that was restored.
        rollback_point: RollbackPoint,

        /// Reason for the rollback.
        reason: String,

        /// Correlation ID for tracking this execution.
        correlation_id: Uuid,
    },
}

/// Events emitted during action execution for observability.
///
/// These events enable real-time monitoring, alerting, audit logging,
/// and debugging of distributed reconciliation behavior.
#[derive(Debug, Clone, PartialEq)]
pub enum ActuationEvent {
    /// An action has started execution.
    ActionStarted {
        /// ID of the action that started.
        action_id: ActionId,

        /// Timestamp when the action started.
        timestamp: Instant,
    },

    /// An action has completed successfully.
    ActionCompleted {
        /// ID of the action that completed.
        action_id: ActionId,

        /// The result of the action.
        result: ActionResult,

        /// Timestamp when the action completed.
        timestamp: Instant,
    },

    /// An action has failed.
    ActionFailed {
        /// ID of the action that failed.
        action_id: ActionId,

        /// Error message describing the failure.
        error: String,

        /// Timestamp when the action failed.
        timestamp: Instant,
    },

    /// A rollback operation has been initiated.
    RollbackInitiated {
        /// The rollback point being restored.
        rollback_point: RollbackPoint,

        /// Reason for initiating the rollback.
        reason: String,

        /// Timestamp when rollback was initiated.
        timestamp: Instant,
    },
}

/// Errors that can occur during actuation.
#[derive(Debug, Clone, PartialEq)]
pub enum ActuationError {
    /// One or more actions failed to execute.
    ActionExecutionFailed {
        /// Number of actions that failed.
        failed_count: usize,

        /// Details of the failures.
        failures: Vec<(ActionId, String)>,
    },

    /// The execution exceeded the maximum allowed duration.
    ExecutionTimeout,

    /// An invalid plan was provided (e.g., circular dependencies).
    InvalidPlan(String),

    /// Concurrency limit validation failed.
    InvalidConcurrencyLimit(String),

    /// Generic error during actuation.
    Other(String),
}

impl std::fmt::Display for ActuationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ActuationError::ActionExecutionFailed { failed_count, failures } => {
                write!(f, "{} actions failed: {:?}", failed_count, failures)
            }
            ActuationError::ExecutionTimeout => write!(f, "Execution timeout"),
            ActuationError::InvalidPlan(msg) => write!(f, "Invalid plan: {}", msg),
            ActuationError::InvalidConcurrencyLimit(msg) => {
                write!(f, "Invalid concurrency limit: {}", msg)
            }
            ActuationError::Other(msg) => write!(f, "Actuation error: {}", msg),
        }
    }
}

impl std::error::Error for ActuationError {}

/// Errors that can occur during rollback operations.
#[derive(Debug, Clone, PartialEq)]
pub enum RollbackError {
    /// Failed to restore the rollback point.
    RestoreFailed(String),

    /// The rollback point is invalid or corrupted.
    InvalidRollbackPoint(String),

    /// Rollback operation timed out.
    Timeout,

    /// Generic rollback error.
    Other(String),
}

impl std::fmt::Display for RollbackError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RollbackError::RestoreFailed(msg) => write!(f, "Restore failed: {}", msg),
            RollbackError::InvalidRollbackPoint(msg) => {
                write!(f, "Invalid rollback point: {}", msg)
            }
            RollbackError::Timeout => write!(f, "Rollback timeout"),
            RollbackError::Other(msg) => write!(f, "Rollback error: {}", msg),
        }
    }
}

impl std::error::Error for RollbackError {}

/// Trait for implementing actuators that execute reconciliation plans.
///
/// Actuators are responsible for:
/// - Executing actions in the correct order respecting dependencies
/// - Managing concurrency to prevent cluster overload
/// - Handling failures with retry logic
/// - Rolling back to safe states when necessary
/// - Emitting events for observability
///
/// # Examples
///
/// ```ignore
/// use orchestrator_core::reconciliation::actuate::{
///     Actuator, ActuationEvent, ExecutionOutcome, ActuationError, RollbackError, RollbackPoint
/// };
/// use orchestrator_core::reconciliation::plan::ReconciliationPlan;
///
/// struct KubernetesActuator;
///
/// #[async_trait::async_trait]
/// impl Actuator for KubernetesActuator {
///     async fn execute(
///         &self,
///         plan: &ReconciliationPlan,
///         concurrency_limit: usize,
///     ) -> Result<ExecutionOutcome, ActuationError> {
///         // Execute actions from the plan
///         todo!()
///     }
///
///     async fn rollback(
///         &self,
///         rollback_point: &RollbackPoint,
///     ) -> Result<(), RollbackError> {
///         // Restore system to rollback point
///         todo!()
///     }
///
///     fn emit_event(&self, event: ActuationEvent) {
///         println!("{:?}", event);
///     }
/// }
/// ```
#[async_trait::async_trait]
pub trait Actuator: Send + Sync {
    /// Executes the reconciliation plan with the given concurrency limit.
    ///
    /// This operation must:
    /// - Respect action dependencies (execute in correct order)
    /// - Limit parallel execution to the concurrency limit
    /// - Handle failures according to retry policies
    /// - Emit events for all significant state changes
    ///
    /// # Arguments
    ///
    /// * `plan` - The reconciliation plan to execute
    /// * `concurrency_limit` - Maximum number of actions to execute in parallel
    ///
    /// # Returns
    ///
    /// Returns an `ExecutionOutcome` describing what happened, or an error
    /// if the execution failed catastrophically.
    ///
    /// # Errors
    ///
    /// - `ActuationError::ActionExecutionFailed`: One or more actions failed
    /// - `ActuationError::ExecutionTimeout`: Execution took too long
    /// - `ActuationError::InvalidPlan`: Plan has circular dependencies or other issues
    /// - `ActuationError::InvalidConcurrencyLimit`: Concurrency limit is invalid
    async fn execute(
        &self,
        plan: &ReconciliationPlan,
        concurrency_limit: usize,
    ) -> Result<ExecutionOutcome, ActuationError>;

    /// Rolls back the system to a previously captured rollback point.
    ///
    /// This operation must:
    /// - Restore system state to match the rollback point
    /// - Leave the system in a consistent state
    /// - Be idempotent (safe to call multiple times)
    ///
    /// # Arguments
    ///
    /// * `rollback_point` - The state to restore to
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if rollback succeeded, or an error if it failed.
    ///
    /// # Errors
    ///
    /// - `RollbackError::RestoreFailed`: Could not restore the state
    /// - `RollbackError::InvalidRollbackPoint`: Rollback point is corrupted
    /// - `RollbackError::Timeout`: Rollback took too long
    async fn rollback(&self, rollback_point: &RollbackPoint) -> Result<(), RollbackError>;

    /// Emits an actuation event for observability.
    ///
    /// Events should be sent to monitoring, logging, or tracing systems
    /// for real-time visibility into reconciliation behavior.
    ///
    /// # Arguments
    ///
    /// * `event` - The event to emit
    fn emit_event(&self, event: ActuationEvent);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_action_result_states() {
        assert_eq!(ActionResult::Pending, ActionResult::Pending);
        assert_eq!(ActionResult::Running, ActionResult::Running);
        assert_eq!(ActionResult::Succeeded, ActionResult::Succeeded);
        assert_eq!(
            ActionResult::Failed("error".to_string()),
            ActionResult::Failed("error".to_string())
        );
        assert_eq!(
            ActionResult::Skipped("reason".to_string()),
            ActionResult::Skipped("reason".to_string())
        );
    }

    #[test]
    fn test_retry_policy_no_retry() {
        let policy = RetryPolicy::no_retry();
        assert_eq!(policy.max_attempts, 0);
        assert_eq!(policy.backoff_strategy, BackoffStrategy::Fixed);
    }

    #[test]
    fn test_retry_policy_fixed() {
        let policy = RetryPolicy::fixed(3, Duration::from_secs(5));
        assert_eq!(policy.max_attempts, 3);
        assert_eq!(policy.backoff_strategy, BackoffStrategy::Fixed);
        assert_eq!(policy.calculate_delay(0), Duration::from_secs(5));
        assert_eq!(policy.calculate_delay(1), Duration::from_secs(5));
        assert_eq!(policy.calculate_delay(2), Duration::from_secs(5));
    }

    #[test]
    fn test_retry_policy_exponential() {
        let policy = RetryPolicy::exponential(4, Duration::from_secs(1));
        assert_eq!(policy.backoff_strategy, BackoffStrategy::Exponential);
        assert_eq!(policy.calculate_delay(0), Duration::from_secs(1));
        assert_eq!(policy.calculate_delay(1), Duration::from_secs(2));
        assert_eq!(policy.calculate_delay(2), Duration::from_secs(4));
        assert_eq!(policy.calculate_delay(3), Duration::from_secs(8));
    }

    #[test]
    fn test_retry_policy_linear() {
        let policy = RetryPolicy::new(
            3,
            BackoffStrategy::Linear,
            Duration::from_secs(2),
        );
        assert_eq!(policy.calculate_delay(0), Duration::from_secs(2));
        assert_eq!(policy.calculate_delay(1), Duration::from_secs(4));
        assert_eq!(policy.calculate_delay(2), Duration::from_secs(6));
    }

    #[test]
    fn test_execution_progress_new() {
        let progress = ExecutionProgress::new(10);
        assert_eq!(progress.total_actions, 10);
        assert_eq!(progress.completed, 0);
        assert_eq!(progress.running, 0);
        assert_eq!(progress.failed, 0);
        assert_eq!(progress.skipped, 0);
        assert_eq!(progress.pending(), 10);
        assert!(!progress.is_complete());
    }

    #[test]
    fn test_execution_progress_completion() {
        let mut progress = ExecutionProgress::new(5);
        progress.completed = 3;
        progress.failed = 1;
        progress.skipped = 1;

        assert_eq!(progress.pending(), 0);
        assert!(progress.is_complete());
        assert_eq!(progress.completion_ratio(), 1.0);
    }

    #[test]
    fn test_execution_progress_partial() {
        let mut progress = ExecutionProgress::new(10);
        progress.completed = 3;
        progress.running = 2;
        progress.failed = 1;

        assert_eq!(progress.pending(), 4);
        assert!(!progress.is_complete());
        assert_eq!(progress.completion_ratio(), 0.4);
    }

    #[test]
    fn test_actuation_error_display() {
        let err = ActuationError::ExecutionTimeout;
        assert_eq!(err.to_string(), "Execution timeout");

        let err = ActuationError::InvalidPlan("circular dependency".to_string());
        assert_eq!(err.to_string(), "Invalid plan: circular dependency");
    }

    #[test]
    fn test_rollback_error_display() {
        let err = RollbackError::Timeout;
        assert_eq!(err.to_string(), "Rollback timeout");

        let err = RollbackError::RestoreFailed("checkpoint corrupt".to_string());
        assert_eq!(err.to_string(), "Restore failed: checkpoint corrupt");
    }
}
