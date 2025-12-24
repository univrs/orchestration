//! Cluster Manager implementations for the orchestrator.
//!
//! This crate provides implementations of the `ClusterManager` trait:
//! - `MockClusterManager`: In-memory mock for testing (default)
//! - `ChitchatClusterManager`: Real gossip-based cluster manager using chitchat (requires `chitchat-cluster` feature)

#[cfg(feature = "mock-cluster")]
pub mod mock;

#[cfg(feature = "chitchat-cluster")]
pub mod chitchat_manager;

// Re-export common types
pub use cluster_manager_interface::{ClusterEvent, ClusterManager, ClusterManagerError};

#[cfg(feature = "mock-cluster")]
pub use mock::MockClusterManager;

#[cfg(feature = "chitchat-cluster")]
pub use chitchat_manager::ChitchatClusterManager;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        // Basic sanity test
        assert!(true);
    }
}
