//! MCP Server for AI-Native Container Orchestration
//!
//! This crate provides a Model Context Protocol (MCP) server that exposes
//! the orchestrator's functionality to AI agents. It allows AI systems to:
//!
//! - **Tools**: Execute operations like creating workloads, scaling, managing nodes
//! - **Resources**: Access cluster state, workload definitions, metrics
//! - **Prompts**: Use predefined templates for common orchestration tasks
//!
//! # Architecture
//!
//! The MCP server wraps the orchestrator's state store and exposes it via JSON-RPC 2.0
//! over stdio transport (for Claude Code integration) or HTTP (for remote access).

pub mod resources;
pub mod server;
pub mod tools;

pub use server::OrchestratorMcpServer;
pub use server::{JsonRpcError, JsonRpcRequest, JsonRpcResponse};
pub use server::{INTERNAL_ERROR, INVALID_PARAMS, INVALID_REQUEST, METHOD_NOT_FOUND, PARSE_ERROR};

// Re-export common types
pub use rmcp;
