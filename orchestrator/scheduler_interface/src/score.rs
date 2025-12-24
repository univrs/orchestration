//! Scheduling score module implementing the scheduling.score trait from DOL spec.
//!
//! This module defines the second phase of the scheduling process where filtered nodes
//! are ranked by preference through a weighted scoring system. After the filter phase
//! eliminates unsuitable nodes, the scoring phase evaluates each remaining node across
//! multiple dimensions to determine the best placement for a workload.
//!
//! # Scoring Functions
//!
//! Each scoring function produces a normalized score between 0-100, which is then
//! multiplied by a configurable weight to produce the final score:
//!
//! - **Resource Balance**: Evaluates balanced resource allocation across nodes
//! - **Spreading**: Promotes distribution across failure domains for high availability
//! - **Binpacking**: Optimizes for resource consolidation and efficiency
//! - **Preferred Affinity**: Supports co-location preferences between workloads
//! - **Anti-Affinity**: Ensures separation requirements between workloads
//! - **Topology Spread**: Evaluates even distribution across topology domains
//!
//! # Score Normalization
//!
//! All scoring functions produce normalized values in the range 0-100:
//! - 0: Worst possible score for this function
//! - 100: Best possible score for this function
//!
//! # Final Score Calculation
//!
//! The final score for each node is computed as:
//! ```text
//! final_score = Σ(normalized_score[i] × weight[i])
//! ```
//!
//! After scoring, nodes are sorted in descending order by final_score, and the
//! scheduler selects the highest-scoring node for placement.

use orchestrator_shared_types::{Node, NodeId, WorkloadDefinition};
use std::collections::HashMap;
use std::sync::Arc;

/// Enumeration of all scoring functions available in the scheduling system.
///
/// Each variant represents a different dimension of node evaluation, with specific
/// metrics that contribute to determining the best placement for a workload.
#[derive(Debug, Clone, PartialEq)]
pub enum ScoringFunction {
    /// Resource balance scoring evaluates how balanced resource allocation would be
    /// after placement. Prevents hotspots and promotes even cluster utilization.
    ResourceBalance {
        /// CPU utilization score (0-100)
        cpu_score: f64,
        /// Memory utilization score (0-100)
        memory_score: f64,
        /// Overall allocation ratio score (0-100)
        allocation_ratio: f64,
    },

    /// Spreading scoring promotes distribution of workloads across failure domains.
    /// Reduces blast radius of failures by encouraging geographic and topological spread.
    Spreading {
        /// Distribution score across nodes (0-100)
        node_distribution: f64,
        /// Distribution score across availability zones (0-100)
        zone_distribution: f64,
        /// Distribution score across failure domains (0-100)
        failure_domain_distribution: f64,
    },

    /// Binpacking scoring optimizes for resource consolidation and efficiency.
    /// Prefers filling partially utilized nodes to reduce fragmentation and enable
    /// better cluster scale-down.
    Binpacking {
        /// Target utilization level (0.0-1.0)
        utilization_target: f64,
        /// How well this placement consolidates resources (0-100)
        consolidation_score: f64,
        /// Penalty for resource fragmentation (0-100)
        fragmentation_penalty: f64,
    },

    /// Preferred affinity scoring evaluates pod-to-pod and pod-to-node affinity preferences.
    /// Supports co-location requirements like databases with caches for improved locality.
    PreferredAffinity {
        /// Score for pod-to-pod affinity satisfaction (0-100)
        pod_affinity_score: f64,
        /// Score for pod-to-node affinity satisfaction (0-100)
        node_affinity_score: f64,
        /// Weight of the preference constraint (0.0-1.0)
        preference_weight: f64,
    },

    /// Anti-affinity scoring evaluates separation requirements between workloads.
    /// Penalizes placements that violate anti-affinity rules to ensure high availability
    /// through distribution.
    AntiAffinity {
        /// Score for maintaining separation (0-100)
        separation_score: f64,
        /// Penalty for violating anti-affinity rules (0-100)
        violation_penalty: f64,
    },

    /// Topology spread scoring evaluates even distribution across topology domains.
    /// Measures skew against target spread constraints for zone-aware and rack-aware
    /// spreading.
    TopologySpread {
        /// Score for meeting topology constraints (0-100)
        constraint_score: f64,
        /// Penalty for distribution skew (0-100)
        skew_penalty: f64,
        /// Target spread distribution (0.0-1.0)
        spread_target: f64,
    },
}

impl ScoringFunction {
    /// Returns the normalized score (0-100) for this scoring function.
    ///
    /// The normalized score represents how well this particular aspect of the
    /// placement performs, independent of its configured weight.
    pub fn normalized_score(&self) -> f64 {
        match self {
            ScoringFunction::ResourceBalance {
                cpu_score,
                memory_score,
                allocation_ratio,
            } => {
                // Average of the three components
                (cpu_score + memory_score + allocation_ratio) / 3.0
            }
            ScoringFunction::Spreading {
                node_distribution,
                zone_distribution,
                failure_domain_distribution,
            } => {
                // Average of distribution scores
                (node_distribution + zone_distribution + failure_domain_distribution) / 3.0
            }
            ScoringFunction::Binpacking {
                consolidation_score,
                fragmentation_penalty,
                ..
            } => {
                // Consolidation score minus fragmentation penalty
                (consolidation_score - fragmentation_penalty).max(0.0)
            }
            ScoringFunction::PreferredAffinity {
                pod_affinity_score,
                node_affinity_score,
                preference_weight,
            } => {
                // Weighted average of affinity scores
                ((pod_affinity_score + node_affinity_score) / 2.0) * preference_weight * 100.0
            }
            ScoringFunction::AntiAffinity {
                separation_score,
                violation_penalty,
            } => {
                // Separation score minus violation penalty
                (separation_score - violation_penalty).max(0.0)
            }
            ScoringFunction::TopologySpread {
                constraint_score,
                skew_penalty,
                ..
            } => {
                // Constraint score minus skew penalty
                (constraint_score - skew_penalty).max(0.0)
            }
        }
        .clamp(0.0, 100.0)
    }
}

/// Weight configuration for a scoring function.
///
/// Weights determine the relative importance of each scoring function in the
/// final score calculation. Operators can tune scheduling behavior by adjusting
/// these weights based on cluster priorities.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScoringWeight {
    /// Priority level for this scoring function (higher = more important)
    pub priority: u32,
    /// Coefficient multiplier for the normalized score
    pub coefficient: f64,
}

impl ScoringWeight {
    /// Creates a new scoring weight with the given priority and coefficient.
    ///
    /// # Arguments
    ///
    /// * `priority` - Priority level (0-100 recommended)
    /// * `coefficient` - Weight coefficient (typically 0.0-10.0)
    pub fn new(priority: u32, coefficient: f64) -> Self {
        Self {
            priority,
            coefficient,
        }
    }

    /// Creates a default weight for balanced scoring.
    pub fn default_weight() -> Self {
        Self {
            priority: 1,
            coefficient: 1.0,
        }
    }

    /// Creates a high priority weight for critical scoring functions.
    pub fn high_priority() -> Self {
        Self {
            priority: 10,
            coefficient: 3.0,
        }
    }

    /// Creates a low priority weight for optional scoring functions.
    pub fn low_priority() -> Self {
        Self {
            priority: 1,
            coefficient: 0.5,
        }
    }
}

/// Score for a single node across all scoring functions.
///
/// Contains the normalized score, weight multiplier, and final contribution
/// to the overall node ranking.
#[derive(Debug, Clone, PartialEq)]
pub struct NodeScore {
    /// Identifier of the node being scored
    pub node_id: NodeId,
    /// Normalized score value (0-100)
    pub normalized_value: u32,
    /// Weight multiplier applied to this score
    pub weight_multiplier: f64,
    /// Final contribution to the total score (normalized_value * weight_multiplier)
    pub contribution: f64,
}

impl NodeScore {
    /// Creates a new node score.
    ///
    /// # Arguments
    ///
    /// * `node_id` - Node identifier
    /// * `normalized_value` - Normalized score (0-100)
    /// * `weight_multiplier` - Weight to apply
    ///
    /// # Returns
    ///
    /// A new `NodeScore` with calculated contribution.
    pub fn new(node_id: NodeId, normalized_value: u32, weight_multiplier: f64) -> Self {
        let contribution = f64::from(normalized_value) * weight_multiplier;
        Self {
            node_id,
            normalized_value,
            weight_multiplier,
            contribution,
        }
    }

    /// Returns the final score contribution for this node.
    pub fn final_score(&self) -> f64 {
        self.contribution
    }
}

impl PartialOrd for NodeScore {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.contribution.partial_cmp(&other.contribution)
    }
}

/// Result of the scoring phase containing ranked nodes and final scores.
///
/// Nodes are sorted in descending order by final score, with the best placement
/// candidate at the front of the ranked_nodes vector.
#[derive(Debug, Clone, PartialEq)]
pub struct ScoringResult {
    /// Nodes ranked by final score (descending order)
    pub ranked_nodes: Vec<NodeScore>,
    /// Map of node IDs to their final scores
    pub final_scores: HashMap<NodeId, f64>,
}

impl ScoringResult {
    /// Creates a new scoring result.
    ///
    /// # Arguments
    ///
    /// * `ranked_nodes` - Vector of node scores (will be sorted descending)
    pub fn new(mut ranked_nodes: Vec<NodeScore>) -> Self {
        // Sort by contribution in descending order (highest score first)
        ranked_nodes.sort_by(|a, b| {
            b.contribution
                .partial_cmp(&a.contribution)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Build final scores map
        let final_scores: HashMap<NodeId, f64> = ranked_nodes
            .iter()
            .map(|score| (score.node_id, score.contribution))
            .collect();

        Self {
            ranked_nodes,
            final_scores,
        }
    }

    /// Returns the node with the highest score, if any.
    pub fn best_node(&self) -> Option<&NodeScore> {
        self.ranked_nodes.first()
    }

    /// Returns the score for a specific node, if it was scored.
    pub fn score_for_node(&self, node_id: &NodeId) -> Option<f64> {
        self.final_scores.get(node_id).copied()
    }

    /// Returns true if no nodes were scored.
    pub fn is_empty(&self) -> bool {
        self.ranked_nodes.is_empty()
    }

    /// Returns the number of scored nodes.
    pub fn len(&self) -> usize {
        self.ranked_nodes.len()
    }
}

/// Context information needed for scoring nodes.
///
/// Contains the workload specification, current cluster state, and other
/// information required by scoring functions to evaluate node placement.
#[derive(Debug, Clone)]
pub struct ScoringContext {
    /// The workload being scheduled
    pub workload: Arc<WorkloadDefinition>,
    /// Current nodes in the cluster with their state
    pub nodes: Vec<Node>,
    /// Current resource allocations per node (node_id -> allocated resources)
    pub current_allocations: HashMap<NodeId, NodeAllocations>,
    /// Existing workload instances in the cluster (for affinity/anti-affinity)
    pub existing_instances: Vec<WorkloadInstanceInfo>,
    /// Topology information (zones, racks, etc.)
    pub topology: HashMap<NodeId, TopologyInfo>,
    /// Scoring function weights
    pub weights: HashMap<String, ScoringWeight>,
}

impl ScoringContext {
    /// Creates a new scoring context.
    pub fn new(workload: Arc<WorkloadDefinition>, nodes: Vec<Node>) -> Self {
        Self {
            workload,
            nodes,
            current_allocations: HashMap::new(),
            existing_instances: Vec::new(),
            topology: HashMap::new(),
            weights: HashMap::new(),
        }
    }

    /// Adds current allocation information for a node.
    pub fn with_allocation(mut self, node_id: NodeId, allocation: NodeAllocations) -> Self {
        self.current_allocations.insert(node_id, allocation);
        self
    }

    /// Adds topology information for a node.
    pub fn with_topology(mut self, node_id: NodeId, topology: TopologyInfo) -> Self {
        self.topology.insert(node_id, topology);
        self
    }

    /// Adds a scoring function weight.
    pub fn with_weight(mut self, function_name: String, weight: ScoringWeight) -> Self {
        self.weights.insert(function_name, weight);
        self
    }

    /// Sets existing workload instances for affinity/anti-affinity calculations.
    pub fn with_instances(mut self, instances: Vec<WorkloadInstanceInfo>) -> Self {
        self.existing_instances = instances;
        self
    }
}

/// Current resource allocations on a node.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct NodeAllocations {
    /// Allocated CPU cores
    pub cpu_allocated: f32,
    /// Allocated memory in MB
    pub memory_allocated: u64,
    /// Allocated disk in MB
    pub disk_allocated: u64,
    /// Number of pods currently running on this node
    pub pod_count: u32,
}

impl NodeAllocations {
    /// Creates a new node allocation tracker.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the CPU utilization ratio (0.0-1.0).
    pub fn cpu_utilization(&self, capacity: f32) -> f64 {
        if capacity <= 0.0 {
            return 0.0;
        }
        (self.cpu_allocated / capacity).min(1.0) as f64
    }

    /// Returns the memory utilization ratio (0.0-1.0).
    pub fn memory_utilization(&self, capacity: u64) -> f64 {
        if capacity == 0 {
            return 0.0;
        }
        (self.memory_allocated as f64 / capacity as f64).min(1.0)
    }

    /// Returns the disk utilization ratio (0.0-1.0).
    pub fn disk_utilization(&self, capacity: u64) -> f64 {
        if capacity == 0 {
            return 0.0;
        }
        (self.disk_allocated as f64 / capacity as f64).min(1.0)
    }
}

/// Information about an existing workload instance for affinity calculations.
#[derive(Debug, Clone, PartialEq)]
pub struct WorkloadInstanceInfo {
    /// Node where this instance is running
    pub node_id: NodeId,
    /// Labels associated with this workload
    pub labels: HashMap<String, String>,
    /// Workload name/identifier
    pub workload_name: String,
}

/// Topology information for a node.
#[derive(Debug, Clone, PartialEq)]
pub struct TopologyInfo {
    /// Availability zone
    pub zone: Option<String>,
    /// Region
    pub region: Option<String>,
    /// Rack identifier
    pub rack: Option<String>,
    /// Hostname
    pub hostname: String,
    /// Custom topology labels
    pub topology_labels: HashMap<String, String>,
}

impl TopologyInfo {
    /// Creates a new topology info with just hostname.
    pub fn new(hostname: String) -> Self {
        Self {
            zone: None,
            region: None,
            rack: None,
            hostname,
            topology_labels: HashMap::new(),
        }
    }

    /// Sets the availability zone.
    pub fn with_zone(mut self, zone: String) -> Self {
        self.zone = Some(zone);
        self
    }

    /// Sets the region.
    pub fn with_region(mut self, region: String) -> Self {
        self.region = Some(region);
        self
    }

    /// Sets the rack.
    pub fn with_rack(mut self, rack: String) -> Self {
        self.rack = Some(rack);
        self
    }
}

/// Trait for implementing scoring strategies.
///
/// Implementors evaluate eligible nodes and produce a ranked list based on
/// the configured scoring functions and weights.
pub trait Scorer: Send + Sync {
    /// Scores a set of eligible nodes and returns them ranked by preference.
    ///
    /// # Arguments
    ///
    /// * `eligible_nodes` - Nodes that passed the filter phase
    /// * `context` - Scoring context with workload and cluster state
    ///
    /// # Returns
    ///
    /// A `ScoringResult` with nodes ranked in descending order by final score.
    fn score(&self, eligible_nodes: &[NodeId], context: &ScoringContext) -> ScoringResult;
}

#[cfg(test)]
mod tests {
    use super::*;
    use orchestrator_shared_types::Keypair;

    fn generate_node_id() -> NodeId {
        Keypair::generate().public_key()
    }

    #[test]
    fn test_scoring_function_normalized_score() {
        let resource_balance = ScoringFunction::ResourceBalance {
            cpu_score: 80.0,
            memory_score: 90.0,
            allocation_ratio: 85.0,
        };
        assert!((resource_balance.normalized_score() - 85.0).abs() < 0.01);

        let spreading = ScoringFunction::Spreading {
            node_distribution: 70.0,
            zone_distribution: 80.0,
            failure_domain_distribution: 75.0,
        };
        assert!((spreading.normalized_score() - 75.0).abs() < 0.01);
    }

    #[test]
    fn test_node_score_creation() {
        let node_id = generate_node_id();
        let score = NodeScore::new(node_id, 85, 2.0);
        assert_eq!(score.node_id, node_id);
        assert_eq!(score.normalized_value, 85);
        assert_eq!(score.weight_multiplier, 2.0);
        assert!((score.contribution - 170.0).abs() < 0.01);
    }

    #[test]
    fn test_scoring_result_ordering() {
        let node1 = generate_node_id();
        let node2 = generate_node_id();
        let node3 = generate_node_id();

        let scores = vec![
            NodeScore::new(node1, 75, 1.0),
            NodeScore::new(node2, 90, 1.0),
            NodeScore::new(node3, 60, 1.0),
        ];

        let result = ScoringResult::new(scores);

        assert_eq!(result.ranked_nodes.len(), 3);
        assert_eq!(result.ranked_nodes[0].node_id, node2); // Highest score
        assert_eq!(result.ranked_nodes[1].node_id, node1);
        assert_eq!(result.ranked_nodes[2].node_id, node3); // Lowest score

        assert_eq!(result.best_node().unwrap().node_id, node2);
    }

    #[test]
    fn test_scoring_weight_presets() {
        let default = ScoringWeight::default_weight();
        assert_eq!(default.priority, 1);
        assert_eq!(default.coefficient, 1.0);

        let high = ScoringWeight::high_priority();
        assert_eq!(high.priority, 10);
        assert_eq!(high.coefficient, 3.0);

        let low = ScoringWeight::low_priority();
        assert_eq!(low.priority, 1);
        assert_eq!(low.coefficient, 0.5);
    }

    #[test]
    fn test_node_allocations_utilization() {
        let allocations = NodeAllocations {
            cpu_allocated: 2.5,
            memory_allocated: 4096,
            disk_allocated: 10240,
            pod_count: 5,
        };

        assert!((allocations.cpu_utilization(4.0) - 0.625).abs() < 0.01);
        assert!((allocations.memory_utilization(8192) - 0.5).abs() < 0.01);
        assert!((allocations.disk_utilization(20480) - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_scoring_context_builder() {
        let workload = Arc::new(WorkloadDefinition {
            id: uuid::Uuid::new_v4(),
            name: "test-workload".to_string(),
            containers: vec![],
            replicas: 3,
            labels: HashMap::new(),
        });

        let node_id = generate_node_id();
        let allocations = NodeAllocations::new();
        let topology = TopologyInfo::new("node1".to_string())
            .with_zone("us-east-1a".to_string())
            .with_region("us-east-1".to_string());

        let context = ScoringContext::new(workload.clone(), vec![])
            .with_allocation(node_id, allocations)
            .with_topology(node_id, topology.clone())
            .with_weight(
                "resource_balance".to_string(),
                ScoringWeight::high_priority(),
            );

        assert_eq!(context.workload.name, "test-workload");
        assert_eq!(context.current_allocations.len(), 1);
        assert_eq!(context.topology.len(), 1);
        assert_eq!(context.weights.len(), 1);
        assert_eq!(
            context.topology.get(&node_id).unwrap().zone,
            Some("us-east-1a".to_string())
        );
    }
}
