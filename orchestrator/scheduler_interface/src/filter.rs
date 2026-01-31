//! Filter module for the scheduling.filter trait implementation.
//!
//! This module defines the filtering phase of the scheduling pipeline, which eliminates
//! nodes that cannot satisfy the basic requirements of a workload. The filter phase is
//! the first critical step in the scheduling process, reducing the search space for
//! subsequent scoring and selection phases.
//!
//! # Filter Predicates
//!
//! Filter predicates represent constraints that nodes must satisfy to be considered
//! eligible for workload placement:
//!
//! - **Node Selectors**: Simple key-value label matching for basic node selection
//! - **Node Affinity**: Advanced selection rules with required and preferred constraints
//! - **Pod Affinity/Anti-Affinity**: Constraints based on other pods' placement
//! - **Taints and Tolerations**: Node specialization and workload isolation mechanism
//! - **Topology Constraints**: Pod distribution across failure domains
//!
//! # Hard vs Soft Constraints
//!
//! The filter phase primarily enforces hard constraints - requirements that MUST be
//! satisfied for a node to be eligible. Soft constraints (preferences with weights)
//! are evaluated during the scoring phase.
//!
//! # Performance Characteristics
//!
//! Filter predicates are deterministic, parallelizable, and stateless. This design
//! enables safe concurrent evaluation of nodes without race conditions, crucial for
//! performance in large clusters (1000+ nodes).

use orchestrator_shared_types::{Node, NodeId, NodeResources, WorkloadDefinition};
use std::collections::HashMap;

/// Represents a Pod for scheduling purposes.
///
/// This is a simplified representation that can be extended as needed.
/// In production, this would map to the actual pod specification.
#[derive(Debug, Clone, PartialEq)]
pub struct Pod {
    /// Pod name
    pub name: String,
    /// Pod labels for affinity/anti-affinity matching
    pub labels: HashMap<String, String>,
    /// Resource requirements (sum of all container requests)
    pub resources: NodeResources,
    /// Filter predicates that apply to this pod
    pub predicates: FilterPredicates,
}

impl From<&WorkloadDefinition> for Pod {
    fn from(workload: &WorkloadDefinition) -> Self {
        // Aggregate resource requests from all containers
        let mut total_resources = NodeResources::default();
        for container in &workload.containers {
            total_resources.cpu_cores += container.resource_requests.cpu_cores;
            total_resources.memory_mb += container.resource_requests.memory_mb;
            total_resources.disk_mb += container.resource_requests.disk_mb;
        }

        Pod {
            name: workload.name.clone(),
            labels: workload.labels.clone(),
            resources: total_resources,
            predicates: FilterPredicates::default(),
        }
    }
}

/// Collection of filter predicates that nodes must satisfy.
///
/// Filter predicates are evaluated during the filter phase to determine which nodes
/// are eligible to host a workload. All predicates must be satisfied for a node to
/// pass filtering.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FilterPredicates {
    /// Simple label-based node selection (required)
    pub node_selector: Option<NodeSelector>,
    /// Advanced node affinity rules (optional)
    pub node_affinity: Option<NodeAffinity>,
    /// Pod co-location preferences (optional)
    pub pod_affinity: Option<PodAffinity>,
    /// Pod separation requirements (optional)
    pub pod_anti_affinity: Option<PodAntiAffinity>,
    /// Taint tolerations for specialized nodes (optional)
    pub tolerations: Vec<Toleration>,
    /// Topology spread constraints (optional)
    pub topology_constraints: Vec<TopologyConstraint>,
}

/// Simple key-value label matching for node selection.
///
/// A pod with node selector labels will only run on nodes that have all the
/// specified labels with matching values.
#[derive(Debug, Clone, PartialEq)]
pub struct NodeSelector {
    /// Label requirements (key-value pairs that must match)
    pub label_requirements: HashMap<String, String>,
    /// Field requirements (node field selectors like metadata.name)
    pub field_requirements: HashMap<String, String>,
}

/// Advanced node selection rules extending node selectors.
///
/// Node affinity supports both required rules (hard constraints) and preferred rules
/// (soft constraints with weights for scoring).
#[derive(Debug, Clone, PartialEq)]
pub struct NodeAffinity {
    /// Required affinity terms (hard constraints - all must match)
    pub required_terms: Vec<AffinityTerm>,
    /// Preferred affinity terms (soft constraints with weights for scoring)
    pub preferred_terms: Vec<WeightedAffinityTerm>,
}

/// Pod affinity rules for co-location preferences.
///
/// Encourages pods to be scheduled on the same node or in the same topology domain
/// as other pods matching specific labels.
#[derive(Debug, Clone, PartialEq)]
pub struct PodAffinity {
    /// Required co-location rules (hard constraints)
    pub required_terms: Vec<PodAffinityTerm>,
    /// Preferred co-location rules (soft constraints)
    pub preferred_terms: Vec<WeightedPodAffinityTerm>,
}

/// Pod anti-affinity rules for separation requirements.
///
/// Enforces pod separation across nodes or topology domains to ensure high
/// availability and fault tolerance.
#[derive(Debug, Clone, PartialEq)]
pub struct PodAntiAffinity {
    /// Required separation rules (hard constraints)
    pub required_terms: Vec<PodAffinityTerm>,
    /// Preferred separation rules (soft constraints)
    pub preferred_terms: Vec<WeightedPodAffinityTerm>,
}

/// A single affinity term with match expressions and fields.
///
/// Affinity terms define rules for matching nodes based on labels and fields.
#[derive(Debug, Clone, PartialEq)]
pub struct AffinityTerm {
    /// Label-based match expressions
    pub match_expressions: Vec<MatchExpression>,
    /// Field-based match expressions
    pub match_fields: Vec<MatchExpression>,
}

/// Weighted affinity term for preferred (soft) constraints.
///
/// The weight determines the importance of this preference during scoring.
#[derive(Debug, Clone, PartialEq)]
pub struct WeightedAffinityTerm {
    /// The affinity term to evaluate
    pub term: AffinityTerm,
    /// Weight (1-100) for scoring this preference
    pub weight: i32,
}

/// Pod affinity term for matching pods based on labels and topology.
#[derive(Debug, Clone, PartialEq)]
pub struct PodAffinityTerm {
    /// Label selector for matching pods
    pub label_selector: HashMap<String, String>,
    /// Topology key (e.g., "kubernetes.io/hostname", "topology.kubernetes.io/zone")
    pub topology_key: String,
    /// Namespaces to consider (empty means same namespace as current pod)
    pub namespaces: Vec<String>,
}

/// Weighted pod affinity term for preferred constraints.
#[derive(Debug, Clone, PartialEq)]
pub struct WeightedPodAffinityTerm {
    /// The pod affinity term to evaluate
    pub term: PodAffinityTerm,
    /// Weight (1-100) for scoring this preference
    pub weight: i32,
}

/// A match expression for label or field matching.
///
/// Match expressions support various operators for flexible matching logic.
#[derive(Debug, Clone, PartialEq)]
pub struct MatchExpression {
    /// The key to match (label key or field name)
    pub key: String,
    /// The operator for matching
    pub operator: MatchOperator,
    /// Values to match against (interpretation depends on operator)
    pub values: Vec<String>,
}

/// Operators for match expressions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchOperator {
    /// Key equals value (exact match)
    In,
    /// Key does not equal value
    NotIn,
    /// Key exists (value is ignored)
    Exists,
    /// Key does not exist (value is ignored)
    DoesNotExist,
    /// Key value is greater than specified value
    Gt,
    /// Key value is less than specified value
    Lt,
}

/// A toleration allows a pod to schedule on nodes with matching taints.
///
/// Tolerations enable pods to "tolerate" node taints, allowing them to be scheduled
/// on nodes that would otherwise reject them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Toleration {
    /// The taint key that this toleration matches
    pub key: String,
    /// The operator for matching the taint
    pub operator: TolerationOperator,
    /// The taint value to match (used with Equal operator)
    pub value: Option<String>,
    /// The taint effect to tolerate
    pub effect: Option<TaintEffect>,
    /// Toleration duration in seconds (for NoExecute effect)
    pub toleration_seconds: Option<i64>,
}

/// Operators for toleration matching.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TolerationOperator {
    /// Toleration matches if taint key and value are equal
    Equal,
    /// Toleration matches if taint key exists (value is ignored)
    Exists,
}

/// A taint marks a node as unsuitable for certain workloads.
///
/// Taints enable node specialization by preventing pods from scheduling unless
/// they have matching tolerations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Taint {
    /// The taint key (e.g., "node-role.kubernetes.io/master")
    pub key: String,
    /// The taint value
    pub value: String,
    /// The taint effect
    pub effect: TaintEffect,
}

/// Effects that a taint can have on pod scheduling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaintEffect {
    /// Pods without matching toleration will not be scheduled (hard constraint)
    NoSchedule,
    /// Scheduler prefers not to schedule pods without matching toleration (soft constraint)
    PreferNoSchedule,
    /// Pods without matching toleration will not be scheduled, and existing pods will be evicted
    NoExecute,
}

/// Topology spread constraint for pod distribution across failure domains.
///
/// Ensures pods are evenly distributed across topology domains (zones, hosts, etc.)
/// for high availability and fault tolerance.
#[derive(Debug, Clone, PartialEq)]
pub struct TopologyConstraint {
    /// Maximum skew between topology domains
    pub max_skew: i32,
    /// Topology key (e.g., "topology.kubernetes.io/zone")
    pub topology_key: String,
    /// How to handle unsatisfiable constraints
    pub when_unsatisfiable: UnsatisfiableConstraintAction,
    /// Label selector for grouping pods
    pub label_selector: HashMap<String, String>,
}

/// Actions to take when a topology constraint cannot be satisfied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnsatisfiableConstraintAction {
    /// Do not schedule the pod (hard constraint)
    DoNotSchedule,
    /// Try to schedule anyway (soft constraint)
    ScheduleAnyway,
}

/// Reason why a node was rejected during filtering.
#[derive(Debug, Clone, PartialEq)]
pub enum RejectionReason {
    /// Insufficient CPU capacity
    InsufficientCpu { requested: f32, available: f32 },
    /// Insufficient memory capacity
    InsufficientMemory { requested: u64, available: u64 },
    /// Insufficient disk capacity
    InsufficientDisk { requested: u64, available: u64 },
    /// Node selector did not match
    NodeSelectorMismatch { details: String },
    /// Node affinity requirement not satisfied
    NodeAffinityMismatch { details: String },
    /// Taint without matching toleration
    TaintWithoutToleration { taint_key: String },
    /// Pod affinity constraint violated
    PodAffinityViolation { details: String },
    /// Pod anti-affinity constraint violated
    PodAntiAffinityViolation { details: String },
    /// Topology spread constraint violated
    TopologySpreadViolation { details: String },
    /// Node is not ready
    NodeNotReady,
    /// Custom rejection reason
    Custom { reason: String },
}

/// Result of the filter phase containing eligible and rejected nodes.
///
/// The filter result provides comprehensive information about which nodes passed
/// filtering and why rejected nodes failed, enabling debugging and optimization.
#[derive(Debug, Clone)]
pub struct FilterResult {
    /// Nodes that passed all filter predicates and are eligible for scheduling
    pub eligible_nodes: Vec<NodeId>,
    /// Nodes that were filtered out with their rejection reasons
    pub filtered_nodes: Vec<(NodeId, RejectionReason)>,
    /// Reasons why eligible nodes were admitted (for debugging/auditing)
    pub admission_reasons: HashMap<NodeId, Vec<String>>,
}

/// The Filter trait defines the filtering phase of the scheduling pipeline.
///
/// Filters are deterministic, parallelizable, and stateless, enabling efficient
/// concurrent evaluation of nodes against scheduling constraints.
///
/// # Examples
///
/// ```rust,ignore
/// use scheduler_interface::filter::{Filter, Pod, FilterResult};
///
/// struct MyFilter;
///
/// impl Filter for MyFilter {
///     fn filter(&self, pod: &Pod, nodes: &[Node]) -> FilterResult {
///         // Evaluate each node against filter predicates
///         // Return eligible nodes and rejection reasons
///         unimplemented!()
///     }
/// }
/// ```
pub trait Filter: Send + Sync {
    /// Filters nodes based on pod requirements and returns eligible nodes.
    ///
    /// This method evaluates each node against the pod's filter predicates to determine
    /// which nodes are capable of hosting the workload. The filter phase eliminates
    /// infeasible nodes before the scoring phase.
    ///
    /// # Arguments
    ///
    /// * `pod` - The pod to be scheduled with its filter predicates
    /// * `nodes` - The pool of available nodes to evaluate
    ///
    /// # Returns
    ///
    /// A `FilterResult` containing:
    /// - `eligible_nodes`: Nodes that passed all filter predicates
    /// - `filtered_nodes`: Nodes that failed with their rejection reasons
    /// - `admission_reasons`: Why each eligible node was admitted
    ///
    /// # Guarantees
    ///
    /// - **Deterministic**: Same inputs always produce same outputs
    /// - **Parallelizable**: Can evaluate nodes concurrently without race conditions
    /// - **Stateless**: No shared state maintained during evaluation
    fn filter(&self, pod: &Pod, nodes: &[Node]) -> FilterResult;
}

/// Resource feasibility checker for validating node capacity.
///
/// This filter predicate checks whether a node has sufficient allocatable resources
/// to accommodate a pod's resource requests, preventing over-commitment and ensuring
/// QoS guarantees can be met.
#[derive(Debug, Clone, Copy, Default)]
pub struct ResourceFeasibilityChecker;

impl ResourceFeasibilityChecker {
    /// Creates a new resource feasibility checker.
    pub fn new() -> Self {
        Self
    }

    /// Validates CPU capacity against pod requirements.
    ///
    /// Returns true if the node has sufficient allocatable CPU to satisfy the pod's
    /// total CPU request.
    pub fn validate_cpu_capacity(&self, pod_request: f32, node_allocatable: f32) -> bool {
        node_allocatable >= pod_request
    }

    /// Validates memory capacity against pod requirements.
    ///
    /// Returns true if the node has sufficient allocatable memory to satisfy the pod's
    /// total memory request.
    pub fn validate_memory_capacity(&self, pod_request: u64, node_allocatable: u64) -> bool {
        node_allocatable >= pod_request
    }

    /// Validates storage capacity against pod requirements.
    ///
    /// Returns true if the node has sufficient allocatable disk space to satisfy the
    /// pod's total storage request.
    pub fn validate_storage_capacity(&self, pod_request: u64, node_allocatable: u64) -> bool {
        node_allocatable >= pod_request
    }

    /// Validates extended resources (GPUs, FPGAs, custom resources).
    ///
    /// Extended resources must match exactly (integral quantities) as they represent
    /// discrete hardware units.
    pub fn validate_extended_resources(
        &self,
        _pod_requests: &HashMap<String, u64>,
        _node_allocatable: &HashMap<String, u64>,
    ) -> bool {
        // TODO: Implement extended resource validation
        // For each requested resource, check if node has sufficient quantity
        true
    }

    /// Checks overall resource feasibility for a pod on a node.
    ///
    /// Validates CPU, memory, storage, and extended resources in a single check.
    pub fn check_feasibility(&self, pod: &Pod, node: &Node) -> Result<(), RejectionReason> {
        // Check CPU capacity
        if !self.validate_cpu_capacity(
            pod.resources.cpu_cores,
            node.resources_allocatable.cpu_cores,
        ) {
            return Err(RejectionReason::InsufficientCpu {
                requested: pod.resources.cpu_cores,
                available: node.resources_allocatable.cpu_cores,
            });
        }

        // Check memory capacity
        if !self.validate_memory_capacity(
            pod.resources.memory_mb,
            node.resources_allocatable.memory_mb,
        ) {
            return Err(RejectionReason::InsufficientMemory {
                requested: pod.resources.memory_mb,
                available: node.resources_allocatable.memory_mb,
            });
        }

        // Check disk capacity
        if !self
            .validate_storage_capacity(pod.resources.disk_mb, node.resources_allocatable.disk_mb)
        {
            return Err(RejectionReason::InsufficientDisk {
                requested: pod.resources.disk_mb,
                available: node.resources_allocatable.disk_mb,
            });
        }

        Ok(())
    }
}

impl Filter for ResourceFeasibilityChecker {
    fn filter(&self, pod: &Pod, nodes: &[Node]) -> FilterResult {
        let mut eligible_nodes = Vec::new();
        let mut filtered_nodes = Vec::new();
        let mut admission_reasons = HashMap::new();

        for node in nodes {
            match self.check_feasibility(pod, node) {
                Ok(()) => {
                    eligible_nodes.push(node.id.clone());
                    admission_reasons.insert(
                        node.id.clone(),
                        vec![format!(
                            "Sufficient resources: CPU={:.2}/{:.2}, Memory={}/{}, Disk={}/{}",
                            pod.resources.cpu_cores,
                            node.resources_allocatable.cpu_cores,
                            pod.resources.memory_mb,
                            node.resources_allocatable.memory_mb,
                            pod.resources.disk_mb,
                            node.resources_allocatable.disk_mb
                        )],
                    );
                }
                Err(reason) => {
                    filtered_nodes.push((node.id.clone(), reason));
                }
            }
        }

        FilterResult {
            eligible_nodes,
            filtered_nodes,
            admission_reasons,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use orchestrator_shared_types::{Keypair, NodeResources, NodeStatus};

    fn generate_node_id() -> NodeId {
        Keypair::generate().public_key()
    }

    fn create_test_node(cpu: f32, memory: u64, disk: u64) -> Node {
        Node {
            id: generate_node_id(),
            address: "127.0.0.1:8080".to_string(),
            status: NodeStatus::Ready,
            labels: HashMap::new(),
            resources_capacity: NodeResources {
                cpu_cores: cpu,
                memory_mb: memory,
                disk_mb: disk,
            },
            resources_allocatable: NodeResources {
                cpu_cores: cpu,
                memory_mb: memory,
                disk_mb: disk,
            },
        }
    }

    fn create_test_pod(cpu: f32, memory: u64, disk: u64) -> Pod {
        Pod {
            name: "test-pod".to_string(),
            labels: HashMap::new(),
            resources: NodeResources {
                cpu_cores: cpu,
                memory_mb: memory,
                disk_mb: disk,
            },
            predicates: FilterPredicates::default(),
        }
    }

    #[test]
    fn test_resource_feasibility_cpu_sufficient() {
        let checker = ResourceFeasibilityChecker::new();
        assert!(checker.validate_cpu_capacity(1.0, 2.0));
        assert!(checker.validate_cpu_capacity(2.0, 2.0));
        assert!(!checker.validate_cpu_capacity(3.0, 2.0));
    }

    #[test]
    fn test_resource_feasibility_memory_sufficient() {
        let checker = ResourceFeasibilityChecker::new();
        assert!(checker.validate_memory_capacity(1024, 2048));
        assert!(checker.validate_memory_capacity(2048, 2048));
        assert!(!checker.validate_memory_capacity(3072, 2048));
    }

    #[test]
    fn test_resource_feasibility_storage_sufficient() {
        let checker = ResourceFeasibilityChecker::new();
        assert!(checker.validate_storage_capacity(1024, 2048));
        assert!(checker.validate_storage_capacity(2048, 2048));
        assert!(!checker.validate_storage_capacity(3072, 2048));
    }

    #[test]
    fn test_filter_eligible_nodes() {
        let checker = ResourceFeasibilityChecker::new();
        let pod = create_test_pod(1.0, 1024, 1024);
        let nodes = vec![
            create_test_node(2.0, 2048, 2048), // Should pass
            create_test_node(0.5, 2048, 2048), // Should fail - insufficient CPU
            create_test_node(2.0, 512, 2048),  // Should fail - insufficient memory
        ];

        let result = checker.filter(&pod, &nodes);

        assert_eq!(result.eligible_nodes.len(), 1);
        assert_eq!(result.filtered_nodes.len(), 2);
        assert_eq!(result.eligible_nodes[0], nodes[0].id);
    }

    #[test]
    fn test_filter_rejection_reasons() {
        let checker = ResourceFeasibilityChecker::new();
        let pod = create_test_pod(2.0, 2048, 2048);
        let nodes = vec![create_test_node(1.0, 1024, 1024)];

        let result = checker.filter(&pod, &nodes);

        assert_eq!(result.eligible_nodes.len(), 0);
        assert_eq!(result.filtered_nodes.len(), 1);

        match &result.filtered_nodes[0].1 {
            RejectionReason::InsufficientCpu {
                requested,
                available,
            } => {
                assert_eq!(*requested, 2.0);
                assert_eq!(*available, 1.0);
            }
            _ => panic!("Expected InsufficientCpu rejection reason"),
        }
    }

    #[test]
    fn test_toleration_operator_equality() {
        assert_eq!(TolerationOperator::Equal, TolerationOperator::Equal);
        assert_ne!(TolerationOperator::Equal, TolerationOperator::Exists);
    }

    #[test]
    fn test_taint_effect_equality() {
        assert_eq!(TaintEffect::NoSchedule, TaintEffect::NoSchedule);
        assert_ne!(TaintEffect::NoSchedule, TaintEffect::PreferNoSchedule);
    }

    #[test]
    fn test_match_operator_variants() {
        let operators = vec![
            MatchOperator::In,
            MatchOperator::NotIn,
            MatchOperator::Exists,
            MatchOperator::DoesNotExist,
            MatchOperator::Gt,
            MatchOperator::Lt,
        ];
        assert_eq!(operators.len(), 6);
    }
}
