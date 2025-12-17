//! Container Runtime implementations for the orchestrator.
//!
//! This crate provides implementations of the `ContainerRuntime` trait:
//! - `MockRuntime`: In-memory mock for testing (default)
//! - `YoukiRuntime`: Real container runtime using libcontainer (requires `youki-runtime` feature)

#[cfg(feature = "mock-runtime")]
pub mod mock;

#[cfg(feature = "youki-runtime")]
pub mod youki;

// Re-export common types
pub use container_runtime_interface::{
    ContainerRuntime, ContainerStatus, CreateContainerOptions, RuntimeError,
};

#[cfg(feature = "mock-runtime")]
pub use mock::MockRuntime;

#[cfg(feature = "youki-runtime")]
pub use youki::YoukiRuntime;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        // Basic sanity test
        assert!(true);
    }
}
