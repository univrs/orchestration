#[cfg(feature = "rest-api")]
pub mod api;

pub mod reconciliation;

use std::sync::Arc;
use tokio::sync::mpsc;
use uuid;

use orchestrator_shared_types::{
    Node, Result, WorkloadDefinition, WorkloadInstance,
    WorkloadInstanceStatus,
};
use container_runtime_interface::ContainerRuntime;
use cluster_manager_interface::{ClusterEvent, ClusterManager};
use scheduler_interface::{ScheduleDecision, ScheduleRequest, Scheduler};
use state_store_interface::StateStore;
use tracing::{error, info, warn};

pub struct Orchestrator {
    state_store: Arc<dyn StateStore>,
    runtime: Arc<dyn ContainerRuntime>,
    cluster_manager: Arc<dyn ClusterManager>,
    scheduler: Arc<dyn Scheduler>,
    // Channel for submitting new workloads or updates
    workload_tx: mpsc::Sender<WorkloadDefinition>,
    workload_rx: mpsc::Receiver<WorkloadDefinition>,
}

impl Orchestrator {
    pub fn new(
        state_store: Arc<dyn StateStore>,
        runtime: Arc<dyn ContainerRuntime>,
        cluster_manager: Arc<dyn ClusterManager>,
        scheduler: Arc<dyn Scheduler>,
    ) -> Self {
        let (workload_tx, workload_rx) = mpsc::channel(100); // Buffer size 100
        Orchestrator {
            state_store,
            runtime,
            cluster_manager,
            scheduler,
            workload_tx,
            workload_rx,
        }
    }

    pub fn get_workload_sender(&self) -> mpsc::Sender<WorkloadDefinition> {
        self.workload_tx.clone()
    }

    pub async fn run(&mut self) -> Result<()> {
        info!("Orchestrator starting...");

        self.cluster_manager.initialize().await?;
        let mut cluster_events_rx = self.cluster_manager.subscribe_to_events().await?;
        
        info!("Cluster manager initialized and subscribed to events.");

        loop {
            tokio::select! {
                // Listen for new/updated workload definitions
                Some(workload_def) = self.workload_rx.recv() => {
                    info!("Received workload definition: {} ({})", workload_def.name, workload_def.id);
                    if let Err(e) = self.handle_workload_update(workload_def).await {
                        error!("Failed to handle workload update: {:?}", e);
                    }
                }
                // Listen for cluster events (node changes) - uses broadcast channel
                // Broadcast channel ensures ALL events are received (no lost events)
                result = cluster_events_rx.recv() => {
                    match result {
                        Ok(event) => {
                            info!("Received cluster event: {:?}", event);
                            if let Err(e) = self.handle_cluster_event(event).await {
                                error!("Failed to handle cluster event: {:?}", e);
                            }
                            if let Err(e) = self.reconcile_all_workloads().await {
                                error!("Failed during reconciliation after cluster event: {:?}", e);
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                            warn!("Cluster event receiver lagged, missed {} events", n);
                            // Continue processing - we'll catch up on next event
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                            warn!("Cluster event channel closed");
                            break;
                        }
                    }
                }
                // TODO: Periodic reconciliation loop (e.g., every 30 seconds)
                // This would ensure desired state matches actual state.
                // _ = tokio::time::sleep(tokio::time::Duration::from_secs(30)) => {
                //    info!("Periodic reconciliation triggered.");
                //    if let Err(e) = self.reconcile_all_workloads().await {
                //        error!("Failed during periodic reconciliation: {:?}", e);
                //    }
                // }
            }
        }
        info!("Orchestrator shutting down.");
        Ok(())
    }

    async fn handle_workload_update(&self, workload_def: WorkloadDefinition) -> Result<()> {
        let workload_id = workload_def.id;

        // Store workload in persistent state
        self.state_store.put_workload(workload_def.clone()).await?;

        info!("Workload {} registered. Triggering reconciliation.", workload_id);
        self.reconcile_workload(&Arc::new(workload_def)).await
    }
    
    async fn handle_cluster_event(&self, event: ClusterEvent) -> Result<()> {
        match event {
            ClusterEvent::NodeAdded(node) | ClusterEvent::NodeUpdated(node) => {
                info!("Node {} added/updated.", node.id);

                // Check if node already exists
                let existing = self.state_store.get_node(&node.id).await?;

                // Initialize the runtime on the new node if it's newly added and ready
                if node.status == orchestrator_shared_types::NodeStatus::Ready && existing.is_none() {
                     match self.runtime.init_node(node.id).await {
                        Ok(_) => info!("Runtime initialized on node {}", node.id),
                        Err(e) => error!("Failed to initialize runtime on node {}: {:?}", node.id, e),
                     }
                }

                // Store node in persistent state
                self.state_store.put_node(node).await?;
            }
            ClusterEvent::NodeRemoved(node_id) => {
                info!("Node {} removed.", node_id);

                // Remove node from persistent state
                self.state_store.delete_node(&node_id).await?;

                // TODO: Handle workloads running on the removed node (reschedule them)
            }
        }
        Ok(())
    }

    async fn reconcile_all_workloads(&self) -> Result<()> {
        info!("Reconciling all workloads...");

        // Get all workloads from persistent state
        let workloads = self.state_store.list_workloads().await?;

        for workload_def in workloads {
            if let Err(e) = self.reconcile_workload(&Arc::new(workload_def.clone())).await {
                error!("Failed to reconcile workload {}: {:?}", workload_def.id, e);
                // Continue to next workload
            }
        }
        info!("Finished reconciling all workloads.");
        Ok(())
    }


    async fn reconcile_workload(&self, workload_def: &Arc<WorkloadDefinition>) -> Result<()> {
        info!("Reconciling workload: {} ({})", workload_def.name, workload_def.id);

        // Phase 1: Determine what needs to be done (read state, decide action)
        // Get current instances from persistent state
        let current_instances = self.state_store
            .list_instances_for_workload(&workload_def.id)
            .await?;

        let desired_replicas = workload_def.replicas;
        let current_active_replicas = current_instances
            .iter()
            .filter(|inst| {
                matches!(inst.status, WorkloadInstanceStatus::Running | WorkloadInstanceStatus::Pending)
            })
            .count() as u32;

        info!(
            "Workload {}: Desired replicas: {}, Current active (running/pending): {}",
            workload_def.id, desired_replicas, current_active_replicas
        );

        let action = if current_active_replicas < desired_replicas {
            let num_to_schedule = desired_replicas - current_active_replicas;

            // Get available nodes from persistent state
            let all_nodes = self.state_store.list_nodes().await?;
            let available_nodes_for_scheduling: Vec<Node> = all_nodes
                .into_iter()
                .filter(|n| n.status == orchestrator_shared_types::NodeStatus::Ready)
                .collect();

            if available_nodes_for_scheduling.is_empty() {
                warn!(
                    "No ready nodes available to schedule {} new instances for workload {}",
                    num_to_schedule, workload_def.id
                );
                WorkloadAction::None
            } else {
                WorkloadAction::ScheduleNew {
                    num_to_schedule,
                    current_instances_state: current_instances.clone(),
                    available_nodes: available_nodes_for_scheduling,
                }
            }
        } else if current_active_replicas > desired_replicas {
            let num_to_remove = current_active_replicas - desired_replicas;

            // Select which specific instances to remove
            let instances_to_remove = current_instances
                .iter()
                .filter(|inst| {
                    matches!(inst.status, WorkloadInstanceStatus::Running | WorkloadInstanceStatus::Pending)
                })
                .take(num_to_remove as usize)
                .cloned()
                .collect::<Vec<_>>();

            if !instances_to_remove.is_empty() {
                 WorkloadAction::RemoveInstances { instances_to_remove }
            } else {
                 WorkloadAction::None
            }
        } else {
            WorkloadAction::None // Already reconciled
        };

        // Phase 2: Execute the action (potentially involves I/O, no state lock initially)
        match action {
            WorkloadAction::ScheduleNew {
                num_to_schedule,
                current_instances_state,
                available_nodes,
            } => {
                info!(
                    "Need to schedule {} new instances for workload {}",
                    num_to_schedule, workload_def.id
                );
                let schedule_request = ScheduleRequest {
                    workload_definition: Arc::clone(workload_def),
                    current_instances: current_instances_state, // Use the owned clone
                };

                let decisions = self
                    .scheduler
                    .schedule(&schedule_request, &available_nodes)
                    .await?;

                for decision in decisions.into_iter().take(num_to_schedule as usize) {
                    match decision {
                        ScheduleDecision::AssignNode(node_id) => {
                            info!(
                                "Scheduler assigned workload {} instance to node {}",
                                workload_def.id, node_id
                            );
                            if let Some(container_config) = workload_def.containers.first() {
                                let options = container_runtime_interface::CreateContainerOptions {
                                    workload_id: workload_def.id,
                                    node_id,
                                };

                                match self.runtime.create_container(container_config, &options).await {
                                    Ok(container_id) => {
                                        info!(
                                            "Container {} created for workload {} on node {}",
                                            container_id, workload_def.id, node_id
                                        );

                                        // Create new instance and save to persistent state
                                        let new_instance = WorkloadInstance {
                                            id: uuid::Uuid::new_v4(),
                                            workload_id: workload_def.id,
                                            node_id,
                                            container_ids: vec![container_id],
                                            status: WorkloadInstanceStatus::Pending,
                                        };

                                        if let Err(e) = self.state_store.put_instance(new_instance).await {
                                            error!("Failed to store instance in state: {:?}", e);
                                        }
                                    }
                                    Err(e) => {
                                        error!(
                                            "Failed to create container for workload {} on node {}: {:?}",
                                            workload_def.id, node_id, e
                                        );
                                    }
                                }
                            } else {
                                warn!(
                                    "Workload {} has no container definitions, cannot schedule instance.",
                                    workload_def.id
                                );
                            }
                        }
                        ScheduleDecision::NoPlacement(reason) => {
                            warn!(
                                "Could not place instance of workload {}: {}",
                                workload_def.id, reason
                            );
                        }
                        ScheduleDecision::Error(err_msg) => {
                            error!(
                                "Scheduler error for workload {}: {}",
                                workload_def.id, err_msg
                            );
                        }
                    }
                }
            }
            WorkloadAction::RemoveInstances { instances_to_remove } => {
                info!("Need to remove {} instances for workload {}", instances_to_remove.len(), workload_def.id);
                for instance_to_remove in instances_to_remove {
                    info!("Attempting to remove instance {} (containers: {:?}) of workload {}", instance_to_remove.id, instance_to_remove.container_ids, workload_def.id);

                    // Stop and remove containers
                    for container_id in &instance_to_remove.container_ids {
                        match self.runtime.stop_container(container_id).await {
                            Ok(_) => info!("Stopped container {}", container_id),
                            Err(e) => error!("Failed to stop container {}: {:?}", container_id, e),
                        }
                        match self.runtime.remove_container(container_id).await {
                            Ok(_) => info!("Removed container {}", container_id),
                            Err(e) => error!("Failed to remove container {}: {:?}", container_id, e),
                        }
                    }

                    // Remove instance from persistent state
                    let instance_id = instance_to_remove.id.to_string();
                    if let Err(e) = self.state_store.delete_instance(&instance_id).await {
                        error!("Failed to delete instance {} from state: {:?}", instance_id, e);
                    }
                }
            }
            WorkloadAction::None => {
                // Nothing to do for this workload
                info!("Workload {} is already reconciled or no action needed.", workload_def.id);
            }
        }
        
        // TODO: Status checking loop for existing instances (could be part of reconcile or separate)
        // This would iterate through instances, get their status from the runtime,
        // and update the WorkloadInstanceStatus in our state.
        // If an instance failed, it might trigger rescheduling by changing current_active_replicas.

        Ok(())
    }
}

// Helper enum for clarity in reconcile_workload
#[derive(Debug)]
enum WorkloadAction {
    ScheduleNew {
        num_to_schedule: u32,
        current_instances_state: Vec<WorkloadInstance>, // Owned clone of instances
        available_nodes: Vec<Node>,                     // Owned clone of nodes
    },
    RemoveInstances {
        instances_to_remove: Vec<WorkloadInstance>,
    },
    None,
}

// This would typically be in a `main.rs` file if `orchestrator_core` was a binary crate.
// For now, let's imagine a function that sets it up.
pub async fn start_orchestrator_service(
    state_store: Arc<dyn StateStore>,
    runtime: Arc<dyn ContainerRuntime>,
    cluster_manager: Arc<dyn ClusterManager>,
    scheduler: Arc<dyn Scheduler>,
) -> Result<mpsc::Sender<WorkloadDefinition>> {
    // Initialize state store
    state_store.initialize().await?;

    let mut orchestrator = Orchestrator::new(state_store, runtime, cluster_manager, scheduler);
    let workload_tx = orchestrator.get_workload_sender();

    tokio::spawn(async move {
        if let Err(e) = orchestrator.run().await {
            error!("Orchestrator service exited with error: {:?}", e);
        }
    });

    Ok(workload_tx)
}
