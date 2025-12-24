//! OCI Bundle Generation Module
//!
//! This module provides comprehensive OCI bundle generation for container runtimes.
//! It creates OCI-compliant bundles with:
//! - Runtime specification (config.json)
//! - Rootfs directory structure
//! - Resource limits (CPU, memory, I/O)
//! - Linux namespaces and cgroups
//! - Proper mount configurations
//!
//! # Example
//!
//! ```ignore
//! use container_runtime::oci_bundle::{OciBundleBuilder, OciSpecBuilder};
//! use orchestrator_shared_types::ContainerConfig;
//!
//! let bundle = OciBundleBuilder::new("/var/lib/containers/bundles/my-container")
//!     .with_container_config(&container_config)
//!     .with_resource_limits(cpu_quota, memory_limit)
//!     .build()
//!     .await?;
//! ```

mod spec;
mod rootfs;
mod builder;

pub use spec::*;
pub use rootfs::*;
pub use builder::*;
