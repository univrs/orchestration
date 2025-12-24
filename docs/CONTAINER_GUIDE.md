# Container Guide: Local Development & Testing

## Univrs.io RustOrchestration

**Version**: 0.1.0  
**Last Updated**: December 17, 2025  
**Audience**: Developers with basic container knowledge

---

## Table of Contents

1. [Prerequisites](#1-prerequisites)
2. [Quick Start (5 minutes)](#2-quick-start-5-minutes)
3. [Detailed Setup](#3-detailed-setup)
4. [Building from Source](#4-building-from-source)
5. [Running the Cluster](#5-running-the-cluster)
6. [Verification & Testing](#6-verification--testing)
7. [Development Workflow](#7-development-workflow)
8. [Troubleshooting](#8-troubleshooting)
9. [Helper Scripts Reference](#9-helper-scripts-reference)
10. [Architecture Overview](#10-architecture-overview)

---

## 1. Prerequisites

### Required Software

| Software | Minimum Version | Purpose | Install Check |
|----------|-----------------|---------|---------------|
| Docker | 24.0+ | Container runtime | `docker --version` |
| Docker Compose | 2.20+ | Multi-container orchestration | `docker compose version` |
| Git | 2.30+ | Source control | `git --version` |
| curl | Any | Health checks | `curl --version` |
| jq | 1.6+ | JSON parsing (optional) | `jq --version` |

### Optional (for building without Docker)

| Software | Version | Purpose |
|----------|---------|---------|
| Rust | 1.75+ | Compile from source |
| libseccomp-dev | 2.5+ | Youki container runtime |

### System Requirements

| Resource | Minimum | Recommended |
|----------|---------|-------------|
| RAM | 4 GB | 8 GB |
| CPU | 2 cores | 4 cores |
| Disk | 10 GB | 20 GB |
| OS | Linux, macOS, Windows (WSL2) | Linux (native) |

---

## 2. Quick Start (5 minutes)

If you just want to get a cluster running:

```bash
# 1. Clone the repository
git clone https://github.com/univrs/ai_native_orchestrator.git
cd ai_native_orchestrator

# 2. Start the cluster
docker compose up -d

# 3. Verify it's running
curl http://localhost:9090/health

# 4. View logs
docker compose logs -f

# 5. Stop when done
docker compose down
```

That's it! You now have a 3-node cluster running locally.

---

## 3. Detailed Setup

### 3.1 Install Docker

#### Linux (Ubuntu/Debian)

```bash
# Remove old versions
sudo apt-get remove docker docker-engine docker.io containerd runc

# Install prerequisites
sudo apt-get update
sudo apt-get install -y \
    ca-certificates \
    curl \
    gnupg \
    lsb-release

# Add Docker's official GPG key
sudo mkdir -m 0755 -p /etc/apt/keyrings
curl -fsSL https://download.docker.com/linux/ubuntu/gpg | \
    sudo gpg --dearmor -o /etc/apt/keyrings/docker.gpg

# Add the repository
echo \
  "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/docker.gpg] \
  https://download.docker.com/linux/ubuntu \
  $(lsb_release -cs) stable" | \
  sudo tee /etc/apt/sources.list.d/docker.list > /dev/null

# Install Docker
sudo apt-get update
sudo apt-get install -y docker-ce docker-ce-cli containerd.io docker-compose-plugin

# Add your user to docker group (logout/login required)
sudo usermod -aG docker $USER
newgrp docker

# Verify
docker run hello-world
```

#### macOS

```bash
# Using Homebrew
brew install --cask docker

# Or download Docker Desktop from:
# https://www.docker.com/products/docker-desktop

# After installation, start Docker Desktop
open -a Docker

# Verify
docker run hello-world
```

#### Windows (WSL2)

```powershell
# 1. Enable WSL2 (PowerShell as Admin)
wsl --install

# 2. Download and install Docker Desktop
# https://www.docker.com/products/docker-desktop

# 3. In Docker Desktop Settings:
#    - Enable "Use WSL 2 based engine"
#    - Enable integration with your WSL distro

# 4. In WSL terminal
docker run hello-world
```

### 3.2 Install Helper Tools

```bash
# Install jq for JSON parsing
# Ubuntu/Debian
sudo apt-get install -y jq

# macOS
brew install jq

# Verify all prerequisites
./scripts/check-prerequisites.sh
```

### 3.3 Clone the Repository

```bash
# HTTPS
git clone https://github.com/univrs/ai_native_orchestrator.git

# Or SSH (if you have keys configured)
git clone git@github.com:univrs/ai_native_orchestrator.git

cd ai_native_orchestrator
```

---

## 4. Building from Source

### 4.1 Using Docker (Recommended)

The Dockerfile uses multi-stage builds for optimal image size:

```bash
# Build the image
docker build -t univrs/orchestrator:dev .

# Build with no cache (if you suspect caching issues)
docker build --no-cache -t univrs/orchestrator:dev .

# Build for a specific platform
docker build --platform linux/amd64 -t univrs/orchestrator:dev .
```

**Build stages:**
1. **Builder**: Rust 1.83 compiles the binary (~5-10 minutes first time)
2. **Runtime**: Minimal Debian image with just the binary (~100MB)

### 4.2 Native Build (Without Docker)

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env

# Install system dependencies (for Youki runtime)
sudo apt-get install -y libseccomp-dev pkg-config

# Build workspace
cargo build --release

# Run tests
cargo test --workspace

# The binary will be at:
# target/release/node
```

### 4.3 Build Features

The orchestrator supports feature flags for different backends:

```bash
# Default build (mock runtimes)
cargo build --release

# With Youki container runtime
cargo build --release --features youki-runtime

# With Chitchat cluster manager
cargo build --release --features chitchat-cluster

# All features
cargo build --release --all-features
```

---

## 5. Running the Cluster

### 5.1 Docker Compose (Standard)

```bash
# Start in background
docker compose up -d

# Start with logs visible
docker compose up

# Start specific services
docker compose up bootstrap -d
docker compose up worker1 worker2 -d
```

### 5.2 Cluster Configuration

The `docker-compose.yml` defines a 3-node cluster:

| Node | Role | Ports | Description |
|------|------|-------|-------------|
| bootstrap | Bootstrap | 9090 (HTTP), 7280 (gossip) | Initial seed node |
| worker1 | Worker | 9091 (HTTP) | Workload runner |
| worker2 | Worker | 9092 (HTTP) | Workload runner |

### 5.3 Environment Variables

You can customize nodes via environment:

```bash
# docker-compose.override.yml (create this file)
services:
  bootstrap:
    environment:
      - RUST_LOG=debug
      - NODE_ID=custom-bootstrap
      - OBSERVABILITY_PORT=9090
```

| Variable | Default | Description |
|----------|---------|-------------|
| `NODE_ID` | hostname | Unique node identifier |
| `NODE_ROLE` | worker | `bootstrap` or `worker` |
| `SEED_NODES` | (none) | Comma-separated seed node addresses |
| `GOSSIP_PORT` | 7280 | Chitchat gossip port |
| `OBSERVABILITY_PORT` | 9090 | Health/metrics HTTP port |
| `RUST_LOG` | info | Log level (trace, debug, info, warn, error) |

### 5.4 Scaling Workers

```bash
# Add more workers
docker compose up -d --scale worker1=3

# Note: You'll need to modify docker-compose.yml for 
# additional named workers with unique ports
```

---

## 6. Verification & Testing

### 6.1 Health Checks

```bash
# Check all nodes
./scripts/cluster-health.sh

# Or manually:
curl -s http://localhost:9090/health | jq
curl -s http://localhost:9091/health | jq
curl -s http://localhost:9092/health | jq
```

**Expected response:**
```json
{
  "status": "healthy",
  "components": {
    "state_store": "healthy",
    "cluster_manager": "healthy",
    "scheduler": "healthy"
  }
}
```

### 6.2 Readiness & Liveness Probes

```bash
# Kubernetes-style probes
curl -s http://localhost:9090/ready   # Readiness
curl -s http://localhost:9090/live    # Liveness

# Shorter aliases also work
curl -s http://localhost:9090/readyz
curl -s http://localhost:9090/livez
```

### 6.3 Metrics

```bash
# Prometheus metrics
curl -s http://localhost:9090/metrics

# Specific metrics of interest:
curl -s http://localhost:9090/metrics | grep orchestrator_nodes
curl -s http://localhost:9090/metrics | grep orchestrator_workloads
```

### 6.4 Gossip Verification

```bash
# Check if nodes discovered each other
docker compose logs | grep -i "member\|discovered\|joined"

# Expected output:
# bootstrap  | New member joined: worker1
# bootstrap  | New member joined: worker2
# worker1    | Connected to bootstrap
```

### 6.5 Run Integration Tests

```bash
# Full test suite
cargo test --workspace

# Specific crate tests
cargo test -p orchestrator_core
cargo test -p observability

# With output
cargo test --workspace -- --nocapture
```

---

## 7. Development Workflow

### 7.1 Live Reload Development

For active development, use bind mounts:

```yaml
# docker-compose.dev.yml
services:
  bootstrap:
    build:
      context: .
      dockerfile: Dockerfile.dev
    volumes:
      - ./target/release/node:/app/node:ro
    # ... rest of config
```

```bash
# Terminal 1: Watch for changes and rebuild
cargo watch -x 'build --release'

# Terminal 2: Restart containers when binary changes
docker compose -f docker-compose.yml -f docker-compose.dev.yml up
```

### 7.2 Debugging

```bash
# Increase log verbosity
RUST_LOG=debug docker compose up

# Trace level for maximum detail
RUST_LOG=trace docker compose up

# Specific crate debugging
RUST_LOG=orchestrator_core=debug,chitchat=trace docker compose up
```

### 7.3 Shell Access

```bash
# Access running container
docker compose exec bootstrap /bin/bash

# Run one-off command
docker compose exec bootstrap cat /etc/os-release
```

### 7.4 Clean Rebuild

```bash
# Remove all containers and volumes
docker compose down -v

# Remove images
docker compose down --rmi all

# Full clean
docker system prune -af
```

---

## 8. Troubleshooting

### 8.1 Common Issues

#### Container won't start

```bash
# Check logs
docker compose logs bootstrap

# Common causes:
# - Port already in use → change ports in docker-compose.yml
# - Build failed → docker compose build --no-cache
```

#### Nodes not discovering each other

```bash
# Check network
docker network ls
docker network inspect ai_native_orchestrator_default

# Verify DNS resolution inside container
docker compose exec bootstrap getent hosts worker1
```

#### Health check failing

```bash
# Check if process is running
docker compose exec bootstrap ps aux

# Check if port is listening
docker compose exec bootstrap netstat -tlnp

# Manual health check from inside
docker compose exec bootstrap curl -v http://localhost:9090/health
```

#### Out of memory

```bash
# Check container resource usage
docker stats

# Increase Docker memory (Docker Desktop → Settings → Resources)
```

### 8.2 Reset Everything

```bash
# Nuclear option: remove everything
./scripts/cluster-reset.sh

# Or manually:
docker compose down -v --rmi all
docker system prune -af
rm -rf target/  # If you want to rebuild Rust too
```

---

## 9. Helper Scripts Reference

Create these scripts in `scripts/` directory:

### scripts/check-prerequisites.sh

```bash
#!/bin/bash
set -e

echo "Checking prerequisites..."

check_command() {
    if command -v $1 &> /dev/null; then
        echo "✅ $1: $(command -v $1)"
        return 0
    else
        echo "❌ $1: NOT FOUND"
        return 1
    fi
}

check_version() {
    local cmd=$1
    local min_version=$2
    local version=$($3)
    echo "   Version: $version (minimum: $min_version)"
}

echo ""
echo "Required:"
check_command docker
check_command "docker compose" || check_command docker-compose
check_command git
check_command curl

echo ""
echo "Optional:"
check_command jq || echo "   (install with: apt-get install jq)"
check_command rustc || echo "   (install with: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh)"

echo ""
echo "Docker daemon:"
if docker info &> /dev/null; then
    echo "✅ Docker daemon is running"
else
    echo "❌ Docker daemon is NOT running"
    echo "   Start with: sudo systemctl start docker"
fi

echo ""
echo "Prerequisites check complete!"
```

### scripts/cluster-start.sh

```bash
#!/bin/bash
set -e

echo "🚀 Starting Univrs Orchestrator Cluster..."

# Build if needed
if [[ "$1" == "--build" ]]; then
    echo "Building images..."
    docker compose build
fi

# Start cluster
docker compose up -d

# Wait for health
echo "Waiting for nodes to be healthy..."
sleep 3

# Check health
./scripts/cluster-health.sh

echo ""
echo "✅ Cluster is running!"
echo ""
echo "Access points:"
echo "  Bootstrap: http://localhost:9090"
echo "  Worker 1:  http://localhost:9091"
echo "  Worker 2:  http://localhost:9092"
echo ""
echo "Commands:"
echo "  Logs:    docker compose logs -f"
echo "  Stop:    docker compose down"
echo "  Reset:   ./scripts/cluster-reset.sh"
```

### scripts/cluster-health.sh

```bash
#!/bin/bash

echo "🏥 Checking cluster health..."
echo ""

check_node() {
    local name=$1
    local port=$2
    
    response=$(curl -s -w "\n%{http_code}" http://localhost:$port/health 2>/dev/null)
    http_code=$(echo "$response" | tail -n1)
    body=$(echo "$response" | sed '$d')
    
    if [[ "$http_code" == "200" ]]; then
        status=$(echo "$body" | jq -r '.status' 2>/dev/null || echo "unknown")
        echo "✅ $name (port $port): $status"
        if command -v jq &> /dev/null; then
            echo "$body" | jq -r '   Components: \(.components | keys | join(", "))' 2>/dev/null || true
        fi
    else
        echo "❌ $name (port $port): unreachable (HTTP $http_code)"
    fi
}

check_node "Bootstrap" 9090
check_node "Worker 1 " 9091
check_node "Worker 2 " 9092

echo ""

# Check gossip membership
echo "📡 Gossip membership:"
docker compose logs --tail=50 2>/dev/null | grep -i "member\|joined\|discovered" | tail -5 || echo "   (no recent membership events)"
```

### scripts/cluster-stop.sh

```bash
#!/bin/bash

echo "🛑 Stopping cluster..."
docker compose down

echo "✅ Cluster stopped"
```

### scripts/cluster-reset.sh

```bash
#!/bin/bash

echo "⚠️  This will remove all containers, volumes, and images."
read -p "Are you sure? (y/N) " -n 1 -r
echo

if [[ $REPLY =~ ^[Yy]$ ]]; then
    echo "Stopping and removing containers..."
    docker compose down -v --rmi all
    
    echo "Pruning Docker system..."
    docker system prune -f
    
    echo "✅ Reset complete"
else
    echo "Cancelled"
fi
```

### scripts/cluster-logs.sh

```bash
#!/bin/bash

# Default: follow all logs
# Usage: ./cluster-logs.sh [service] [--no-follow]

SERVICE=${1:-}
FOLLOW=${2:--f}

if [[ "$1" == "--no-follow" ]]; then
    SERVICE=""
    FOLLOW=""
fi

if [[ -n "$SERVICE" && "$SERVICE" != "--no-follow" ]]; then
    docker compose logs $FOLLOW $SERVICE
else
    docker compose logs $FOLLOW
fi
```

### Make scripts executable

```bash
chmod +x scripts/*.sh
```

---

## 10. Architecture Overview

### 10.1 Container Layout

```
┌─────────────────────────────────────────────────────────────────────────┐
│                         LOCAL DOCKER NETWORK                             │
│                      (ai_native_orchestrator_default)                    │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│  ┌─────────────────────┐                                                │
│  │     BOOTSTRAP       │                                                │
│  │  ┌───────────────┐  │                                                │
│  │  │ orchestrator  │  │  Ports:                                        │
│  │  │    node       │  │  - 9090 → HTTP (health, metrics)              │
│  │  │               │  │  - 7280 → Gossip (chitchat)                   │
│  │  └───────────────┘  │                                                │
│  │  Role: bootstrap    │                                                │
│  └──────────┬──────────┘                                                │
│             │                                                            │
│             │ Gossip Protocol                                           │
│             │                                                            │
│  ┌──────────┴──────────┐  ┌─────────────────────┐                       │
│  │      WORKER 1       │  │      WORKER 2       │                       │
│  │  ┌───────────────┐  │  │  ┌───────────────┐  │                       │
│  │  │ orchestrator  │  │  │  │ orchestrator  │  │                       │
│  │  │    node       │  │  │  │    node       │  │                       │
│  │  └───────────────┘  │  │  └───────────────┘  │                       │
│  │  Port: 9091         │  │  Port: 9092         │                       │
│  │  Role: worker       │  │  Role: worker       │                       │
│  └─────────────────────┘  └─────────────────────┘                       │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
                              │
                              │ Host Network
                              ▼
                    ┌─────────────────────┐
                    │    YOUR MACHINE     │
                    │                     │
                    │  localhost:9090     │ → Bootstrap health/metrics
                    │  localhost:9091     │ → Worker 1 health/metrics
                    │  localhost:9092     │ → Worker 2 health/metrics
                    │                     │
                    └─────────────────────┘
```

### 10.2 Process Flow

```
1. docker compose up -d
   │
   ├─→ Bootstrap starts
   │   ├─→ Initializes state store (in-memory)
   │   ├─→ Starts chitchat gossip listener on 7280
   │   ├─→ Starts HTTP server on 9090
   │   └─→ Waits for workers
   │
   ├─→ Worker 1 starts
   │   ├─→ Resolves "bootstrap" DNS name
   │   ├─→ Connects to bootstrap:7280 (gossip)
   │   ├─→ Joins cluster membership
   │   └─→ Starts HTTP server on 9091
   │
   └─→ Worker 2 starts
       ├─→ Same as Worker 1
       └─→ Starts HTTP server on 9092

2. Gossip protocol maintains membership
   │
   ├─→ Bootstrap knows about Worker 1, Worker 2
   ├─→ Worker 1 knows about Bootstrap, Worker 2
   └─→ Worker 2 knows about Bootstrap, Worker 1

3. Health checks verify cluster state
   │
   └─→ /health returns component status
```

### 10.3 File Structure

```
ai_native_orchestrator/
├── Dockerfile                    # Multi-stage build
├── docker-compose.yml            # 3-node cluster definition
├── docker-compose.dev.yml        # Development overrides (optional)
├── scripts/
│   ├── check-prerequisites.sh    # Verify system requirements
│   ├── cluster-start.sh          # Start with health check
│   ├── cluster-stop.sh           # Stop cluster
│   ├── cluster-reset.sh          # Full cleanup
│   ├── cluster-health.sh         # Health check all nodes
│   └── cluster-logs.sh           # View logs
├── orchestrator_core/
│   └── src/
│       └── bin/
│           └── node.rs           # Production node binary
├── observability/                # Health, metrics, tracing
├── state_store_interface/        # State management
├── cluster_manager/              # Chitchat gossip
├── container_runtime/            # Mock + Youki
├── mcp_server/                   # AI control plane
└── CONTAINER_GUIDE.md            # This file
```

---

## Quick Reference Card

```bash
# ─────────────────────────────────────────────────────────
# UNIVRS ORCHESTRATOR - QUICK REFERENCE
# ─────────────────────────────────────────────────────────

# SETUP
git clone https://github.com/univrs/ai_native_orchestrator.git
cd ai_native_orchestrator
./scripts/check-prerequisites.sh

# START
docker compose up -d              # Start cluster
./scripts/cluster-health.sh       # Verify health

# MONITOR
docker compose logs -f            # All logs
docker compose logs -f bootstrap  # Single node
curl localhost:9090/metrics       # Prometheus metrics

# DEVELOP
cargo test --workspace            # Run tests
cargo build --release             # Build binary
docker compose build              # Rebuild images

# STOP
docker compose down               # Stop cluster
./scripts/cluster-reset.sh        # Full cleanup

# TROUBLESHOOT
docker compose ps                 # Container status
docker stats                      # Resource usage
docker compose exec bootstrap sh  # Shell access
```

---

## Next Steps

After verifying your local cluster:

1. **Explore the MCP Server** - AI-native control plane
2. **Run the Dashboard** - Visual cluster management
3. **Deploy a Workload** - Test the reconciliation loop
4. **Join the Network** - Connect to community nodes

See `VISION_SESSION_2025-12-17.md` for the project roadmap.

---

*Container Guide v1.0 - Univrs.io RustOrchestration*
