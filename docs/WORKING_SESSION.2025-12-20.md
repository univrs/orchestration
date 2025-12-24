# UNIVRS UNIFICATION: Claude-Flow Execution Plan

## Overview

This plan creates the unified identity and storage layers for Univrs.
Execute these commands in order, waiting for each to complete before the next.

---

## PHASE 1: Create univrs-identity Crate

### Step 1.1: Create the repository and crate structure

```bash
npx claude-flow@alpha swarm \
  "Create a new Rust crate at ~/repos/univrs-identity with the following:

  1. Initialize git repo and Cargo.toml:
     - name = 'univrs-identity'
     - version = '0.1.0'
     - edition = '2021'
     - authors = ['Univrs.io Contributors']
     - license = 'MIT'
     - description = 'Unified Ed25519 cryptographic identity for Univrs ecosystem'
  
  2. Dependencies in Cargo.toml:
     - ed25519-dalek = { version = '2.1', features = ['serde', 'rand_core'] }
     - rand = '0.8'
     - serde = { version = '1.0', features = ['derive'] }
     - serde_json = '1.0'
     - bs58 = '0.5'
     - thiserror = '1.0'
     - zeroize = { version = '1.7', features = ['derive'] }
     - chacha20poly1305 = '0.10'
     - scrypt = '0.11'
     - base64 = '0.21'
  
  3. Create src/lib.rs with modules:
     - pub mod keypair;
     - pub mod signature;
     - pub mod storage;
     - pub mod claims;
     - pub mod error;
  
  4. Reference the DOL spec at ~/repos/RustOrchestration/ontology/prospective/identity.dol
     for the contracts each module must fulfill.
  
  5. Create .gitignore for Rust project.
  
  LIST ALL CREATED FILES with full paths." \
  --claude
```

### Step 1.2: Implement core modules

```bash
npx claude-flow@alpha swarm \
  "In ~/repos/univrs-identity/src/, implement the following modules:

  1. error.rs:
     - IdentityError enum with variants:
       InvalidSignature, InvalidPublicKey, DecryptionFailed,
       EncryptionFailed, IoError, SerializationError
     - Implement std::error::Error
  
  2. keypair.rs:
     - Keypair struct wrapping ed25519_dalek::SigningKey
     - Keypair::generate() -> Self using OsRng
     - Keypair::from_bytes(secret: &[u8; 32]) -> Result<Self>
     - Keypair::public_key(&self) -> PublicKey
     - Keypair::sign(&self, message: &[u8]) -> Signature
     - Implement Zeroize for Keypair (secure memory cleanup)
  
  3. signature.rs:
     - PublicKey newtype around ed25519_dalek::VerifyingKey
     - Signature newtype around ed25519_dalek::Signature
     - PublicKey::verify(&self, message: &[u8], sig: &Signature) -> bool
     - PublicKey::to_base58(&self) -> String
     - PublicKey::from_base58(s: &str) -> Result<Self>
     - PublicKey::to_peer_id(&self) -> String (first 8 chars of base58)
     - Implement Serialize/Deserialize for both
  
  4. Add comprehensive tests for each module.
  
  Run 'cargo build' and 'cargo test' to verify.
  LIST ALL CREATED FILES." \
  --claude
```

### Step 1.3: Implement storage and claims

```bash
npx claude-flow@alpha swarm \
  "In ~/repos/univrs-identity/src/, implement:

  1. storage.rs:
     - StoredKeypair struct with:
       encrypted_private_key: Vec<u8>
       public_key: PublicKey
       nonce: [u8; 12]
       salt: [u8; 32]
     - Keypair::save(&self, path: &Path, passphrase: &str) -> Result<()>
       Use scrypt for key derivation, chacha20poly1305 for encryption
     - Keypair::load(path: &Path, passphrase: &str) -> Result<Self>
     - Write atomically (write to .tmp, then rename)
  
  2. claims.rs:
     - SignedClaim struct:
       signer: PublicKey
       subject: String
       predicate: String  
       timestamp: u64 (unix millis)
       signature: Signature
     - Keypair::make_claim(&self, subject: &str, predicate: &str) -> SignedClaim
     - SignedClaim::verify(&self) -> bool
     - Implement Serialize/Deserialize
  
  3. Update lib.rs with re-exports:
     pub use keypair::Keypair;
     pub use signature::{PublicKey, Signature};
     pub use claims::SignedClaim;
     pub use error::IdentityError;
  
  4. Add integration tests in tests/integration.rs:
     - Test generate, save, load roundtrip
     - Test claim creation and verification
     - Test wrong passphrase fails
  
  Run 'cargo test' to verify all pass.
  Run 'cargo doc --open' to verify documentation." \
  --claude
```

---

## PHASE 2: Create univrs-state Crate

### Step 2.1: Create the repository and crate structure

```bash
npx claude-flow@alpha swarm \
  "Create a new Rust crate at ~/repos/univrs-state with the following:

  1. Initialize git repo and Cargo.toml:
     - name = 'univrs-state'
     - version = '0.1.0'
     - edition = '2021'
     - authors = ['Univrs.io Contributors']
     - license = 'MIT'
     - description = 'Unified state storage abstraction for Univrs ecosystem'
  
  2. Dependencies in Cargo.toml:
     - tokio = { version = '1', features = ['full'] }
     - async-trait = '0.1'
     - serde = { version = '1.0', features = ['derive'] }
     - serde_json = '1.0'
     - thiserror = '1.0'
     - sqlx = { version = '0.8', features = ['sqlite', 'runtime-tokio'] }
     - parking_lot = '0.12'
     - tokio-stream = '0.1'
     - futures = '0.3'
     - tracing = '0.1'
  
  3. Create src/lib.rs with modules:
     - pub mod store;      // StateStore trait
     - pub mod sqlite;     // SQLite implementation
     - pub mod memory;     // In-memory implementation
     - pub mod error;      // Error types
     - pub mod watch;      // Watch/notification types
  
  4. Create migrations/ directory for SQLite schema.
  
  5. Reference the DOL spec at ~/repos/RustOrchestration/ontology/prospective/storage.dol
  
  LIST ALL CREATED FILES with full paths." \
  --claude
```

### Step 2.2: Implement StateStore trait and types

```bash
npx claude-flow@alpha swarm \
  "In ~/repos/univrs-state/src/, implement:

  1. error.rs:
     - StateError enum: KeyNotFound, VersionConflict, AlreadyExists,
       TransactionFailed, ConnectionError, StorageFull, SqlError
     - Implement std::error::Error
  
  2. watch.rs:
     - WatchEventType enum: Created, Updated, Deleted
     - WatchEvent struct: key, event_type, old_value, new_value, version
     - WatchStream type alias for async stream
  
  3. store.rs:
     - Entry struct: key, value (Vec<u8>), version (u64), created_at, updated_at
     - TxOp enum: Set(key, value), Delete(key), CheckVersion(key, version)
     - StateStore trait (async_trait):
       async fn get(&self, key: &str) -> Result<Option<Entry>>
       async fn set(&self, key: &str, value: Vec<u8>) -> Result<u64>
       async fn delete(&self, key: &str) -> Result<()>
       async fn list(&self, prefix: &str) -> Result<Vec<String>>
       async fn compare_and_set(&self, key: &str, version: u64, value: Vec<u8>) -> Result<u64>
       async fn transaction(&self, ops: Vec<TxOp>) -> Result<()>
       async fn watch(&self, prefix: &str) -> Result<WatchStream>
  
  4. Add tests for trait definitions.
  
  Run 'cargo build' to verify." \
  --claude
```

### Step 2.3: Implement SQLite backend

```bash
npx claude-flow@alpha swarm \
  "In ~/repos/univrs-state/, implement SQLite backend:

  1. Create migrations/001_initial.sql:
     CREATE TABLE IF NOT EXISTS kv_store (
       key TEXT PRIMARY KEY,
       value BLOB NOT NULL,
       version INTEGER NOT NULL DEFAULT 1,
       created_at INTEGER NOT NULL,
       updated_at INTEGER NOT NULL
     );
     CREATE INDEX idx_kv_prefix ON kv_store(key);
  
  2. Implement src/sqlite.rs:
     - SqliteStore struct with SqlitePool
     - SqliteStore::new(path: &str) -> Result<Self>
     - SqliteStore::memory() -> Result<Self> (for testing)
     - SqliteStore::run_migrations(&self) -> Result<()>
     - Implement StateStore trait for SqliteStore
     - Use sqlx for all operations
     - Implement watch using tokio broadcast channel
  
  3. Implement src/memory.rs:
     - MemoryStore struct with RwLock<HashMap>
     - Implement StateStore trait
     - Use tokio broadcast for watch
  
  4. Update lib.rs with re-exports:
     pub use store::{StateStore, Entry, TxOp};
     pub use sqlite::SqliteStore;
     pub use memory::MemoryStore;
     pub use error::StateError;
     pub use watch::{WatchEvent, WatchEventType};
  
  5. Add integration tests:
     - CRUD operations
     - Transactions
     - Version conflicts
     - Watch notifications
  
  Run 'cargo test' to verify." \
  --claude
```

---

## PHASE 3: Integrate into Orchestrator

### Step 3.1: Add dependencies

```bash
npx claude-flow@alpha swarm \
  "In ~/repos/RustOrchestration/ai_native_orchestrator/:

  1. Add to workspace Cargo.toml [workspace.dependencies]:
     univrs-identity = { path = '../../univrs-identity' }
     univrs-state = { path = '../../univrs-state' }
  
  2. Update orchestrator_shared_types/Cargo.toml:
     Add univrs-identity dependency
  
  3. Update state_store_interface/Cargo.toml:
     Add univrs-state dependency
  
  4. Update orchestrator_core/Cargo.toml:
     Add univrs-identity and univrs-state dependencies
  
  Run 'cargo build' to verify dependencies resolve." \
  --claude
```

### Step 3.2: Migrate NodeId to Ed25519

```bash
npx claude-flow@alpha swarm \
  "In ~/repos/RustOrchestration/ai_native_orchestrator/:

  1. In orchestrator_shared_types/src/lib.rs:
     - Import univrs_identity::{PublicKey, Keypair}
     - Change NodeId from UUID to PublicKey wrapper:
       #[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
       pub struct NodeId(pub univrs_identity::PublicKey);
     - Add NodeId::generate() -> (NodeId, Keypair)
     - Add NodeId::from_public_key(pk: PublicKey) -> Self
     - Add NodeId::to_base58(&self) -> String
  
  2. Update all code that creates NodeId:
     - orchestrator_core/src/main.rs
     - orchestrator_core/src/bin/node.rs
     - cluster_manager/src/chitchat_manager.rs
     - Any tests that create NodeIds
  
  3. Ensure Serialize/Deserialize work with base58 encoding.
  
  Run 'cargo build' and 'cargo test'.
  Report which files were modified." \
  --claude
```

### Step 3.3: Add SQLite StateStore

```bash
npx claude-flow@alpha swarm \
  "In ~/repos/RustOrchestration/ai_native_orchestrator/:

  1. In state_store_interface/src/lib.rs:
     - Re-export univrs_state types
     - Keep existing StateStore trait if different, or migrate to univrs_state::StateStore
  
  2. Create state_store_interface/src/sqlite.rs:
     - Wrapper around univrs_state::SqliteStore
     - Implement any orchestrator-specific methods
  
  3. Update orchestrator_core/src/main.rs:
     - Add CLI flag: --state-backend [memory|sqlite]
     - Add CLI flag: --state-path (for sqlite file location)
     - Default to sqlite with path './orchestrator.db'
     - Initialize SqliteStore and run migrations on startup
  
  4. Update orchestrator_core/src/api/state.rs:
     - Accept Arc<dyn StateStore> instead of concrete type
  
  Run 'cargo build' and 'cargo test'.
  Test with: cargo run -- --state-backend sqlite --state-path test.db" \
  --claude
```

---

## PHASE 4: Integrate into Mycelial-Dashboard

### Step 4.1: Add univrs-identity dependency

```bash
npx claude-flow@alpha swarm \
  "In ~/repos/mycelial-dashboard/:

  1. Add to workspace Cargo.toml [workspace.dependencies]:
     univrs-identity = { path = '../univrs-identity' }
  
  2. Update crates/mycelial-core/Cargo.toml:
     Replace ed25519-dalek direct dependency with univrs-identity
  
  3. Update crates/mycelial-core/src/peer.rs:
     - Use univrs_identity::PublicKey for PeerId
     - Use univrs_identity::Keypair for key generation
     - Remove duplicate crypto code
  
  4. Update crates/mycelial-network/src/lib.rs:
     - Use univrs_identity for keypair management
     - Derive libp2p PeerId from univrs_identity::PublicKey
  
  Run 'cargo build' and 'cargo test'.
  Report which files were modified." \
  --claude
```

### Step 4.2: Migrate to univrs-state

```bash
npx claude-flow@alpha swarm \
  "In ~/repos/mycelial-dashboard/:

  1. Add to workspace Cargo.toml [workspace.dependencies]:
     univrs-state = { path = '../univrs-state' }
  
  2. In crates/mycelial-state/src/lib.rs:
     - Consider whether to migrate to univrs-state or keep separate
     - If migrating: re-export univrs_state types
     - If keeping: ensure compatible trait definitions
  
  3. Document the decision in crates/mycelial-state/README.md
  
  Run 'cargo build' and 'cargo test'." \
  --claude
```

---

## PHASE 5: Copy DOL Specs

### Step 5.1: Add specs to metadol

```bash
npx claude-flow@alpha swarm \
  "Copy the DOL specs to the metadol repository:

  1. Copy identity.dol to ~/repos/metadol/docs/ontology/prospective/identity/
     - Create systems/identity.dol
     - Create genes/keypair.dol (extract gene definitions)
     - Create traits/sign.dol (extract trait definitions)
     - Create constraints/uniqueness.dol (extract constraints)
  
  2. Copy storage.dol to ~/repos/metadol/docs/ontology/prospective/storage/
     - Create systems/storage.dol
     - Create genes/entry.dol
     - Create traits/operations.dol
     - Create constraints/durability.dol
  
  3. Run dol-parse on all new files to validate.
  
  4. Update docs/ontology/prospective/README.md with new domains.
  
  LIST ALL CREATED FILES." \
  --claude
```

---

## Verification Commands

After completing all phases, run these verification commands:

```bash
# Verify univrs-identity
cd ~/repos/univrs-identity && cargo test && cargo doc

# Verify univrs-state
cd ~/repos/univrs-state && cargo test && cargo doc

# Verify orchestrator integration
cd ~/repos/RustOrchestration/ai_native_orchestrator && cargo test --all

# Verify mycelial-dashboard integration
cd ~/repos/mycelial-dashboard && cargo test --all

# Verify DOL specs
cd ~/repos/metadol && cargo run --bin dol-parse -- docs/ontology/prospective/identity/*.dol
cd ~/repos/metadol && cargo run --bin dol-parse -- docs/ontology/prospective/storage/*.dol
```

---

## Summary

| Phase | Creates | Purpose |
|-------|---------|---------|
| 1 | univrs-identity | Unified Ed25519 identity |
| 2 | univrs-state | Unified storage abstraction |
| 3 | - | Orchestrator uses both |
| 4 | - | Mycelial uses identity |
| 5 | - | DOL specs in metadol |

Total estimated time: 2-3 hours with claude-flow

---

## Notes for Execution

1. **Wait for each step to complete** before proceeding
2. **Check cargo test passes** at each checkpoint
3. **If a step fails**, share the error output for analysis
4. **Path references must be exact** - claude-flow needs absolute paths
5. **Back up repos** before major changes (git commit current state)
