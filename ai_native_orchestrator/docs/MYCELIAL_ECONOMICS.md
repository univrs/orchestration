# Mycelial Economics

**Credit System & Reputation for Resource Accounting**

Version: 0.1.0  
Date: December 18, 2025  
Status: Design Document (Phase 3)

---

## Overview

The Mycelial Credit System is what makes Univrs fundamentally different from Kubernetes. Instead of centralized resource quotas, Univrs uses a peer-to-peer credit network where:

- **Node operators earn credits** by providing compute resources
- **Users spend credits** to run workloads  
- **Reputation determines trust** for scheduling decisions
- **No central authority** controls the economy

This creates a self-sustaining ecosystem where resource providers and consumers find equilibrium through market dynamics rather than administrative fiat.

### Philosophy

```
┌─────────────────────────────────────────────────────────────────────────┐
│                      THE MYCELIAL PHILOSOPHY                             │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│  "Like the fungal networks that connect forest trees, sharing           │
│   nutrients based on need and contribution, the Mycelial Credit         │
│   System enables resource sharing without central coordination."        │
│                                                                          │
│  PRINCIPLES:                                                            │
│                                                                          │
│  1. CONTRIBUTION = CREDIT                                               │
│     Run a node → Earn credits from users                                │
│     Provide GPU → Earn premium credits                                  │
│     High uptime → Reputation bonus                                      │
│                                                                          │
│  2. CONSUMPTION = COST                                                  │
│     CPU time → Credits per core-second                                  │
│     Memory → Credits per GB-hour                                        │
│     Network → Credits per GB transferred                                │
│                                                                          │
│  3. TRUST = REPUTATION                                                  │
│     Reliable service → Higher reputation                                │
│     Completed workloads → Reputation increase                           │
│     Failed commitments → Reputation penalty                             │
│                                                                          │
│  4. NO CENTRAL BANK                                                     │
│     Credits minted by consensus                                         │
│     No single entity controls supply                                    │
│     Inflation bounded by protocol                                       │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## Credit System Architecture

### Credit Flow

```
┌─────────────────────────────────────────────────────────────────────────┐
│                         CREDIT FLOW                                      │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│                          ┌─────────────┐                                │
│                          │  CONSENSUS  │                                │
│                          │   (Raft)    │                                │
│                          └──────┬──────┘                                │
│                                 │                                        │
│                    Mints new credits (bounded)                           │
│                                 │                                        │
│                                 ▼                                        │
│    ┌─────────────────────────────────────────────────────────┐          │
│    │                    CREDIT LEDGER                         │          │
│    │  ┌───────────────────────────────────────────────────┐  │          │
│    │  │ Account      │ Balance │ Reserved │ Last Updated  │  │          │
│    │  ├───────────────────────────────────────────────────┤  │          │
│    │  │ user_alice   │ 10,000  │ 2,500    │ 2025-12-18   │  │          │
│    │  │ user_bob     │ 5,000   │ 0        │ 2025-12-17   │  │          │
│    │  │ node_op_1    │ 50,000  │ 0        │ 2025-12-18   │  │          │
│    │  │ node_op_2    │ 35,000  │ 0        │ 2025-12-18   │  │          │
│    │  └───────────────────────────────────────────────────┘  │          │
│    └─────────────────────────────────────────────────────────┘          │
│                                                                          │
│    USER DEPLOYS WORKLOAD                NODE PROVIDES RESOURCES          │
│    ───────────────────────              ────────────────────────         │
│                                                                          │
│    1. Reserve credits                   1. Resources consumed            │
│       (hold for duration)                  (metered)                     │
│                                                                          │
│    2. Consume reserved                  2. Credits earned                │
│       (per resource-second)                (proportional)                │
│                                                                          │
│    3. Release unused                    3. Balance updated               │
│       (workload ends)                      (via consensus)               │
│                                                                          │
│    ┌───────────┐         Transfer          ┌───────────┐                │
│    │   USER    │ ─────────────────────────▶│  NODE OP  │                │
│    │  Credits  │                           │  Credits  │                │
│    └───────────┘                           └───────────┘                │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

### Credit Data Model

```rust
// mycelial/src/credits.rs

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A credit account
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreditAccount {
    /// Account owner (peer ID)
    pub owner: PeerId,
    
    /// Available balance
    pub balance: u64,
    
    /// Reserved for pending operations
    pub reserved: u64,
    
    /// Total earned all-time
    pub total_earned: u64,
    
    /// Total spent all-time
    pub total_spent: u64,
    
    /// Account creation time
    pub created_at: DateTime<Utc>,
    
    /// Last transaction time
    pub last_activity: DateTime<Utc>,
}

impl CreditAccount {
    pub fn available(&self) -> u64 {
        self.balance.saturating_sub(self.reserved)
    }
    
    pub fn reserve(&mut self, amount: u64) -> Result<(), CreditError> {
        if self.available() < amount {
            return Err(CreditError::InsufficientFunds {
                required: amount,
                available: self.available(),
            });
        }
        self.reserved += amount;
        Ok(())
    }
    
    pub fn consume(&mut self, amount: u64) -> Result<(), CreditError> {
        if self.reserved < amount {
            return Err(CreditError::InsufficientReservation);
        }
        self.reserved -= amount;
        self.balance -= amount;
        self.total_spent += amount;
        self.last_activity = Utc::now();
        Ok(())
    }
    
    pub fn release(&mut self, amount: u64) {
        self.reserved = self.reserved.saturating_sub(amount);
    }
    
    pub fn credit(&mut self, amount: u64) {
        self.balance += amount;
        self.total_earned += amount;
        self.last_activity = Utc::now();
    }
}

/// A credit transaction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreditTransaction {
    pub id: TransactionId,
    pub from: PeerId,
    pub to: PeerId,
    pub amount: u64,
    pub tx_type: TransactionType,
    pub reference: Option<String>,  // e.g., workload ID
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransactionType {
    /// Direct transfer between accounts
    Transfer,
    
    /// Reservation for pending workload
    Reserve { workload_id: WorkloadId },
    
    /// Consumption of reserved credits
    Consume { workload_id: WorkloadId, duration_secs: u64 },
    
    /// Release of unused reservation
    Release { workload_id: WorkloadId },
    
    /// New credits minted by consensus
    Mint { reason: MintReason },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MintReason {
    /// Initial allocation for new account
    InitialAllocation,
    
    /// Reward for node uptime
    UptimeReward { node_id: NodeId, hours: u64 },
    
    /// Bonus for successful workload completion
    CompletionBonus { workload_id: WorkloadId },
}
```

---

## Pricing Model

### Resource Pricing

```rust
// mycelial/src/pricing.rs

/// Resource pricing configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PricingConfig {
    /// Credits per CPU core per hour
    pub cpu_per_core_hour: u64,
    
    /// Credits per GB memory per hour
    pub memory_per_gb_hour: u64,
    
    /// Credits per GB disk per hour
    pub disk_per_gb_hour: u64,
    
    /// Credits per GB network egress
    pub network_per_gb: u64,
    
    /// Minimum charge per workload
    pub minimum_charge: u64,
}

impl Default for PricingConfig {
    fn default() -> Self {
        Self {
            cpu_per_core_hour: 100,      // 100 credits/core/hour
            memory_per_gb_hour: 50,       // 50 credits/GB/hour
            disk_per_gb_hour: 10,         // 10 credits/GB/hour
            network_per_gb: 20,           // 20 credits/GB
            minimum_charge: 10,           // 10 credits minimum
        }
    }
}

/// Calculate cost for a workload
pub fn calculate_workload_cost(
    spec: &WorkloadSpec,
    duration_hours: f64,
    pricing: &PricingConfig,
) -> u64 {
    let cpu_cost = (spec.resources.cpu_cores * duration_hours * pricing.cpu_per_core_hour as f64) as u64;
    let memory_gb = spec.resources.memory_mb as f64 / 1024.0;
    let memory_cost = (memory_gb * duration_hours * pricing.memory_per_gb_hour as f64) as u64;
    let disk_gb = spec.resources.disk_mb as f64 / 1024.0;
    let disk_cost = (disk_gb * duration_hours * pricing.disk_per_gb_hour as f64) as u64;
    
    let total = cpu_cost + memory_cost + disk_cost;
    total.max(pricing.minimum_charge)
}

/// Estimate cost for scheduling decision
pub fn estimate_hourly_cost(spec: &WorkloadSpec, pricing: &PricingConfig) -> u64 {
    calculate_workload_cost(spec, 1.0, pricing) * spec.replicas as u64
}
```

### Dynamic Pricing (Future)

```rust
/// Market-based price adjustment
pub struct DynamicPricing {
    base_pricing: PricingConfig,
    
    /// Current cluster utilization (0.0 - 1.0)
    utilization: f64,
    
    /// Price multiplier based on demand
    demand_multiplier: f64,
}

impl DynamicPricing {
    pub fn current_pricing(&self) -> PricingConfig {
        let mut pricing = self.base_pricing.clone();
        
        // Higher prices when utilization > 80%
        if self.utilization > 0.8 {
            let surge = 1.0 + (self.utilization - 0.8) * 2.5; // Up to 1.5x
            pricing.cpu_per_core_hour = (pricing.cpu_per_core_hour as f64 * surge) as u64;
            pricing.memory_per_gb_hour = (pricing.memory_per_gb_hour as f64 * surge) as u64;
        }
        
        // Lower prices when utilization < 30% (attract workloads)
        if self.utilization < 0.3 {
            let discount = 0.7 + self.utilization; // Down to 0.7x
            pricing.cpu_per_core_hour = (pricing.cpu_per_core_hour as f64 * discount) as u64;
            pricing.memory_per_gb_hour = (pricing.memory_per_gb_hour as f64 * discount) as u64;
        }
        
        pricing
    }
}
```

---

## Reputation System

### Reputation Model

```rust
// mycelial/src/reputation.rs

/// Reputation score for a peer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reputation {
    /// Peer ID
    pub peer_id: PeerId,
    
    /// Current reputation score (0-1000)
    pub score: u32,
    
    /// Component scores
    pub components: ReputationComponents,
    
    /// History of reputation changes
    pub history: Vec<ReputationEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReputationComponents {
    /// Uptime reliability (0-100)
    pub uptime: u32,
    
    /// Workload completion rate (0-100)
    pub completion_rate: u32,
    
    /// Resource honesty (0-100) - do they provide what they claim?
    pub resource_honesty: u32,
    
    /// Response time (0-100)
    pub responsiveness: u32,
    
    /// Age/tenure bonus (0-100)
    pub tenure: u32,
    
    /// Vouches from other reputable peers (0-100)
    pub social_trust: u32,
}

impl Reputation {
    pub fn calculate_score(&self) -> u32 {
        let c = &self.components;
        
        // Weighted average
        let weighted = 
            c.uptime as f64 * 0.25 +           // 25%
            c.completion_rate as f64 * 0.25 +   // 25%
            c.resource_honesty as f64 * 0.20 +  // 20%
            c.responsiveness as f64 * 0.10 +    // 10%
            c.tenure as f64 * 0.10 +            // 10%
            c.social_trust as f64 * 0.10;       // 10%
        
        (weighted * 10.0) as u32 // Scale to 0-1000
    }
    
    /// Is this peer trustworthy enough for critical workloads?
    pub fn is_trusted(&self) -> bool {
        self.score >= 700
    }
    
    /// Is this peer acceptable for any workloads?
    pub fn is_acceptable(&self) -> bool {
        self.score >= 300
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReputationEvent {
    pub timestamp: DateTime<Utc>,
    pub event_type: ReputationEventType,
    pub score_delta: i32,
    pub details: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ReputationEventType {
    /// Successfully completed a workload
    WorkloadCompleted { workload_id: WorkloadId },
    
    /// Failed to complete a workload
    WorkloadFailed { workload_id: WorkloadId, reason: String },
    
    /// Node went offline unexpectedly
    UnexpectedOffline { duration_secs: u64 },
    
    /// Node came back online after maintenance
    CameOnline,
    
    /// Received vouch from another peer
    ReceivedVouch { from: PeerId, weight: u32 },
    
    /// Tenure milestone reached
    TenureMilestone { months: u32 },
    
    /// Resource verification passed
    ResourceVerified,
    
    /// Resource verification failed
    ResourceMismatch { claimed: String, actual: String },
}
```

### Reputation Updates

```rust
impl Reputation {
    pub fn apply_event(&mut self, event: ReputationEvent) {
        match &event.event_type {
            ReputationEventType::WorkloadCompleted { .. } => {
                self.components.completion_rate = 
                    (self.components.completion_rate + 5).min(100);
            }
            ReputationEventType::WorkloadFailed { .. } => {
                self.components.completion_rate = 
                    self.components.completion_rate.saturating_sub(10);
            }
            ReputationEventType::UnexpectedOffline { duration_secs } => {
                let penalty = (*duration_secs / 3600) as u32; // -1 per hour
                self.components.uptime = 
                    self.components.uptime.saturating_sub(penalty);
            }
            ReputationEventType::ReceivedVouch { weight, .. } => {
                self.components.social_trust = 
                    (self.components.social_trust + weight / 10).min(100);
            }
            // ... other events
        }
        
        self.history.push(event);
        self.score = self.calculate_score();
    }
}
```

---

## Credit-Aware Scheduling

### Scheduler Integration

```rust
// scheduler/src/credit_scheduler.rs

pub struct CreditAwareScheduler {
    credit_service: Arc<CreditService>,
    reputation_service: Arc<ReputationService>,
    pricing: PricingConfig,
}

impl Scheduler for CreditAwareScheduler {
    async fn schedule(
        &self,
        workload: &WorkloadSpec,
        nodes: &[NodeInfo],
    ) -> Result<SchedulingDecision, SchedulerError> {
        let owner = &workload.owner;
        
        // 1. Check credit balance
        let estimated_cost = estimate_hourly_cost(workload, &self.pricing) * 24; // 24h reserve
        let account = self.credit_service.get_account(owner).await?;
        
        if account.available() < estimated_cost {
            return Err(SchedulerError::InsufficientCredits {
                required: estimated_cost,
                available: account.available(),
            });
        }
        
        // 2. Filter nodes by reputation (user's policy)
        let min_reputation = workload.policy.min_node_reputation.unwrap_or(300);
        let eligible_nodes: Vec<_> = nodes.iter()
            .filter(|n| {
                let rep = self.reputation_service.get(&n.operator_id);
                rep.score >= min_reputation
            })
            .collect();
        
        if eligible_nodes.is_empty() {
            return Err(SchedulerError::NoEligibleNodes);
        }
        
        // 3. Score nodes (reputation + capacity + price)
        let mut scored: Vec<_> = eligible_nodes.iter()
            .map(|n| {
                let rep = self.reputation_service.get(&n.operator_id);
                let price_factor = self.node_price_factor(n);
                let capacity_factor = self.node_capacity_factor(n, workload);
                
                let score = 
                    rep.score as f64 * 0.4 +        // 40% reputation
                    capacity_factor * 300.0 +        // 30% capacity
                    (1.0 - price_factor) * 300.0;    // 30% price (lower = better)
                
                (n, score)
            })
            .collect();
        
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        
        // 4. Reserve credits
        self.credit_service.reserve(owner, estimated_cost).await?;
        
        // 5. Return top node
        Ok(SchedulingDecision {
            node_id: scored[0].0.id.clone(),
            reserved_credits: estimated_cost,
        })
    }
}
```

---

## API Endpoints

### Credit API

```
┌─────────────────────────────────────────────────────────────────────────┐
│                       CREDIT API ENDPOINTS                               │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│  GET  /api/v1/credits/balance                                           │
│       Returns your current credit balance                                │
│                                                                          │
│  GET  /api/v1/credits/account                                           │
│       Returns full account details including history                     │
│                                                                          │
│  GET  /api/v1/credits/transactions                                      │
│       Returns transaction history with pagination                        │
│                                                                          │
│  POST /api/v1/credits/transfer                                          │
│       Transfer credits to another peer                                   │
│       Body: { "to": "<peer_id>", "amount": 1000 }                       │
│                                                                          │
│  GET  /api/v1/credits/estimate?workload=<spec>                          │
│       Estimate cost for a workload                                       │
│                                                                          │
│  GET  /api/v1/reputation/:peer_id                                       │
│       Get reputation score for a peer                                    │
│                                                                          │
│  GET  /api/v1/reputation/leaderboard                                    │
│       Top nodes by reputation                                            │
│                                                                          │
│  POST /api/v1/reputation/vouch                                          │
│       Vouch for another peer                                             │
│       Body: { "peer_id": "<peer_id>", "comment": "..." }                │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

### Response Examples

```json
// GET /api/v1/credits/balance
{
  "balance": 10000,
  "reserved": 2500,
  "available": 7500
}

// GET /api/v1/credits/account
{
  "owner": "peer_abc123",
  "balance": 10000,
  "reserved": 2500,
  "total_earned": 50000,
  "total_spent": 40000,
  "created_at": "2025-01-01T00:00:00Z",
  "last_activity": "2025-12-18T12:00:00Z"
}

// GET /api/v1/reputation/peer_xyz789
{
  "peer_id": "peer_xyz789",
  "score": 850,
  "components": {
    "uptime": 95,
    "completion_rate": 92,
    "resource_honesty": 100,
    "responsiveness": 88,
    "tenure": 60,
    "social_trust": 45
  },
  "is_trusted": true,
  "is_acceptable": true
}
```

---

## CLI Integration

```bash
# Check balance
orch credits balance
# Balance: 10,000 credits
# Reserved: 2,500 credits  
# Available: 7,500 credits

# View account details
orch credits account
# Account: peer_abc123
# Balance: 10,000 | Reserved: 2,500 | Available: 7,500
# Total Earned: 50,000 | Total Spent: 40,000
# Member since: Jan 1, 2025

# Transfer credits
orch credits send peer_xyz789 1000
# ✓ Transferred 1,000 credits to peer_xyz789

# View transaction history
orch credits history --limit 10
# DATE          TYPE      AMOUNT   BALANCE  REFERENCE
# 2025-12-18    CONSUME   -500     10,000   workload/nginx
# 2025-12-17    EARN      +2,000   10,500   node-uptime
# 2025-12-16    TRANSFER  -1,000   8,500    to:peer_xyz789

# Estimate workload cost
orch credits estimate --image nginx --replicas 3 --cpu 0.5 --memory 256
# Estimated cost: 150 credits/hour
# 24h reservation: 3,600 credits

# View reputation
orch reputation peer_xyz789
# Peer: peer_xyz789
# Score: 850/1000 (TRUSTED)
# Uptime: 95% | Completion: 92% | Honesty: 100%

# Vouch for a peer
orch reputation vouch peer_xyz789 --comment "Reliable node operator"
# ✓ Vouched for peer_xyz789
```

---

## Dashboard Integration

```
┌─────────────────────────────────────────────────────────────────────────┐
│                      DASHBOARD: CREDITS VIEW                             │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│  ┌─────────────────────────────────────────────────────────────────┐    │
│  │  YOUR CREDITS                                         TRANSFER ▶│    │
│  ├─────────────────────────────────────────────────────────────────┤    │
│  │                                                                  │    │
│  │     10,000                2,500               7,500             │    │
│  │     BALANCE              RESERVED            AVAILABLE          │    │
│  │                                                                  │    │
│  │  ═══════════════════════════════════════════════════════════   │    │
│  │                                                                  │    │
│  │  SPENDING (24h)          EARNING (24h)       NET                │    │
│  │  ▼ 500                   ▲ 2,000             ▲ 1,500            │    │
│  │                                                                  │    │
│  └─────────────────────────────────────────────────────────────────┘    │
│                                                                          │
│  RECENT TRANSACTIONS                                                    │
│  ─────────────────────────────────────────────────────────────────────  │
│  │ 12:00  CONSUME   -500    nginx deployment                      │    │
│  │ 08:00  EARN      +2,000  node uptime reward                    │    │
│  │ Yesterday  TRANSFER  -1,000  sent to alice                     │    │
│                                                                          │
│  TOP NODES BY REPUTATION                                                │
│  ─────────────────────────────────────────────────────────────────────  │
│  │ 1. node_alpha    ████████████████████ 950  TRUSTED             │    │
│  │ 2. node_beta     ██████████████████░░ 880  TRUSTED             │    │
│  │ 3. node_gamma    ████████████████░░░░ 820  TRUSTED             │    │
│  │ 4. node_delta    ██████████████░░░░░░ 720  TRUSTED             │    │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## Implementation Phases

### Phase 1: Basic Credits (Week 1)

- [ ] CreditAccount data structure
- [ ] Credit ledger (in-memory, replicated via Raft)
- [ ] Basic operations: balance, reserve, consume, release
- [ ] API endpoints for balance and transfer
- [ ] CLI commands

### Phase 2: Metering (Week 2)

- [ ] Resource metering per workload
- [ ] Periodic credit consumption
- [ ] Node earnings calculation
- [ ] Transaction history

### Phase 3: Reputation (Week 2-3)

- [ ] Reputation data model
- [ ] Event-based reputation updates
- [ ] API endpoints
- [ ] Integration with scheduler

### Phase 4: Advanced Features (Week 3-4)

- [ ] Dynamic pricing
- [ ] Vouch system
- [ ] Credit lines (peer-to-peer credit)
- [ ] Dashboard views

---

## Dependencies

```toml
[dependencies]
# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# Time handling
chrono = { version = "0.4", features = ["serde"] }

# No external dependencies for core credit logic!
# Keeping it simple and auditable
```

---

## Security Considerations

### Double-Spend Prevention

All credit operations go through Raft consensus:
- Only leader accepts writes
- Log replication ensures all nodes agree
- No credit can be spent twice

### Sybil Resistance

New accounts start with:
- Zero or minimal credits (must earn or receive)
- Low reputation (must build over time)
- Rate-limited transfers

### Reputation Gaming

Protections:
- Vouch weight scales with voucher's reputation
- Self-vouching impossible (can't vouch for yourself)
- Suspicious patterns flagged (cluster of vouches, sudden changes)

---

## Success Criteria

1. Credit balance reflects resource consumption accurately
2. Node operators earn credits proportional to resources provided
3. Workloads can't run without sufficient credits
4. Reputation accurately reflects node reliability
5. No double-spending or credit creation exploits
6. Dashboard shows real-time credit and reputation data

---

## References

- [Stellar Consensus Protocol](https://www.stellar.org/papers/stellar-consensus-protocol) - Inspiration for decentralized agreement
- [EigenTrust](https://nlp.stanford.edu/pubs/eigentrust.pdf) - P2P reputation algorithm
- [Filecoin Economics](https://filecoin.io/blog/filecoin-economics/) - Resource market inspiration

---

*Mycelial Economics v0.1.0*  
*Building a self-sustaining resource economy*
