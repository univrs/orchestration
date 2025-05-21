use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use std::collections::HashMap;

use orchestrator_shared_types::{
    Node, NodeId, OrchestrationError, Result, WorkloadDefinition, WorkloadId, WorkloadInstance,
    WorkloadInstanceStatus,
};
use container_runtime_interface::ContainerRuntime;
use cluster_manager_interface::{ClusterEvent, ClusterManager};
use scheduler_interface::{ScheduleDecision, ScheduleRequest, Scheduler, SimpleScheduler}; // Using SimpleScheduler for now
use tracing::{error, info, warn};

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
                Ok(Some(event)) = cluster_events_rx.recv() => {
                    info!("Received cluster event: {:?}", event);
                    if let Err(e) = self.handle_cluster_event(event).await {
                         error!("Failed to handle cluster event: {:?}", e);
                    }
                    // After a cluster event, might need to re-evaluate workload placements
                    if let Err(e) = self.reconcile_all_workloads().await {
                        error!("Failed during reconciliation after cluster event: {:?}", e);
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
        let mut state = self.state.lock().await;

        let current_instances = state.instances.entry(workload_def.id).or_default();
        let desired_replicas = workload_def.replicas;
        let current_running_replicas = current_instances
            .iter()
            .filter(|inst| inst.status == WorkloadInstanceStatus::Running || inst.status == WorkloadInstanceStatus::Pending)
            .count() as u32;

        info!("Workload {}: Desired replicas: {}, Current running/pending: {}", workload_def.id, desired_replicas, current_running_replicas);

        if current_running_replicas < desired_replicas {
            let num_to_schedule = desired_replicas - current_running_replicas;
            info!("Need to schedule {} new instances for workload {}", num_to_schedule, workload_def.id);

            let available_nodes: Vec<Node> = state.nodes.values()
                .filter(|n| n.status == orchestrator_shared_types::NodeStatus::Ready) // Only schedule on ready nodes
                .cloned()
                .collect();
            
            if available_nodes.is_empty() {
                warn!("No ready nodes available to schedule workload {}", workload_def.id);
                return Ok(()); // Can't schedule if no nodes are ready
            }

            let schedule_request = ScheduleRequest {
                workload_definition: Arc::clone(workload_def),
                current_instances: current_instances.clone(),
            };
            
            // Drop lock before calling scheduler to avoid holding it too long
            drop(state); 
            let decisions = self.scheduler.schedule(&schedule_request, &available_nodes).await?;
            // Re-acquire lock to update state
            let mut state = self.state.lock().await;
            let current_instances = state.instances.entry(workload_def.id).or_default();


            for decision in decisions.into_iter().take(num_to_schedule as usize) {
                match decision {
                    ScheduleDecision::AssignNode(node_id) => {
                        info!("Scheduling new instance of workload {} on node {}", workload_def.id, node_id);
                        // This is where you'd iterate through `workload_def.containers`
                        // and call `self.runtime.create_container` for each.
                        // For simplicity, we'll just create a placeholder instance.
                        // Actual container creation needs more detail (passing container configs, options).
                        
                        // Placeholder: Assume first container is the main one for now
                        if let Some(container_config) = workload_def.containers.first() {
                            let options = container_runtime_interface::CreateContainerOptions {
                                workload_id: workload_def.id,
                                node_id,
                            };
                            // Drop lock before potentially long-running I/O (runtime call)
                            drop(state);
                            match self.runtime.create_container(container_config, &options).await {
                                Ok(container_id) => {
                                    info!("Container {} created for workload {} on node {}", container_id, workload_def.id, node_id);
                                    // Re-acquire lock
                                    state = self.state.lock().await;
                                    let current_instances = state.instances.entry(workload_def.id).or_default();
                                    
                                    let new_instance = WorkloadInstance {
                                        id: Uuid::new_v4(),
                                        workload_id: workload_def.id,
                                        node_id,
                                        container_ids: vec![container_id], // Store the actual container ID
                                        status: WorkloadInstanceStatus::Pending, // Will update based on runtime status later
                                    };
                                    current_instances.push(new_instance);
                                }
                                Err(e) => {
                                    error!("Failed to create container for workload {} on node {}: {:?}", workload_def.id, node_id, e);
                                    // Re-acquire lock if dropped and error occurred
                                    state = self.state.lock().await; 
                                }
                            }
                        } else {
                            warn!("Workload {} has no container definitions, cannot schedule.", workload_def.id);
                        }
                    }
                    ScheduleDecision::NoPlacement(reason) => {
                        warn!("Could not place instance of workload {}: {}", workload_def.id, reason);
                    }
                    ScheduleDecision::Error(err_msg) => {
                        error!("Scheduler error for workload {}: {}", workload_def.id, err_msg);
                    }
                }
            }

        } else if current_running_replicas > desired_replicas {
            let num_to_remove = current_running_replicas - desired_replicas;
            info!("Need to remove {} instances of workload {}", num_to_remove, workload_def.id);
            // TODO: Implement logic to select and remove surplus instances.
            // This would involve:
            // 1. Selecting which instances to terminate (e.g., oldest, least healthy).
            // 2. Iterating through their `container_ids`.
            // 3. Calling `self.runtime.stop_container()` and `self.runtime.remove_container()`.
            // 4. Updating the `WorkloadInstanceStatus` to Terminating, then removing from `state.instances`.
            warn!("Workload scale-down (removing {} instances) not yet implemented for workload {}", num_to_remove, workload_def.id);
        }

        // TODO: Implement status checking for existing instances.
        // Iterate through `current_instances`, call `self.runtime.get_container_status()` for each container,
        // and update `WorkloadInstanceStatus` accordingly. If an instance failed, it might need rescheduling.
        
        Ok(())
    }
}

// This would typically be in a `main.rs` file if `orchestrator_core` was a binary crate.
// For now, let's imagine a function that sets it up.
pub async fn start_orchestrator_service(
    runtime: Arc<dyn ContainerRuntime>,
    cluster_manager: Arc<dyn ClusterManager>,
    scheduler: Arc<dyn Scheduler>,
) -> Result<mpsc::Sender<WorkloadDefinition>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env().add_directive("info".parse().unwrap()))
        .init();

    let mut orchestrator = Orchestrator::new(runtime, cluster_manager, scheduler);
    let workload_tx = orchestrator.get_workload_sender();

    tokio::spawn(async move {
        if let Err(e) = orchestrator.run().await {
            error!("Orchestrator service exited with error: {:?}", e);
        }
    });

    Ok(workload_tx)
}