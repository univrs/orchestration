use orchestrator_shared_types::{NodeId, NodeResources};
use std::time::{Duration, Instant};
use uuid::Uuid;

/// Tiebreaker strategy for selecting among equally-scored candidate nodes.
///
/// When multiple nodes have identical or similar scores, the tiebreaker strategy
/// determines which node is ultimately selected. Different strategies optimize
/// for different operational goals.
///
/// # Strategy Details
///
/// - **Random**: Selects randomly among top-scored nodes. Provides simple,
///   unpredictable distribution without bias.
///
/// - **LeastLoaded**: Among tied nodes, chooses the one with the lowest overall
///   resource utilization. Helps balance cluster load.
///
/// - **RoundRobin**: Distributes selections evenly across tied nodes in a
///   round-robin fashion. Ensures fair distribution over time.
///
/// - **WeightedRandom**: Probabilistic selection weighted by scores. Higher-scored
///   nodes are more likely to be selected, but lower-scored nodes still have a chance.
///
/// - **AffinityBased**: Prefers nodes with affinity to the container/workload
///   based on labels, topology, or co-location requirements.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TiebreakerStrategy {
    Random,
    LeastLoaded,
    RoundRobin,
    WeightedRandom,
    AffinityBased,
}

/// Resource claim representing resources to be reserved on a node.
///
/// This type wraps `NodeResources` to represent a temporary reservation
/// of resources on a selected node. The reservation prevents other schedulers
/// from double-booking resources during the bind operation.
pub type ResourceClaim = NodeResources;

/// A temporary resource reservation on a selected node.
///
/// When a node is selected for workload placement, a reservation is created
/// to temporarily lock the required resources. This prevents race conditions
/// where multiple scheduling decisions try to use the same resources before
/// binding completes.
///
/// # Timeout Behavior
///
/// Reservations have an expiration time to prevent resource deadlocks. If the
/// bind operation does not complete before the expiration time, the reservation
/// is automatically released and the resources become available again.
///
/// # Fields
///
/// - `lock_id`: Unique identifier for this reservation, used to validate the
///   reservation during the bind operation.
///
/// - `node_id`: The node on which resources are reserved.
///
/// - `expiration_time`: Absolute time when this reservation expires. After this
///   time, the reservation is invalid and resources are released.
///
/// - `resources_reserved`: The specific resources (CPU, memory, disk) reserved
///   on the node.
#[derive(Debug, Clone, PartialEq)]
pub struct Reservation {
    pub lock_id: Uuid,
    pub node_id: NodeId,
    pub expiration_time: Instant,
    pub resources_reserved: ResourceClaim,
}

impl Reservation {
    /// Creates a new reservation with the specified timeout duration.
    ///
    /// # Arguments
    ///
    /// * `node_id` - The node on which to reserve resources
    /// * `resources` - The resources to reserve
    /// * `timeout` - Duration from now until the reservation expires
    ///
    /// # Returns
    ///
    /// A new `Reservation` with a unique lock ID and expiration time.
    pub fn new(node_id: NodeId, resources: ResourceClaim, timeout: Duration) -> Self {
        Self {
            lock_id: Uuid::new_v4(),
            node_id,
            expiration_time: Instant::now() + timeout,
            resources_reserved: resources,
        }
    }

    /// Checks if this reservation has expired.
    ///
    /// # Returns
    ///
    /// `true` if the current time is past the expiration time, `false` otherwise.
    pub fn is_expired(&self) -> bool {
        Instant::now() > self.expiration_time
    }

    /// Returns the remaining time before this reservation expires.
    ///
    /// # Returns
    ///
    /// `Some(Duration)` with the remaining time, or `None` if already expired.
    pub fn time_remaining(&self) -> Option<Duration> {
        let now = Instant::now();
        if now < self.expiration_time {
            Some(self.expiration_time - now)
        } else {
            None
        }
    }
}

/// The result of a node selection operation.
///
/// Contains all information about the selected node, including the node identifier,
/// final score, reservation handle, and timestamp. This information is used by
/// subsequent operations in the scheduling pipeline (particularly the bind operation)
/// and for auditing/debugging purposes.
///
/// # Fields
///
/// - `winner`: The node ID of the selected node
///
/// - `score_value`: The final score that led to this node being selected.
///   Useful for debugging and understanding scheduling decisions.
///
/// - `reservation_handle`: A reservation object that holds resources on the
///   selected node. Must be validated during the bind operation.
///
/// - `selection_timestamp`: The time when the selection was made, used for
///   tracking scheduling latency and debugging.
#[derive(Debug, Clone, PartialEq)]
pub struct SelectionResult {
    pub winner: NodeId,
    pub score_value: f64,
    pub reservation_handle: Reservation,
    pub selection_timestamp: Instant,
}

impl SelectionResult {
    /// Creates a new selection result.
    ///
    /// # Arguments
    ///
    /// * `winner` - The selected node ID
    /// * `score_value` - The final score for the selected node
    /// * `reservation_handle` - The reservation created for this selection
    ///
    /// # Returns
    ///
    /// A new `SelectionResult` with the current timestamp.
    pub fn new(winner: NodeId, score_value: f64, reservation_handle: Reservation) -> Self {
        Self {
            winner,
            score_value,
            reservation_handle,
            selection_timestamp: Instant::now(),
        }
    }
}

/// A candidate node with its associated score.
///
/// Represents a node that has passed the filtering phase and has been scored
/// by the scoring functions. The score represents the relative preference for
/// placing a workload on this node, with higher scores indicating better fit.
///
/// # Fields
///
/// - `node_id`: The identifier of the candidate node
///
/// - `score`: The final score from the scoring phase. Typically a weighted sum
///   of multiple scoring functions (resource balance, spreading, affinity, etc.).
#[derive(Debug, Clone, PartialEq)]
pub struct ScoredCandidate {
    pub node_id: NodeId,
    pub score: f64,
}

impl ScoredCandidate {
    /// Creates a new scored candidate.
    ///
    /// # Arguments
    ///
    /// * `node_id` - The node identifier
    /// * `score` - The node's score from the scoring phase
    pub fn new(node_id: NodeId, score: f64) -> Self {
        Self { node_id, score }
    }
}

/// Errors that can occur during the selection process.
///
/// Selection can fail for various reasons, from having no candidates available
/// to failures in the tiebreaker logic or reservation creation.
#[derive(Debug, Clone, thiserror::Error, PartialEq)]
pub enum SelectionError {
    /// No candidates were provided for selection.
    ///
    /// This indicates that either the filtering phase eliminated all nodes,
    /// or no nodes were available in the cluster.
    #[error("No candidates available for selection")]
    NoCandidates,

    /// All candidates were filtered out during the selection process.
    ///
    /// This can occur if additional filtering logic in the selector (beyond
    /// the main filter phase) eliminated all remaining candidates.
    #[error("All candidates were filtered out: {0}")]
    AllCandidatesFiltered(String),

    /// Failed to create a reservation on the selected node.
    ///
    /// This can happen if the node's state changed between scoring and selection,
    /// making it no longer suitable for placement.
    #[error("Failed to create reservation on node {0}: {1}")]
    ReservationFailed(Box<NodeId>, String),

    /// The tiebreaker strategy failed to select a winner.
    ///
    /// This indicates an error in the tiebreaker logic itself, such as
    /// invalid configuration or unexpected state.
    #[error("Tiebreaker strategy {0:?} failed: {1}")]
    TiebreakerFailed(TiebreakerStrategy, String),
}

/// Trait for implementing node selection logic.
///
/// The `Selector` trait defines the interface for the final decision phase in
/// the scheduling pipeline. It receives scored candidates from the scoring phase
/// and applies tiebreaker logic to select a single winning node, then creates
/// a reservation for that node.
///
/// # Selection Process
///
/// 1. Receive scored candidates from the scheduling.score phase
/// 2. Apply tiebreaker strategy when multiple nodes have similar scores
/// 3. Create a temporary resource reservation on the selected node
/// 4. Return a selection result with the winner and reservation handle
///
/// # Tiebreaker Logic
///
/// When multiple candidates have identical or very similar scores, the tiebreaker
/// strategy determines which node is ultimately selected. Different strategies
/// optimize for different goals (randomness, load balancing, affinity, etc.).
///
/// # Reservation Atomicity
///
/// Implementations must ensure that reservation creation is atomic and conflict-free.
/// If multiple selectors try to reserve the same resources simultaneously, proper
/// locking or optimistic concurrency control must be used.
///
/// # Performance Requirements
///
/// - Selection should complete in O(n) time where n is the number of candidates
/// - Reservation operations must be atomic and conflict-free
/// - Timeout values should account for network latency and bind operation time
pub trait Selector {
    /// Selects a winning node from a list of scored candidates.
    ///
    /// # Arguments
    ///
    /// * `candidates` - Slice of scored candidates from the scoring phase
    /// * `strategy` - The tiebreaker strategy to use for resolving ties
    ///
    /// # Returns
    ///
    /// * `Ok(SelectionResult)` - Successfully selected a node and created a reservation
    /// * `Err(SelectionError)` - Selection failed for one of several reasons
    ///
    /// # Errors
    ///
    /// - `SelectionError::NoCandidates` - The candidates slice is empty
    /// - `SelectionError::AllCandidatesFiltered` - All candidates were filtered out
    /// - `SelectionError::ReservationFailed` - Could not create reservation on selected node
    /// - `SelectionError::TiebreakerFailed` - Tiebreaker strategy execution failed
    ///
    /// # Example
    ///
    /// ```ignore
    /// use scheduler_interface::select::{Selector, ScoredCandidate, TiebreakerStrategy};
    /// use uuid::Uuid;
    ///
    /// let candidates = vec![
    ///     ScoredCandidate::new(Uuid::new_v4(), 85.0),
    ///     ScoredCandidate::new(Uuid::new_v4(), 90.0),
    ///     ScoredCandidate::new(Uuid::new_v4(), 90.0), // Tie with second
    /// ];
    ///
    /// let selector = MySelector::new();
    /// let result = selector.select(&candidates, TiebreakerStrategy::Random)?;
    ///
    /// println!("Selected node: {}", result.winner);
    /// println!("Score: {}", result.score_value);
    /// ```
    fn select(
        &self,
        candidates: &[ScoredCandidate],
        strategy: TiebreakerStrategy,
    ) -> Result<SelectionResult, SelectionError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use orchestrator_shared_types::Keypair;

    fn generate_node_id() -> NodeId {
        Keypair::generate().public_key()
    }

    #[test]
    fn test_reservation_creation() {
        let node_id = generate_node_id();
        let resources = NodeResources {
            cpu_cores: 2.0,
            memory_mb: 4096,
            disk_mb: 10240,
        };
        let timeout = Duration::from_secs(30);

        let reservation = Reservation::new(node_id, resources.clone(), timeout);

        assert_eq!(reservation.node_id, node_id);
        assert_eq!(reservation.resources_reserved, resources);
        assert!(!reservation.is_expired());
    }

    #[test]
    fn test_reservation_expiration() {
        let node_id = generate_node_id();
        let resources = NodeResources::default();
        let timeout = Duration::from_millis(10);

        let reservation = Reservation::new(node_id, resources, timeout);

        // Should not be expired immediately
        assert!(!reservation.is_expired());

        // Wait for expiration
        std::thread::sleep(Duration::from_millis(15));

        // Should now be expired
        assert!(reservation.is_expired());
        assert!(reservation.time_remaining().is_none());
    }

    #[test]
    fn test_scored_candidate_creation() {
        let node_id = generate_node_id();
        let score = 85.5;

        let candidate = ScoredCandidate::new(node_id, score);

        assert_eq!(candidate.node_id, node_id);
        assert_eq!(candidate.score, score);
    }

    #[test]
    fn test_selection_result_creation() {
        let node_id = generate_node_id();
        let resources = NodeResources::default();
        let reservation = Reservation::new(node_id, resources, Duration::from_secs(30));
        let score = 92.3;

        let result = SelectionResult::new(node_id, score, reservation.clone());

        assert_eq!(result.winner, node_id);
        assert_eq!(result.score_value, score);
        assert_eq!(result.reservation_handle, reservation);
    }

    #[test]
    fn test_tiebreaker_strategy_enum() {
        let strategies = vec![
            TiebreakerStrategy::Random,
            TiebreakerStrategy::LeastLoaded,
            TiebreakerStrategy::RoundRobin,
            TiebreakerStrategy::WeightedRandom,
            TiebreakerStrategy::AffinityBased,
        ];

        // Ensure all strategies are unique
        for (i, strategy1) in strategies.iter().enumerate() {
            for (j, strategy2) in strategies.iter().enumerate() {
                if i == j {
                    assert_eq!(strategy1, strategy2);
                } else {
                    assert_ne!(strategy1, strategy2);
                }
            }
        }
    }

    #[test]
    fn test_selection_error_variants() {
        let error1 = SelectionError::NoCandidates;
        let error2 = SelectionError::AllCandidatesFiltered("test reason".to_string());
        let error3 =
            SelectionError::ReservationFailed(Box::new(generate_node_id()), "test".to_string());
        let error4 =
            SelectionError::TiebreakerFailed(TiebreakerStrategy::Random, "test".to_string());

        // Verify error messages are formatted correctly
        assert!(error1.to_string().contains("No candidates"));
        assert!(error2.to_string().contains("filtered out"));
        assert!(error3.to_string().contains("reservation"));
        assert!(error4.to_string().contains("Tiebreaker"));
    }
}
