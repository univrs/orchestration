use async_trait::async_trait;
use orchestrator_shared_types::{
    Node, NodeId, OrchestrationError, Result, WorkloadDefinition, WorkloadInstance,
};
// To get node information
use std::sync::Arc;

pub mod bind;
pub mod filter;
pub mod resources;
pub mod score;
pub mod select;

/// Input for a scheduling decision.
#[derive(Debug, Clone)]
pub struct ScheduleRequest {
    pub workload_definition: Arc<WorkloadDefinition>,
    pub current_instances: Vec<WorkloadInstance>, // Existing instances of this workload
                                                  // Potentially other constraints like anti-affinity, taints/tolerations
}

/// Output of a scheduling decision.
#[derive(Debug, Clone)]
pub enum ScheduleDecision {
    /// Assign the workload instance to the specified node.
    AssignNode(NodeId),
    /// No suitable node found, or workload should not be scheduled right now.
    NoPlacement(String), // Reason for no placement
    /// An error occurred during scheduling.
    Error(String),
}

#[async_trait]
pub trait Scheduler: Send + Sync {
    /// Makes a scheduling decision for a given workload.
    /// This would typically be called when a new workload is created or an existing one needs rescheduling.
    async fn schedule(
        &self,
        request: &ScheduleRequest,
        available_nodes: &[Node], // Current state of schedulable nodes
    ) -> Result<Vec<ScheduleDecision>>; // Returns a decision for each replica to be scheduled

    // Potentially, a method to decide if a workload instance should be preempted
    // async fn decide_preemption(&self, instance: &WorkloadInstance, nodes: &[Node]) -> Result<bool>;
}

// Example of a specific error for this interface
#[derive(Debug, thiserror::Error)]
pub enum SchedulerError {
    #[error("No suitable nodes available for workload {0}: {1}")]
    NoSuitableNodes(String, String), // Workload name, reason
    #[error("Insufficient resources on candidate node {0} for workload {1}")]
    InsufficientResources(NodeId, String),
    #[error("Failed to evaluate placement constraints: {0}")]
    ConstraintEvaluationFailed(String),
}

impl From<SchedulerError> for OrchestrationError {
    fn from(err: SchedulerError) -> Self {
        OrchestrationError::SchedulingError(err.to_string())
    }
}

// A very simple scheduler implementation for demonstration
#[derive(Clone, Copy)]
pub struct SimpleScheduler;

#[async_trait]
impl Scheduler for SimpleScheduler {
    async fn schedule(
        &self,
        request: &ScheduleRequest,
        available_nodes: &[Node],
    ) -> Result<Vec<ScheduleDecision>> {
        let mut decisions = Vec::new();
        let needed_replicas = request
            .workload_definition
            .replicas
            .saturating_sub(request.current_instances.len() as u32);

        if needed_replicas == 0 {
            return Ok(decisions);
        }

        if available_nodes.is_empty() {
            for _ in 0..needed_replicas {
                decisions.push(ScheduleDecision::NoPlacement(
                    "No nodes available".to_string(),
                ));
            }
            return Ok(decisions);
        }

        // Super simple: round-robin or pick first available that meets basic criteria (if any)
        // For now, just pick the first available node for all needed replicas (very naive)
        let mut node_iter = available_nodes.iter().cycle(); // Cycle through nodes

        for _ in 0..needed_replicas {
            if let Some(node) = node_iter.next() {
                // TODO: Actual resource checking against node.resources_allocatable
                // and request.workload_definition.containers[*].resource_requests
                decisions.push(ScheduleDecision::AssignNode(node.id.clone()));
            } else {
                // This case should not be hit if available_nodes is not empty due to cycle()
                // but as a safeguard:
                decisions.push(ScheduleDecision::NoPlacement(
                    "Failed to pick a node (internal error)".to_string(),
                ));
            }
        }
        Ok(decisions)
    }
}
