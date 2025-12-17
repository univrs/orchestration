//! MCP Tools for orchestrator operations.
//!
//! Tools are executable functions that AI agents can invoke to perform actions
//! on the orchestrator. Each tool has a defined schema and performs side effects.

pub mod workload;
pub mod node;
pub mod cluster;

pub use workload::*;
pub use node::*;
pub use cluster::*;
