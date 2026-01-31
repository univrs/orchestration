//! Reconciliation control loop implementation.
//!
//! The reconciliation loop continuously drives the system toward desired state:
//! 1. **Sense**: Observe current state of the cluster
//! 2. **Compare**: Diff current vs desired state
//! 3. **Plan**: Generate ordered actions to reconcile
//! 4. **Actuate**: Execute the plan
//!
//! # Architecture
//!
//! The reconciliation loop follows the classic control loop pattern:
//!
//! ```text
//! ┌─────────┐      ┌─────────┐      ┌──────┐      ┌─────────┐
//! │ Sense   │─────▶│ Compare │─────▶│ Plan │─────▶│ Actuate │
//! └─────────┘      └─────────┘      └──────┘      └─────────┘
//!      ▲                                                │
//!      │                                                │
//!      └────────────────────────────────────────────────┘
//!                    Feedback Loop
//! ```
//!
//! # Usage
//!
//! ```ignore
//! use orchestrator_core::reconciliation::{
//!     ReconciliationLoop, Sensor, Comparator, Planner, Actuator,
//!     DesiredState, SenseScope,
//! };
//!
//! // Assemble the reconciliation loop with concrete implementations
//! let loop_instance = ReconciliationLoop::new(
//!     Box::new(my_sensor),
//!     Box::new(my_comparator),
//!     Box::new(my_planner),
//!     Box::new(my_actuator),
//!     4, // concurrency limit
//! );
//!
//! // Run a single reconciliation cycle
//! let desired = DesiredState::new();
//! let scope = SenseScope::default();
//! loop_instance.run_cycle(&desired, &scope).await?;
//! ```

pub mod actuate;
pub mod compare;
pub mod plan;
pub mod sense;

pub use actuate::{ActuationEvent, Actuator, ExecutionOutcome};
pub use compare::{Comparator, CompareResult, CurrentState, DesiredState, Diff};
pub use plan::{Action, ActionSequence, Planner, ReconciliationPlan};
pub use sense::{CurrentState as SenseCurrentState, SenseOperation, SenseScope, Sensor};

use std::fmt;
use tracing::{debug, error};

/// Error type for reconciliation operations.
#[derive(Debug)]
pub enum ReconciliationError {
    /// Error during sensing phase
    SenseError(String),
    /// Error during comparison phase
    CompareError(String),
    /// Error during planning phase
    PlanError(String),
    /// Error during actuation phase
    ActuateError(String),
}

impl fmt::Display for ReconciliationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReconciliationError::SenseError(msg) => write!(f, "Sense error: {}", msg),
            ReconciliationError::CompareError(msg) => write!(f, "Compare error: {}", msg),
            ReconciliationError::PlanError(msg) => write!(f, "Plan error: {}", msg),
            ReconciliationError::ActuateError(msg) => write!(f, "Actuate error: {}", msg),
        }
    }
}

impl std::error::Error for ReconciliationError {}

/// The reconciliation control loop.
///
/// This struct ties together the four phases of reconciliation:
/// - **Sensor**: Observes the current state of the cluster
/// - **Comparator**: Computes differences between current and desired state
/// - **Planner**: Generates an ordered sequence of actions to reconcile
/// - **Actuator**: Executes the planned actions
///
/// # Example
///
/// ```ignore
/// use orchestrator_core::reconciliation::ReconciliationLoop;
///
/// let reconciliation = ReconciliationLoop::new(
///     Box::new(my_sensor),
///     Box::new(my_comparator),
///     Box::new(my_planner),
///     Box::new(my_actuator),
///     4, // concurrency limit
/// );
///
/// // Execute one cycle
/// reconciliation.run_cycle(&desired_state, &scope).await?;
/// ```
pub struct ReconciliationLoop {
    /// Component responsible for sensing current state
    sensor: Box<dyn Sensor>,
    /// Component responsible for comparing states
    comparator: Box<dyn Comparator>,
    /// Component responsible for planning reconciliation
    planner: Box<dyn Planner>,
    /// Component responsible for actuating changes
    actuator: Box<dyn Actuator>,
    /// Concurrency limit for action execution
    concurrency_limit: usize,
}

impl ReconciliationLoop {
    /// Create a new reconciliation loop with the provided components.
    ///
    /// # Arguments
    ///
    /// * `sensor` - Component for observing current state
    /// * `comparator` - Component for computing state differences
    /// * `planner` - Component for generating reconciliation plans
    /// * `actuator` - Component for executing actions
    /// * `concurrency_limit` - Maximum number of actions to execute in parallel
    pub fn new(
        sensor: Box<dyn Sensor>,
        comparator: Box<dyn Comparator>,
        planner: Box<dyn Planner>,
        actuator: Box<dyn Actuator>,
        concurrency_limit: usize,
    ) -> Self {
        Self {
            sensor,
            comparator,
            planner,
            actuator,
            concurrency_limit,
        }
    }

    /// Execute a single reconciliation cycle.
    ///
    /// This method runs through all four phases:
    /// 1. Sense the current state
    /// 2. Compare against desired state
    /// 3. Plan the reconciliation actions
    /// 4. Actuate the plan
    ///
    /// # Arguments
    ///
    /// * `desired_state` - The desired state to reconcile towards
    /// * `scope` - The scope of entities to observe
    ///
    /// # Errors
    ///
    /// Returns an error if any phase fails.
    ///
    /// # Example
    ///
    /// ```ignore
    /// reconciliation_loop.run_cycle(&desired_state, &scope).await?;
    /// ```
    pub async fn run_cycle(
        &self,
        desired_state: &DesiredState,
        scope: &SenseScope,
    ) -> Result<ExecutionOutcome, ReconciliationError> {
        // Phase 1: Sense current state
        let sense_operation = self
            .sensor
            .sense(scope)
            .await
            .map_err(|e| ReconciliationError::SenseError(e.to_string()))?;

        // Convert SenseOperation result to CurrentState for comparison
        let current_state = self.convert_sense_to_current_state(&sense_operation);

        // Phase 2: Compare current vs desired state
        let compare_result = self.comparator.compare(desired_state, &current_state);

        // If no differences, we're done
        if !compare_result.diff.has_changes() {
            return Ok(ExecutionOutcome::NoActionRequired);
        }

        // Phase 3: Plan reconciliation actions
        let plan = self
            .planner
            .plan(&compare_result.diff)
            .await
            .map_err(|e| ReconciliationError::PlanError(e.to_string()))?;

        // Phase 4: Actuate the plan
        let outcome = self
            .actuator
            .execute(&plan, self.concurrency_limit)
            .await
            .map_err(|e| ReconciliationError::ActuateError(e.to_string()))?;

        Ok(outcome)
    }

    /// Convert a SenseOperation result to the CurrentState expected by the Comparator.
    fn convert_sense_to_current_state(&self, sense_op: &SenseOperation) -> CurrentState {
        // Create a new CurrentState from the sense operation
        // This is a simple conversion - in production you'd map the sensed entities
        CurrentState::new(sense_op.correlation_id)
    }

    /// Run the reconciliation loop continuously until stopped.
    ///
    /// This method will run cycles repeatedly with a configurable interval.
    ///
    /// # Arguments
    ///
    /// * `desired_state` - The desired state to reconcile towards
    /// * `scope` - The scope of entities to observe
    /// * `interval` - Duration to wait between cycles
    ///
    /// # Errors
    ///
    /// Returns an error if a cycle fails critically. Some errors may be
    /// logged and the loop continues.
    pub async fn run_continuous(
        &self,
        desired_state: &DesiredState,
        scope: &SenseScope,
        interval: std::time::Duration,
    ) -> Result<(), ReconciliationError> {
        loop {
            match self.run_cycle(desired_state, scope).await {
                Ok(outcome) => {
                    debug!("Reconciliation cycle completed: {:?}", outcome);
                }
                Err(e) => {
                    error!("Reconciliation cycle failed: {}", e);
                    // Optionally return error or continue depending on policy
                    // For now, we continue
                }
            }

            tokio::time::sleep(interval).await;
        }
    }

    /// Get a reference to the sensor component.
    pub fn sensor(&self) -> &dyn Sensor {
        &*self.sensor
    }

    /// Get a reference to the comparator component.
    pub fn comparator(&self) -> &dyn Comparator {
        &*self.comparator
    }

    /// Get a reference to the planner component.
    pub fn planner(&self) -> &dyn Planner {
        &*self.planner
    }

    /// Get a reference to the actuator component.
    pub fn actuator(&self) -> &dyn Actuator {
        &*self.actuator
    }
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;

    // Mock implementations for testing would go here
}
