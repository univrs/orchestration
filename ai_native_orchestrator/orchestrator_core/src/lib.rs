use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use std::collections::HashMap;
use uuid;

use orchestrator_shared_types::{
    Node, NodeId, Result, WorkloadDefinition, WorkloadId, WorkloadInstance,
    WorkloadInstanceStatus,
};
use container_runtime_interface::ContainerRuntime;
use cluster_manager_interface::{ClusterEvent, ClusterManager};
use scheduler_interface::{ScheduleDecision, ScheduleRequest, Scheduler}; // Using SimpleScheduler for now
use tracing::{error, info, warn, trace};

// In-memory state for now. This would be replaced by a persistent store (e.g., etcd, TiKV wrapper).
#[derive(Default)]
struct OrchestratorState {
    nodes: HashMap<NodeId, Node>,
    workloads: HashMap<WorkloadId, Arc<WorkloadDefinition>>,
    instances: HashMap<WorkloadId, Vec<WorkloadInstance>>, // Instances per workload
}

pub struct Orchestrator {
    state: Arc<Mutex<OrchestratorState>>,
    runtime: Arc<dyn ContainerRuntime>,
    cluster_manager: Arc<dyn ClusterManager>,
    scheduler: Arc<dyn Scheduler>,
    // Channel for submitting new workloads or updates
    workload_tx: mpsc::Sender<WorkloadDefinition>,
    workload_rx: mpsc::Receiver<WorkloadDefinition>,
}

impl Orchestrator {
    pub fn new(
        runtime: Arc<dyn ContainerRuntime>,
        cluster_manager: Arc<dyn ClusterManager>,
        scheduler: Arc<dyn Scheduler>,
    ) -> Self {
        let (workload_tx, workload_rx) = mpsc::channel(100); // Buffer size 100
        Orchestrator {
            state: Arc::new(Mutex::new(OrchestratorState::default())),
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
                // Listen for cluster events (node changes)
                
                Ok(()) = cluster_events_rx.changed() => {
                    // create a new scope to ensure watch::Ref is dropperd before awaits
                    let cloned_event_option: Option<ClusterEvent> = {

                        let event_ref = cluster_events_rx.borrow_and_update();
                        (*event_ref).clone()
                    }; // event_ref (the watch::Ref) is dropped here 


                    if let Some(owned_event) = cloned_event_option {
                        info!("Received cluster event: {:?}", owned_event);
                        // Now we are only working with the `owned event`, which is `CluterEvent` (Send + Clone)

                        if let Err(e) = self.handle_cluster_event(owned_event).await {
                            error!("Failed to handle cluster event: {:?}", e);
                        }

                        if let Err(e) = self.reconcile_all_workloads().await {
                            error!("Failed during reconciliation after cluster event: {:?}", e);
                        }

                   } else {

                        trace!("Cluster event Channerl updated to None or was initialized to None");
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
                else => {
                    warn!("A channel closed or select! branch completed unexpectedly. Orchestrator might be shutting down.");
                    break;
                }
            }
        }
        info!("Orchestrator shutting down.");
        Ok(())
    }

    async fn handle_workload_update(&self, workload_def: WorkloadDefinition) -> Result<()> {
        let workload_id = workload_def.id;
        let workload_def_arc = Arc::new(workload_def);
        {
            let mut state = self.state.lock().await;
            state.workloads.insert(workload_id, workload_def_arc.clone());
            state.instances.entry(workload_id).or_insert_with(Vec::new); // Ensure entry exists
        }
        info!("Workload {} registered. Triggering reconciliation.", workload_id);
        self.reconcile_workload(&workload_def_arc).await
    }
    
    async fn handle_cluster_event(&self, event: ClusterEvent) -> Result<()> {
        let mut state = self.state.lock().await;
        match event {
            ClusterEvent::NodeAdded(node) | ClusterEvent::NodeUpdated(node) => {
                info!("Node {} added/updated.", node.id);
                // Initialize the runtime on the new node if it's newly added and ready
                if node.status == orchestrator_shared_types::NodeStatus::Ready && !state.nodes.contains_key(&node.id) {
                     match self.runtime.init_node(node.id).await {
                        Ok(_) => info!("Runtime initialized on node {}", node.id),
                        Err(e) => error!("Failed to initialize runtime on node {}: {:?}", node.id, e),
                     }
                }
                state.nodes.insert(node.id, node);
            }
            ClusterEvent::NodeRemoved(node_id) => {
                info!("Node {} removed.", node_id);
                state.nodes.remove(&node_id);
                // TODO: Handle workloads running on the removed node (reschedule them)
            }
        }
        Ok(())
    }

    async fn reconcile_all_workloads(&self) -> Result<()> {
        info!("Reconciling all workloads...");
        let workloads_to_reconcile = {
            let state = self.state.lock().await;
            state.workloads.values().cloned().collect::<Vec<_>>()
        };

        for workload_def in workloads_to_reconcile {
            if let Err(e) = self.reconcile_workload(&workload_def).await {
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
        // This scope controls the lifetime of the state lock for reading.
        let action = {
            let mut state = self.state.lock().await;
            let instances_vec_mut_ref = state.instances.entry(workload_def.id).or_default();
            
            // Clone for calculations and for passing to scheduler if needed,
            // releasing the direct borrow from instances_vec_mut_ref sooner.
            let current_instances_clone: Vec<WorkloadInstance> = instances_vec_mut_ref.clone();

            let desired_replicas = workload_def.replicas;
            let current_active_replicas = current_instances_clone // Use the clone
                .iter()
                .filter(|inst| {
                    matches!(inst.status, WorkloadInstanceStatus::Running | WorkloadInstanceStatus::Pending)
                })
                .count() as u32;

            info!(
                "Workload {}: Desired replicas: {}, Current active (running/pending): {}",
                workload_def.id, desired_replicas, current_active_replicas
            );

            if current_active_replicas < desired_replicas {
                let num_to_schedule = desired_replicas - current_active_replicas;
                let available_nodes_for_scheduling: Vec<Node> = state
                    .nodes
                    .values()
                    .filter(|n| n.status == orchestrator_shared_types::NodeStatus::Ready)
                    .cloned()
                    .collect();

                if available_nodes_for_scheduling.is_empty() {
                    warn!(
                        "No ready nodes available to schedule {} new instances for workload {}",
                        num_to_schedule, workload_def.id
                    );
                    WorkloadAction::None // Or a specific "NoNodes" action
                } else {
                    WorkloadAction::ScheduleNew {
                        num_to_schedule,
                        current_instances_state: current_instances_clone, // Pass the clone
                        available_nodes: available_nodes_for_scheduling,
                    }
                }
            } else if current_active_replicas > desired_replicas {
                let num_to_remove = current_active_replicas - desired_replicas;
                // TODO: Select which specific instances to remove
                let instances_to_remove = current_instances_clone
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
                     WorkloadAction::None // Should not happen if num_to_remove > 0 and instances exist
                }
            } else {
                WorkloadAction::None // Already reconciled
            }
        }; // MutexGuard for `state` is dropped here

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

                // Re-acquire lock to update state with new instances
                let mut state = self.state.lock().await;
                let instances_vec_mut_ref = state.instances.entry(workload_def.id).or_default();

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
                                
                                // Temporarily drop lock for runtime call if it's long or might re-enter
                                // For now, assuming runtime.create_container is reasonably fast or we manage locks carefully.
                                // If it's very long, you'd drop `state` guard, call runtime, then re-acquire.
                                // This example keeps it simple if `create_container` is not expected to deadlock or be too slow.
                                match self.runtime.create_container(container_config, &options).await {
                                    Ok(container_id) => {
                                        info!(
                                            "Container {} created for workload {} on node {}",
                                            container_id, workload_def.id, node_id
                                        );
                                        let new_instance = WorkloadInstance {
                                            id: uuid::Uuid::new_v4(),
                                            workload_id: workload_def.id,
                                            node_id,
                                            container_ids: vec![container_id],
                                            status: WorkloadInstanceStatus::Pending,
                                        };
                                        instances_vec_mut_ref.push(new_instance);
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
                    for container_id in &instance_to_remove.container_ids {
                        // Drop state lock before long I/O for stopping/removing container
                        // This part needs careful lock management if runtime calls are slow or might re-enter.
                        // For simplicity now, we'll assume it's acceptable or runtime calls are quick.
                        match self.runtime.stop_container(container_id).await {
                            Ok(_) => info!("Stopped container {}", container_id),
                            Err(e) => error!("Failed to stop container {}: {:?}", container_id, e),
                        }
                        match self.runtime.remove_container(container_id).await {
                            Ok(_) => info!("Removed container {}", container_id),
                            Err(e) => error!("Failed to remove container {}: {:?}", container_id, e),
                        }
                    }
                    // Re-acquire lock to update state
                    let mut state = self.state.lock().await;
                    let instances_vec_mut_ref = state.instances.entry(workload_def.id).or_default();
                    instances_vec_mut_ref.retain(|inst| inst.id != instance_to_remove.id);
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
    runtime: Arc<dyn ContainerRuntime>,
    cluster_manager: Arc<dyn ClusterManager>,
    scheduler: Arc<dyn Scheduler>,
) -> Result<mpsc::Sender<WorkloadDefinition>> {
    //use tracing_subscriber::{fmt, EnvFilter};
    // Initialize logging
    //tracing_subscriber::fmt()
    //    .with_env_filter(tracing_subscriber::EnvFilter::from_default_env().add_directive("info".parse().unwrap()))
    //    .init();

    let mut orchestrator = Orchestrator::new(runtime, cluster_manager, scheduler);
    let workload_tx = orchestrator.get_workload_sender();

    tokio::spawn(async move {
        if let Err(e) = orchestrator.run().await {
            error!("Orchestrator service exited with error: {:?}", e);
        }
    });

    Ok(workload_tx)
}
