//! P2P network message types for gossipsub topics.
//!
//! Defines message formats for the four P2P topics:
//! - Gradient: Resource availability and pricing signals
//! - Election: Leader election consensus
//! - Credit: Economic/credit transactions
//! - Septal: Coordination barriers and synchronization

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ============================================================================
// NETWORK MESSAGE WRAPPER
// ============================================================================

/// Wrapper for all P2P network messages.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkMessage {
    /// Unique message ID for deduplication.
    pub id: String,
    /// Originating node ID.
    pub source: String,
    /// Topic this message belongs to.
    pub topic: String,
    /// Message payload (serialized message content).
    pub payload: Vec<u8>,
    /// When the message was created.
    pub timestamp: DateTime<Utc>,
    /// Optional correlation ID for request-response patterns.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<String>,
    /// Sequence number for ordering guarantees.
    pub sequence: u64,
    /// TTL in hops (for flood limiting).
    pub ttl: u8,
}

impl NetworkMessage {
    /// Create a new network message.
    pub fn new(source: String, topic: String, payload: Vec<u8>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            source,
            topic,
            payload,
            timestamp: Utc::now(),
            correlation_id: None,
            sequence: 0,
            ttl: 64,
        }
    }

    /// Create a network message with correlation ID.
    pub fn with_correlation(mut self, correlation_id: impl Into<String>) -> Self {
        self.correlation_id = Some(correlation_id.into());
        self
    }

    /// Create a network message with sequence number.
    pub fn with_sequence(mut self, sequence: u64) -> Self {
        self.sequence = sequence;
        self
    }

    /// Create a network message with custom TTL.
    pub fn with_ttl(mut self, ttl: u8) -> Self {
        self.ttl = ttl;
        self
    }
}

// ============================================================================
// GRADIENT TOPIC: Resource Availability Price Signals
// ============================================================================

/// Gradient messages communicate resource pricing/availability.
///
/// Each node broadcasts its resource gradients to allow workload placement
/// decisions based on price signals rather than central scheduling.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GradientMessage {
    /// Node ID advertising resources.
    pub node_id: String,
    /// CPU availability per core (0.0-1.0 scale).
    pub cpu_gradient: f32,
    /// Memory availability ratio (0.0-1.0).
    pub memory_gradient: f32,
    /// Disk availability ratio (0.0-1.0).
    pub disk_gradient: f32,
    /// Price signal (economic units per CPU core).
    pub cpu_price: f32,
    /// Price signal (economic units per GB memory).
    pub memory_price: f32,
    /// Price signal (economic units per GB disk).
    pub disk_price: f32,
    /// Timestamp when gradient was computed.
    pub computed_at: DateTime<Utc>,
    /// Freshness factor (1.0 = just computed, decays over time).
    pub freshness: f32,
    /// Additional metadata.
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

impl GradientMessage {
    /// Create a new gradient message with current availability.
    pub fn new(
        node_id: String,
        cpu_available: f32,
        memory_available: f32,
        disk_available: f32,
    ) -> Self {
        Self {
            node_id,
            cpu_gradient: cpu_available,
            memory_gradient: memory_available,
            disk_gradient: disk_available,
            // Base prices (inverse of availability - scarce resources cost more)
            cpu_price: if cpu_available > 0.0 {
                1.0 / cpu_available
            } else {
                f32::MAX
            },
            memory_price: if memory_available > 0.0 {
                1.0 / memory_available
            } else {
                f32::MAX
            },
            disk_price: if disk_available > 0.0 {
                1.0 / disk_available
            } else {
                f32::MAX
            },
            computed_at: Utc::now(),
            freshness: 1.0,
            metadata: HashMap::new(),
        }
    }

    /// Serialize to bytes.
    pub fn to_bytes(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec(self)
    }

    /// Deserialize from bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, serde_json::Error> {
        serde_json::from_slice(bytes)
    }
}

// ============================================================================
// ELECTION TOPIC: Leader Election Consensus
// ============================================================================

/// Phase in the election protocol.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ElectionPhase {
    /// Candidate announces candidacy.
    Prepare,
    /// Acceptors promise to reject lower proposals.
    Promise,
    /// Candidate sends accepted value.
    Accept,
    /// Acceptor confirms acceptance.
    Accepted,
    /// Leader elected and announced.
    Elected,
}

/// Paxos/Raft-inspired leader election message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElectionMessage {
    /// Election round number (monotonically increasing).
    pub round: u64,
    /// Proposer node ID.
    pub proposer: String,
    /// Phase in election protocol.
    pub phase: ElectionPhase,
    /// Ballot/proposal number.
    pub ballot: u64,
    /// Current elected leader (if known).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub leader: Option<String>,
    /// Acceptor node IDs that have promised.
    pub promises: Vec<String>,
    /// Required quorum size.
    pub quorum_size: u32,
    /// Current quorum achieved.
    pub quorum_achieved: u32,
    /// Election region/scope (for partitioned leadership).
    #[serde(default)]
    pub region: String,
}

impl ElectionMessage {
    /// Create a new election prepare message.
    pub fn prepare(proposer: String, round: u64, ballot: u64, quorum_size: u32) -> Self {
        Self {
            round,
            proposer,
            phase: ElectionPhase::Prepare,
            ballot,
            leader: None,
            promises: Vec::new(),
            quorum_size,
            quorum_achieved: 0,
            region: String::new(),
        }
    }

    /// Create a promise response.
    pub fn promise(proposer: String, round: u64, ballot: u64, acceptor: String) -> Self {
        Self {
            round,
            proposer,
            phase: ElectionPhase::Promise,
            ballot,
            leader: None,
            promises: vec![acceptor],
            quorum_size: 0,
            quorum_achieved: 1,
            region: String::new(),
        }
    }

    /// Create an elected announcement.
    pub fn elected(leader: String, round: u64, ballot: u64) -> Self {
        Self {
            round,
            proposer: leader.clone(),
            phase: ElectionPhase::Elected,
            ballot,
            leader: Some(leader),
            promises: Vec::new(),
            quorum_size: 0,
            quorum_achieved: 0,
            region: String::new(),
        }
    }

    /// Serialize to bytes.
    pub fn to_bytes(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec(self)
    }

    /// Deserialize from bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, serde_json::Error> {
        serde_json::from_slice(bytes)
    }
}

// ============================================================================
// CREDIT TOPIC: Economic Transactions
// ============================================================================

/// Type of credit/economic transaction.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TransactionType {
    /// Node pays for resources used.
    ResourcePayment,
    /// Node receives credit from work done.
    ResourceCredit,
    /// Debt settlement between nodes.
    DebtSettlement,
    /// Penalty/fee assessment.
    Penalty,
    /// Credit transfer between nodes.
    Transfer,
}

/// Economic transaction for credit/payment tracking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreditMessage {
    /// Transaction ID.
    pub transaction_id: String,
    /// Debtor node (sender).
    pub from: String,
    /// Creditor node (receiver).
    pub to: String,
    /// Type of transaction.
    pub transaction_type: TransactionType,
    /// Amount (in credit units).
    pub amount: f64,
    /// Currency/unit type.
    pub unit: String,
    /// Resource involved (if applicable).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource: Option<String>,
    /// Duration of credit period.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub period: Option<String>,
    /// Settlement status.
    pub settled: bool,
    /// Timestamp of transaction.
    pub created_at: DateTime<Utc>,
    /// Optional reference to related transaction.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
}

impl CreditMessage {
    /// Create a new resource payment transaction.
    pub fn resource_payment(from: String, to: String, amount: f64, resource: String) -> Self {
        Self {
            transaction_id: Uuid::new_v4().to_string(),
            from,
            to,
            transaction_type: TransactionType::ResourcePayment,
            amount,
            unit: "credits".to_string(),
            resource: Some(resource),
            period: None,
            settled: false,
            created_at: Utc::now(),
            reference: None,
        }
    }

    /// Create a credit transfer transaction.
    pub fn transfer(from: String, to: String, amount: f64) -> Self {
        Self {
            transaction_id: Uuid::new_v4().to_string(),
            from,
            to,
            transaction_type: TransactionType::Transfer,
            amount,
            unit: "credits".to_string(),
            resource: None,
            period: None,
            settled: false,
            created_at: Utc::now(),
            reference: None,
        }
    }

    /// Serialize to bytes.
    pub fn to_bytes(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec(self)
    }

    /// Deserialize from bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, serde_json::Error> {
        serde_json::from_slice(bytes)
    }
}

// ============================================================================
// SEPTAL TOPIC: Coordination Barriers & Synchronization
// ============================================================================

/// Type of coordination barrier.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BarrierType {
    /// Two-phase commit for global consistency.
    TwoPhaseCommit,
    /// Ensure all nodes have consistent state.
    StateConsistency,
    /// Synchronize scheduling decisions.
    SchedulingSync,
    /// Coordinate resource reservations.
    ReservationSync,
    /// Generic sync point.
    Checkpoint,
}

/// Septal messages for distributed coordination.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeptalMessage {
    /// Barrier type.
    pub barrier_type: BarrierType,
    /// Barrier ID/epoch.
    pub barrier_id: String,
    /// Initiator node.
    pub initiator: String,
    /// Participants in the barrier.
    pub participants: Vec<String>,
    /// Phase of the barrier (1=Enter, 2=Commit/Abort).
    pub phase: u8,
    /// Votes/acknowledgments from participants.
    pub votes: HashMap<String, bool>,
    /// Global decision (if voting complete).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decision: Option<bool>,
    /// Timeout in milliseconds.
    pub timeout_ms: u64,
    /// Timestamp when barrier was initiated.
    pub initiated_at: DateTime<Utc>,
    /// Optional payload for the barrier.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload: Option<serde_json::Value>,
}

impl SeptalMessage {
    /// Create a new barrier initiation message.
    pub fn initiate(
        barrier_type: BarrierType,
        initiator: String,
        participants: Vec<String>,
        timeout_ms: u64,
    ) -> Self {
        Self {
            barrier_type,
            barrier_id: Uuid::new_v4().to_string(),
            initiator,
            participants,
            phase: 1,
            votes: HashMap::new(),
            decision: None,
            timeout_ms,
            initiated_at: Utc::now(),
            payload: None,
        }
    }

    /// Create a vote response message.
    pub fn vote(barrier_id: String, voter: String, vote: bool) -> Self {
        let mut votes = HashMap::new();
        votes.insert(voter.clone(), vote);

        Self {
            barrier_type: BarrierType::Checkpoint,
            barrier_id,
            initiator: voter,
            participants: Vec::new(),
            phase: 1,
            votes,
            decision: None,
            timeout_ms: 0,
            initiated_at: Utc::now(),
            payload: None,
        }
    }

    /// Create a decision announcement message.
    pub fn decide(barrier_id: String, initiator: String, decision: bool) -> Self {
        Self {
            barrier_type: BarrierType::Checkpoint,
            barrier_id,
            initiator,
            participants: Vec::new(),
            phase: 2,
            votes: HashMap::new(),
            decision: Some(decision),
            timeout_ms: 0,
            initiated_at: Utc::now(),
            payload: None,
        }
    }

    /// Serialize to bytes.
    pub fn to_bytes(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec(self)
    }

    /// Deserialize from bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, serde_json::Error> {
        serde_json::from_slice(bytes)
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_network_message_creation() {
        let msg = NetworkMessage::new(
            "node-1".to_string(),
            "/orchestrator/gradient/1.0.0".to_string(),
            vec![1, 2, 3],
        );
        assert!(!msg.id.is_empty());
        assert_eq!(msg.source, "node-1");
        assert_eq!(msg.ttl, 64);
    }

    #[test]
    fn test_gradient_message_serialization() {
        let gradient = GradientMessage::new("node-1".to_string(), 0.8, 0.6, 0.9);

        let bytes = gradient.to_bytes().unwrap();
        let parsed = GradientMessage::from_bytes(&bytes).unwrap();

        assert_eq!(parsed.node_id, "node-1");
        assert_eq!(parsed.cpu_gradient, 0.8);
        assert_eq!(parsed.memory_gradient, 0.6);
        assert_eq!(parsed.freshness, 1.0);
    }

    #[test]
    fn test_election_message_phases() {
        let prepare = ElectionMessage::prepare("candidate-1".to_string(), 1, 100, 3);
        assert_eq!(prepare.phase, ElectionPhase::Prepare);
        assert_eq!(prepare.quorum_size, 3);

        let elected = ElectionMessage::elected("leader-1".to_string(), 1, 100);
        assert_eq!(elected.phase, ElectionPhase::Elected);
        assert_eq!(elected.leader, Some("leader-1".to_string()));
    }

    #[test]
    fn test_credit_message() {
        let payment = CreditMessage::resource_payment(
            "node-1".to_string(),
            "node-2".to_string(),
            10.5,
            "cpu".to_string(),
        );

        assert_eq!(payment.transaction_type, TransactionType::ResourcePayment);
        assert_eq!(payment.amount, 10.5);
        assert!(!payment.settled);
    }

    #[test]
    fn test_septal_message_barrier() {
        let barrier = SeptalMessage::initiate(
            BarrierType::TwoPhaseCommit,
            "coordinator".to_string(),
            vec!["node-1".to_string(), "node-2".to_string()],
            5000,
        );

        assert_eq!(barrier.phase, 1);
        assert_eq!(barrier.participants.len(), 2);
        assert_eq!(barrier.timeout_ms, 5000);

        let decision =
            SeptalMessage::decide(barrier.barrier_id.clone(), "coordinator".to_string(), true);
        assert_eq!(decision.phase, 2);
        assert_eq!(decision.decision, Some(true));
    }
}
