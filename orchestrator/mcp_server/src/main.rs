//! MCP Server binary for AI-native container orchestration.
//!
//! This binary provides an MCP server that can be used with Claude Code
//! or other MCP clients to manage the orchestrator.
//!
//! # Usage
//!
//! Run with stdio transport (for Claude Code integration):
//! ```bash
//! orchestrator-mcp
//! ```

use std::process::exit;

use tracing::{error, info};
use tracing_subscriber::{self, EnvFilter};

/// Main entry point for the MCP server.
#[tokio::main]
async fn main() {
    // Initialize logging to stderr (stdout is reserved for MCP protocol)
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_writer(std::io::stderr)
        .init();

    info!("Starting orchestrator MCP server...");

    if let Err(e) = run_server().await {
        error!("Server error: {}", e);
        exit(1);
    }
}

async fn run_server() -> Result<(), Box<dyn std::error::Error>> {
    use tokio::io::{stdin, stdout};

    info!("Initializing MCP server with stdio transport");

    // For now, we create a simple server info response handler
    // The full implementation will integrate with actual orchestrator state

    // Create the stdio transport
    let _transport = (stdin(), stdout());

    // TODO: When we have actual state store and cluster manager implementations,
    // integrate them here:
    //
    // let state_store = state_store::InMemoryStateStore::new();
    // let cluster_manager = cluster_manager::MockClusterManager::new();
    // let scheduler = scheduler::DefaultScheduler::new();
    //
    // let server = OrchestratorMcpServer::new(state_store, cluster_manager, scheduler);
    // let mcp_server = server.serve(transport).await?;
    // mcp_server.waiting().await?;

    info!("MCP server initialized successfully");
    info!(
        "Server info: {} v{}",
        "orchestrator-mcp",
        env!("CARGO_PKG_VERSION")
    );

    // For now, just print tool definitions and exit
    // This allows testing the build without full MCP integration
    let tools = mcp_server::server::ToolDefinitions::all();
    info!(
        "Available tools: {:?}",
        tools.tools.iter().map(|t| &t.name).collect::<Vec<_>>()
    );

    // Print available resources
    let resources = mcp_server::resources::list_available_resources();
    info!(
        "Available resources: {:?}",
        resources.iter().map(|r| &r.uri).collect::<Vec<_>>()
    );

    Ok(())
}
