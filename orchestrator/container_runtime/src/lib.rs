//! Container Runtime implementations for the orchestrator.
//!
//! This crate provides implementations of the `ContainerRuntime` trait:
//! - `MockRuntime`: In-memory mock for testing (default)
//! - `YoukiRuntime`: Real container runtime using libcontainer (requires `youki-runtime` feature)
//! - `YoukiCliRuntime`: CLI-based runtime using youki binary (requires `youki-cli` feature)
//!
//! Additionally, the `oci_bundle` module provides OCI bundle generation utilities
//! that can be used by any OCI-compliant runtime.
//!
//! The `image` module (requires `image-pull` feature) provides image pulling
//! and extraction from Docker Hub and other registries.

pub mod oci_bundle;
pub mod image;

#[cfg(feature = "mock-runtime")]
pub mod mock;

#[cfg(feature = "youki-runtime")]
pub mod youki;

#[cfg(feature = "youki-cli")]
pub mod youki_cli;

// Re-export common types
pub use container_runtime_interface::{
    ContainerRuntime, ContainerStatus, CreateContainerOptions, RuntimeError,
};

pub use image::{ImageManager, ImageReference, ImageError, Manifest};

#[cfg(feature = "mock-runtime")]
pub use mock::MockRuntime;

#[cfg(feature = "youki-runtime")]
pub use youki::YoukiRuntime;

#[cfg(feature = "youki-cli")]
pub use youki_cli::{
    YoukiCliRuntime, YoukiCliConfig, YoukiCliError, YoukiState, ContainerStats,
    LogEntry, LogOptions, LogReceiver,
};

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        // Basic sanity test
        assert!(true);
    }
}
