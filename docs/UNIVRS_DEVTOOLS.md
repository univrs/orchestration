# Univrs Developer Tools

**SDK, CLI, and API Reference**

Version: 0.2.0  
Last Updated: December 17, 2025

---

## Overview

Univrs provides a layered developer tooling ecosystem designed around user sovereignty and AI-native orchestration. Each layer builds on the one below, enabling developers to interact with the platform at their preferred level of abstraction.

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    DEVELOPER TOOLING LAYERS                              │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│   Layer 4: DASHBOARD                                                    │
│            └── Visual management, monitoring, real-time views           │
│                                                                          │
│   Layer 3: META-APPS & OVERLAYS                                         │
│            └── Templates, generators, IDE plugins                        │
│                                                                          │
│   Layer 2: CLI TOOLS                                                    │
│            ├── orch    (end-user interface)                             │
│            └── uictl   (operator control)                               │
│                                                                          │
│   Layer 1: SDK LIBRARIES                                                │
│            ├── univrs-sdk-rust   (native)                               │
│            ├── univrs-sdk-ts     (TypeScript/Node)                      │
│            └── univrs-sdk-python (Python)                               │
│                                                                          │
│   Layer 0: PROTOCOL & API                                               │
│            ├── MCP - AI agent control                                   │
│            ├── REST - Traditional integration                           │
│            └── WebSocket - Real-time events                             │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## Part I: Protocol & API (Layer 0)

### API Endpoints

All APIs are served from the orchestrator node on port 9090 by default.

| Protocol | Endpoint | Description | Status |
|----------|----------|-------------|--------|
| REST | `/api/v1/*` | CRUD operations | ✅ Available |
| WebSocket | `/api/v1/events` | Real-time event streaming | ✅ Available |
| MCP | stdio | AI agent integration | ✅ Available |
| Health | `/health`, `/ready`, `/live` | Kubernetes-compatible probes | ✅ Available |
| Metrics | `/metrics` | Prometheus format | ✅ Available |

### REST API Reference

#### Workloads

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/api/v1/workloads` | List all workloads |
| `POST` | `/api/v1/workloads` | Create a new workload |
| `GET` | `/api/v1/workloads/:id` | Get workload details |
| `PUT` | `/api/v1/workloads/:id` | Update workload |
| `DELETE` | `/api/v1/workloads/:id` | Delete workload |
| `GET` | `/api/v1/workloads/:id/instances` | List workload instances |

**Create Workload Request:**

```json
{
  "name": "my-app",
  "replicas": 3,
  "containers": [
    {
      "name": "main",
      "image": "myregistry/myapp:v1",
      "ports": [{"containerPort": 8080}],
      "env_vars": {"NODE_ENV": "production"}
    }
  ]
}
```

**Response:**

```json
{
  "id": "a5228e27-aff6-4a26-a61a-eecd16a91b04",
  "name": "my-app",
  "replicas": 3,
  "labels": {},
  "containers": [...]
}
```

#### Nodes

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/api/v1/nodes` | List all nodes |
| `GET` | `/api/v1/nodes/:id` | Get node details |

**Response:**

```json
{
  "items": [
    {
      "id": "00000000-0000-0000-0000-000000000001",
      "address": "172.28.0.2:7280",
      "status": "Ready",
      "resources_capacity": {
        "cpu_cores": 4.0,
        "memory_mb": 4096,
        "disk_mb": 51200
      },
      "resources_allocatable": {
        "cpu_cores": 3.6,
        "memory_mb": 3686,
        "disk_mb": 46080
      }
    }
  ],
  "count": 1
}
```

#### Cluster

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/api/v1/cluster/status` | Cluster overview |

**Response:**

```json
{
  "total_nodes": 2,
  "ready_nodes": 2,
  "not_ready_nodes": 0,
  "total_workloads": 1,
  "total_instances": 2,
  "running_instances": 0,
  "pending_instances": 2,
  "total_cpu_capacity": 6.0,
  "total_memory_mb": 6144
}
```

#### Observability

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/health` | Health check (JSON) |
| `GET` | `/ready` | Readiness probe |
| `GET` | `/live` | Liveness probe |
| `GET` | `/metrics` | Prometheus metrics |

### WebSocket Events

Connect to `ws://localhost:9090/api/v1/events` for real-time event streaming.

**Subscribe to events:**

```json
{"type": "subscribe", "topics": ["workloads", "nodes", "cluster"]}
```

**Event types:**

| Topic | Events |
|-------|--------|
| `nodes` | `node_added`, `node_removed`, `node_status_changed` |
| `workloads` | `workload_created`, `workload_scaled`, `workload_deleted`, `instance_started`, `instance_failed` |
| `cluster` | `cluster_topology_changed`, `leader_elected` |

**Example event:**

```json
{
  "type": "node_added",
  "timestamp": "2025-12-17T20:15:03Z",
  "data": {
    "id": "00000000-0000-0000-0000-000000000002",
    "address": "172.28.0.3:7280",
    "status": "Ready"
  }
}
```

### MCP Protocol (AI Agent Integration)

The Model Context Protocol (MCP) server enables AI agents like Claude Code to interact directly with the orchestrator through a structured JSON-RPC 2.0 interface.

#### Starting the MCP Server

The MCP server can run over stdio (for Claude Code integration) or as a standalone service:

```bash
# Via stdio (for Claude Code)
MCP_STDIO=true orchestrator_node

# Or run the standalone MCP server
cargo run -p mcp_server
```

#### Claude Code Configuration

Add to your Claude Code MCP settings (`~/.config/claude-code/mcp.json` or project `.claude/mcp.json`):

```json
{
  "mcpServers": {
    "orchestrator": {
      "command": "orchestrator_node",
      "args": [],
      "env": {
        "MCP_STDIO": "true",
        "RUST_LOG": "info"
      }
    }
  }
}
```

#### Available Tools

| Tool | Description | Parameters |
|------|-------------|------------|
| `list_workloads` | List all workload definitions | `namespace` (optional) |
| `create_workload` | Create a new workload | `name`, `image`, `replicas`, `labels` |
| `scale_workload` | Scale workload replicas | `workload_id`, `replicas` |
| `delete_workload` | Delete a workload | `workload_id` |
| `list_nodes` | List all cluster nodes | `status_filter` (optional) |
| `get_cluster_status` | Get cluster overview | `include_nodes`, `include_workloads` |

**Example tool call:**

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "create_workload",
    "arguments": {
      "name": "nginx",
      "image": "nginx:latest",
      "replicas": 3
    }
  }
}
```

**Response:**

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "content": [
      {
        "type": "text",
        "text": "Created workload nginx with ID: a5228e27-aff6-4a26-a61a-eecd16a91b04"
      }
    ]
  }
}
```

#### Available Resources

| Resource URI | Description |
|--------------|-------------|
| `orchestrator://nodes` | List of all cluster nodes |
| `orchestrator://nodes/{id}` | Specific node details |
| `orchestrator://workloads` | List of all workloads |
| `orchestrator://workloads/{id}` | Specific workload details |
| `orchestrator://cluster/status` | Cluster status summary |
| `orchestrator://cluster/metrics` | Cluster resource metrics |

**Example resource read:**

```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "resources/read",
  "params": {
    "uri": "orchestrator://cluster/status"
  }
}
```

**Response:**

```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "result": {
    "contents": [
      {
        "uri": "orchestrator://cluster/status",
        "mimeType": "application/json",
        "text": "{\"total_nodes\":2,\"ready_nodes\":2,\"total_workloads\":3,\"total_instances\":8}"
      }
    ]
  }
}
```

#### Protocol Handshake

The MCP protocol requires an initialization handshake:

```json
// 1. Client sends initialize
{
  "jsonrpc": "2.0",
  "id": 0,
  "method": "initialize",
  "params": {
    "protocolVersion": "2024-11-05",
    "capabilities": {},
    "clientInfo": {
      "name": "claude-code",
      "version": "1.0.0"
    }
  }
}

// 2. Server responds with capabilities
{
  "jsonrpc": "2.0",
  "id": 0,
  "result": {
    "protocolVersion": "2024-11-05",
    "capabilities": {
      "tools": { "listChanged": false },
      "resources": { "subscribe": false, "listChanged": false }
    },
    "serverInfo": {
      "name": "orchestrator-mcp",
      "version": "0.2.0"
    }
  }
}

// 3. Client sends initialized notification
{
  "jsonrpc": "2.0",
  "method": "initialized"
}

// 4. Now ready for tool calls and resource reads
```

#### Example AI Interaction Session

```
Human: Deploy a web server with 3 replicas

Claude: I'll deploy an nginx web server with 3 replicas using the orchestrator.

[Uses tools/call with create_workload]

Created workload "nginx-web" with ID: 7f3b2a1c-...
- Image: nginx:latest
- Replicas: 3

Let me check the cluster status to confirm the deployment...

[Uses resources/read with orchestrator://cluster/status]

Cluster Status:
- Total Nodes: 2 (all ready)
- Total Workloads: 4
- Running Instances: 11

The workload has been deployed successfully.
```

### Authentication

Univrs uses Ed25519 cryptographic signatures for authentication. No tokens, no sessions, no central auth server.

**Request signing:**

```
Authorization: Univrs <public_key>:<signature>:<timestamp>
```

Where:
- `public_key`: Base64-encoded Ed25519 public key
- `signature`: Base64-encoded signature of `METHOD\nPATH\nTIMESTAMP\nBODY_SHA256`
- `timestamp`: ISO8601 timestamp (must be within ±5 minutes)

**Note:** For local development, authentication can be disabled with `AUTH_DISABLED=true`.

---

## Part II: SDK Libraries (Layer 1)

### Design Principles

All Univrs SDKs follow these principles:

| Principle | Description |
|-----------|-------------|
| **Identity-First** | SDK manages keypairs and signing automatically |
| **Policy-Aware** | Local policy enforcement before API calls |
| **Offline-Capable** | Cache state, queue writes for sync |
| **Type-Safe** | Strong typing, generated from shared schema |
| **Async-Native** | All network operations are async |

### Rust SDK

```rust
use univrs_sdk::{Client, Identity, WorkloadSpec};

#[tokio::main]
async fn main() -> Result<()> {
    // Load identity from ~/.config/univrs/
    let identity = Identity::load_default()?;
    
    // Create client with auto-discovery
    let client = Client::builder()
        .identity(identity)
        .discover()
        .await?
        .build()?;
    
    // Deploy a workload
    let workload = client.workloads().create(WorkloadSpec {
        name: "my-app".into(),
        image: "myregistry/myapp:v1".into(),
        replicas: 3,
        ..Default::default()
    }).await?;
    
    println!("Created: {}", workload.id);
    
    // Stream events
    let mut events = client.events().subscribe(["workloads"]).await?;
    while let Some(event) = events.next().await {
        println!("Event: {:?}", event);
    }
    
    Ok(())
}
```

### TypeScript SDK

```typescript
import { Client, Identity } from '@univrs/sdk';

async function main() {
  // Load identity
  const identity = await Identity.loadDefault();
  
  // Create client
  const client = await Client.create({
    identity,
    discover: true,
  });
  
  // Deploy workload
  const workload = await client.workloads.create({
    name: 'my-app',
    image: 'myregistry/myapp:v1',
    replicas: 3,
  });
  
  console.log(`Created: ${workload.id}`);
  
  // Subscribe to events
  for await (const event of client.events.subscribe(['workloads'])) {
    console.log('Event:', event);
  }
}
```

### Python SDK

```python
from univrs import Client, Identity
import asyncio

async def main():
    # Load identity
    identity = Identity.load_default()
    
    # Create client
    client = await Client.create(identity=identity, discover=True)
    
    # Deploy workload
    workload = await client.workloads.create(
        name="my-app",
        image="myregistry/myapp:v1",
        replicas=3
    )
    
    print(f"Created: {workload.id}")
    
    # Subscribe to events
    async for event in client.events.subscribe(["workloads"]):
        print(f"Event: {event}")

asyncio.run(main())
```

---

## Part III: CLI Tools (Layer 2)

Univrs provides two CLI tools with distinct purposes:

| Tool | Audience | Purpose |
|------|----------|---------|
| `orch` | Developers, end-users | Deploy and manage workloads |
| `uictl` | Operators, SREs | Node and cluster management |

### orch - User CLI

The primary CLI for deploying and managing workloads.

#### Installation

```bash
# From source
cargo install --path ui_cli

# Binary is named 'orch'
orch --version
```

#### Quick Start

```bash
# Initialize identity (first-time setup)
orch init

# Deploy a workload
orch deploy --name nginx --image nginx:latest --replicas 2

# Check status
orch status

# Scale up
orch scale nginx 5

# View in JSON format
orch status --format json
```

#### Command Reference

```
orch - Univrs orchestration CLI

USAGE:
    orch <COMMAND> [OPTIONS]

COMMANDS:
    Identity
    ─────────────────────────────────────────────────────────
    init              Create identity keypair (~/.config/univrs/)
    identity show     Display your public identity
    identity export   Export identity for backup (encrypted)
    identity import   Import identity from backup
    
    Workloads
    ─────────────────────────────────────────────────────────
    deploy            Deploy a workload
    status            Show workload status
    scale             Scale workload replicas
    logs              Stream workload logs
    delete            Delete a workload
    
    Credits
    ─────────────────────────────────────────────────────────
    credits balance   Show credit balance
    credits history   Transaction history
    credits send      Transfer credits

GLOBAL OPTIONS:
    --api-url <URL>      API endpoint (default: http://localhost:9090)
    --format <FORMAT>    Output: table, json, yaml (default: table)
    -v, --verbose        Increase verbosity
    -h, --help           Show help

EXAMPLES:
    orch init
    orch deploy --name myapp --image myapp:v1 --replicas 3
    orch status --format json
    orch scale myapp 5
```

### uictl - Operator CLI

For node operators and cluster administrators.

#### Command Reference

```
uictl - Univrs operator control

USAGE:
    uictl <COMMAND> [OPTIONS]

COMMANDS:
    Node Operations
    ─────────────────────────────────────────────────────────
    node init         Initialize this machine as a node
    node register     Register node with the network
    node status       Show this node's status
    node drain        Evacuate all workloads
    node cordon       Mark node as unschedulable
    node uncordon     Mark node as schedulable
    
    Cluster Operations
    ─────────────────────────────────────────────────────────
    cluster status    Show cluster overview
    cluster nodes     List all nodes
    cluster events    Stream cluster events
    
    Debug
    ─────────────────────────────────────────────────────────
    debug gossip      Show gossip protocol state
    debug state       Inspect state store
    debug reconcile   Trigger manual reconciliation

EXAMPLES:
    uictl node init --resources "cpu=4,memory=8Gi"
    uictl node register --bootstrap 192.168.1.10:7280
    uictl cluster status
    uictl debug gossip --verbose
```

---

## Part IV: Dashboard (Layer 4)

The Univrs Dashboard provides visual management and monitoring through a React web interface.

### Features

| View | Description |
|------|-------------|
| **Network Graph** | P2P peer visualization |
| **Workloads** | Deploy, scale, and monitor workloads |
| **Nodes** | Node health, resources, status |
| **Cluster** | Overall cluster metrics and health |
| **Credits** | Credit balance and transactions |
| **Governance** | Proposals and voting |

### Running the Dashboard

```bash
cd mycelial-dashboard/dashboard
pnpm install
pnpm dev

# Opens at http://localhost:3000
```

### Configuration

Create `.env` in the dashboard directory:

```bash
# API endpoint
VITE_API_URL=http://localhost:9090/api

# WebSocket endpoint
VITE_WS_URL=ws://localhost:9090/api/v1/events

# Use mock data (for development without backend)
VITE_USE_MOCK_DATA=false
```

### Data Flow

```
┌─────────────────────────────────────────────────────────────┐
│                    DASHBOARD                                 │
│                       │                                      │
│         ┌─────────────┼─────────────┐                       │
│         │             │             │                       │
│         ▼             ▼             ▼                       │
│    ┌─────────┐   ┌─────────┐   ┌─────────┐                 │
│    │  REST   │   │WebSocket│   │   MCP   │                 │
│    │(queries)│   │(events) │   │(AI chat)│                 │
│    └────┬────┘   └────┬────┘   └────┬────┘                 │
│         │             │             │                       │
│         └─────────────┴─────────────┘                       │
│                       │                                      │
│                       ▼                                      │
│              ┌────────────────┐                             │
│              │  ORCHESTRATOR  │                             │
│              │  (Rust backend)│                             │
│              └────────────────┘                             │
└─────────────────────────────────────────────────────────────┘
```

---

## Part V: Templates & Generators (Layer 3)

### Workload Templates

Create workloads from templates:

```bash
# Create from template
orch create web --name my-app --port 8080

# Interactive mode
orch create
# ? Select template: web
# ? Application name: my-app
# ? Port: 8080
# Created ./my-app/univrs.yaml
```

### univrs.yaml Specification

```yaml
name: my-app
image: myregistry/myapp:v1
replicas: 3

resources:
  cpu: 100m
  memory: 128Mi

env:
  - name: NODE_ENV
    value: production

health:
  http:
    path: /health
    port: 8080

routing:
  - match:
      prefix: /
    port: 8080
```

---

## Part VI: Configuration

### Directory Structure

```
~/.config/univrs/
├── identity/
│   ├── keypair.age          # Encrypted Ed25519 keypair
│   ├── public.key           # Public key (shareable)
│   └── recovery.txt.age     # Encrypted recovery phrase
├── policy/
│   ├── trust.toml           # Trusted operators
│   └── limits.toml          # Resource limits
├── state/
│   ├── workloads.db         # Local workload cache
│   └── credits.db           # Credit history
└── config.toml              # Main configuration
```

### config.toml

```toml
[api]
url = "http://localhost:9090"
timeout_seconds = 30

[identity]
path = "~/.config/univrs/identity/keypair.age"

[output]
format = "table"  # table, json, yaml
color = true
```

---

## Quick Reference

### Common Operations

```bash
# Setup
orch init                              # Create identity

# Deploy
orch deploy --name app --image img:v1 --replicas 3
orch status                            # View all workloads
orch scale app 5                       # Scale to 5 replicas
orch delete app                        # Remove workload

# Monitor
orch logs app                          # Stream logs
orch status --watch                    # Watch status changes

# Credits
orch credits balance                   # Check balance
```

### API Quick Reference

```bash
# Health check
curl http://localhost:9090/health

# List nodes
curl http://localhost:9090/api/v1/nodes

# List workloads
curl http://localhost:9090/api/v1/workloads

# Create workload
curl -X POST http://localhost:9090/api/v1/workloads \
  -H "Content-Type: application/json" \
  -d '{"name":"nginx","replicas":2,"containers":[{"name":"nginx","image":"nginx:latest"}]}'

# Cluster status
curl http://localhost:9090/api/v1/cluster/status
```

### WebSocket Quick Reference

```javascript
const ws = new WebSocket('ws://localhost:9090/api/v1/events');

ws.onopen = () => {
  ws.send(JSON.stringify({
    type: 'subscribe',
    topics: ['nodes', 'workloads', 'cluster']
  }));
};

ws.onmessage = (event) => {
  const data = JSON.parse(event.data);
  console.log('Event:', data.type, data.data);
};
```

---

## Status & Roadmap

### Current Status (v0.2.0)

| Layer | Component | Status |
|-------|-----------|--------|
| **Layer 0** | REST API | ✅ Available |
| | WebSocket Events | ✅ Available |
| | MCP Server | ✅ Available |
| | Health/Metrics | ✅ Available |
| **Layer 1** | Rust SDK | 📋 Planned |
| | TypeScript SDK | 📋 Planned |
| | Python SDK | 📋 Planned |
| **Layer 2** | orch CLI | ✅ Available |
| | uictl CLI | 📋 Planned |
| **Layer 3** | Templates | 📋 Planned |
| | IDE Extensions | 📋 Planned |
| **Layer 4** | Dashboard | ✅ Available |

### Upcoming Features

- Mycelial Credit System
- OpenRaft Consensus
- libp2p P2P Networking
- Real Container Runtime (Youki)
- IDE Extensions (VS Code, JetBrains)
- CI/CD Integrations

---

*Univrs Developer Tools v0.2.0*  
*Building the future of sovereign infrastructure*