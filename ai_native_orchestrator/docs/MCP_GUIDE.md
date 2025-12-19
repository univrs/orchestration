# MCP Integration Guide for AI-Native Orchestrator

This guide explains how to set up and use the Model Context Protocol (MCP) server to enable AI agents like Claude Code to interact with the AI-Native Container Orchestrator.

## What is MCP?

The [Model Context Protocol](https://modelcontextprotocol.io/) (MCP) is an open standard developed by Anthropic that enables AI systems to securely interact with external tools and data sources. Our orchestrator exposes its functionality through MCP, allowing AI agents to:

- Create, scale, and manage container workloads
- Monitor cluster health and node status
- Query resource utilization and scheduling decisions
- Perform cluster-wide operations

## Prerequisites

- Rust toolchain (1.75+)
- Claude Code CLI or another MCP-compatible client
- The orchestrator workspace built successfully

## Building the MCP Server

```bash
cd ai_native_orchestrator
cargo build -p mcp_server --release
```

The binary will be available at `target/release/orchestrator-mcp`.

## Setting Up Claude Code Integration

### Step 1: Locate Your Claude Code Configuration

Claude Code stores its MCP server configuration in:

- **macOS/Linux**: `~/.claude/claude_desktop_config.json`
- **Windows**: `%APPDATA%\Claude\claude_desktop_config.json`

### Step 2: Add the Orchestrator MCP Server

Edit your configuration file to add the orchestrator server:

```json
{
  "mcpServers": {
    "orchestrator": {
      "command": "/path/to/ai_native_orchestrator/target/release/orchestrator-mcp",
      "args": [],
      "env": {
        "RUST_LOG": "info"
      }
    }
  }
}
```

Replace `/path/to/ai_native_orchestrator` with the actual path to your project.

### Step 3: Restart Claude Code

After saving the configuration, restart Claude Code to load the new MCP server.

### Step 4: Verify the Integration

In Claude Code, you can verify the server is loaded by asking:

> "What orchestrator tools are available?"

Claude should list the available MCP tools from the orchestrator.

## Available MCP Tools

### Workload Management

| Tool | Description | Parameters |
|------|-------------|------------|
| `list_workloads` | List all workloads in the cluster | `name_filter` (optional) |
| `create_workload` | Create a new container workload | `name`, `replicas`, `image`, `cpu_cores`, `memory_mb`, `ports`, `env_vars` |
| `get_workload` | Get detailed workload information | `workload_id` |
| `scale_workload` | Scale workload to new replica count | `workload_id`, `replicas` |
| `delete_workload` | Delete a workload and its instances | `workload_id` |

### Node Management

| Tool | Description | Parameters |
|------|-------------|------------|
| `list_nodes` | List cluster nodes with optional filters | `status_filter`, `label_key`, `label_value` |
| `get_node` | Get detailed node information | `node_id` |
| `cordon_node` | Prevent new workloads on a node | `node_id` |
| `uncordon_node` | Allow new workloads on a node | `node_id` |
| `drain_node` | Evict all workloads from a node | `node_id`, `grace_period_seconds`, `force` |
| `label_node` | Add or update a node label | `node_id`, `key`, `value` |

### Cluster Operations

| Tool | Description | Parameters |
|------|-------------|------------|
| `get_cluster_status` | Get overall cluster health and metrics | `include_nodes`, `include_workloads` |
| `get_cluster_events` | Retrieve recent cluster events | `limit`, `event_type`, `resource_id` |
| `analyze_scheduling` | Analyze scheduling status and get recommendations | `workload_id`, `include_what_if` |

## MCP Resources

The orchestrator also exposes read-only resources via MCP:

| Resource URI | Description |
|--------------|-------------|
| `orchestrator://nodes` | List of all cluster nodes |
| `orchestrator://nodes/{id}` | Specific node details |
| `orchestrator://workloads` | List of all workloads |
| `orchestrator://workloads/{id}` | Specific workload details |
| `orchestrator://cluster/status` | Cluster status summary |
| `orchestrator://cluster/metrics` | Resource utilization metrics |

## Example Usage with Claude Code

### Creating a Workload

> "Create a new nginx workload with 3 replicas, 0.5 CPU cores, and 256MB memory"

Claude will use the `create_workload` tool:

```json
{
  "name": "nginx-web",
  "replicas": 3,
  "image": "nginx:latest",
  "cpu_cores": 0.5,
  "memory_mb": 256,
  "ports": [{"container_port": 80, "protocol": "tcp"}]
}
```

### Scaling a Workload

> "Scale the nginx-web workload to 5 replicas"

### Checking Cluster Health

> "What's the current cluster status? Are all nodes healthy?"

### Draining a Node for Maintenance

> "Drain node abc123 with a 60 second grace period for maintenance"

## Roadmap: Planned MCP Tools

As the AI-Native Orchestrator develops, we plan to add these additional MCP capabilities:

### Phase 2: Advanced Scheduling

| Tool | Description |
|------|-------------|
| `set_node_affinity` | Configure workload affinity rules |
| `set_anti_affinity` | Configure anti-affinity constraints |
| `set_resource_limits` | Define resource limits and requests |
| `prioritize_workload` | Set workload scheduling priority |

### Phase 3: Observability

| Tool | Description |
|------|-------------|
| `get_workload_logs` | Retrieve container logs |
| `get_metrics` | Query Prometheus-style metrics |
| `get_events_stream` | Subscribe to real-time events |
| `health_check` | Run health probes on workloads |

### Phase 4: Network & Storage

| Tool | Description |
|------|-------------|
| `create_network_policy` | Define network access rules |
| `create_volume` | Provision persistent storage |
| `attach_volume` | Attach storage to workloads |
| `configure_ingress` | Set up external access |

### Phase 5: Security & RBAC

| Tool | Description |
|------|-------------|
| `create_secret` | Store sensitive configuration |
| `set_security_context` | Configure container security |
| `audit_access` | Review access patterns |
| `rotate_credentials` | Rotate service credentials |

### Phase 6: AI-Native Features

| Tool | Description |
|------|-------------|
| `auto_scale_recommendation` | Get AI-powered scaling suggestions |
| `predict_resource_usage` | Forecast resource needs |
| `optimize_placement` | AI-driven workload placement |
| `anomaly_detection` | Detect unusual cluster behavior |

## Troubleshooting

### Server Not Loading

1. Check the binary path in your config is correct
2. Ensure the binary has execute permissions: `chmod +x orchestrator-mcp`
3. Check logs: `RUST_LOG=debug orchestrator-mcp`

### Connection Issues

1. Verify no other process is using the same stdio pipes
2. Check Claude Code logs for MCP connection errors
3. Try running the server manually to see startup errors

### Tool Errors

1. Ensure the orchestrator state store is initialized
2. Check that node/workload IDs are valid UUIDs
3. Review the error message returned by the tool

## Architecture

```
┌─────────────────┐     ┌──────────────────┐     ┌─────────────────┐
│   Claude Code   │────▶│   MCP Server     │────▶│  Orchestrator   │
│   (MCP Client)  │◀────│ (orchestrator-   │◀────│   Components    │
│                 │     │      mcp)        │     │                 │
└─────────────────┘     └──────────────────┘     └─────────────────┘
        │                       │                        │
        │    JSON-RPC 2.0       │     Rust Async        │
        │    over stdio         │     Interfaces        │
        │                       │                       │
        ▼                       ▼                       ▼
   AI Agent              Tool Handlers           State Store
   Prompts               Resource Access         Cluster Manager
   Context               Schema Generation       Scheduler
```

## Contributing

When adding new MCP tools:

1. Define input/output types in `mcp_server/src/tools/`
2. Implement the handler in `server.rs`
3. Add the tool to `ToolDefinitions::all()`
4. Update this guide with the new tool documentation
5. Add tests for the new functionality

## References

- [MCP Specification](https://spec.modelcontextprotocol.io/)
- [rmcp Rust SDK](https://github.com/modelcontextprotocol/rust-sdk)
- [Claude Code Documentation](https://docs.anthropic.com/claude-code)
