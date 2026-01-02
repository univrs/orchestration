#!/bin/bash
# Test script: Start 4-node cluster + dashboard and verify connections
# Usage: ./scripts/test-dashboard-cluster.sh

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ORCHESTRATOR_DIR="$(dirname "$SCRIPT_DIR")"
DASHBOARD_DIR="/home/ardeshir/repos/univrs-network/dashboard"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

log() { echo -e "${BLUE}[$(date +%H:%M:%S)]${NC} $1"; }
success() { echo -e "${GREEN}[✓]${NC} $1"; }
warn() { echo -e "${YELLOW}[!]${NC} $1"; }
error() { echo -e "${RED}[✗]${NC} $1"; }

# Cleanup function
cleanup() {
    log "Cleaning up..."
    cd "$ORCHESTRATOR_DIR"
    docker-compose down 2>/dev/null || true
    if [ -n "$DASHBOARD_PID" ]; then
        kill $DASHBOARD_PID 2>/dev/null || true
    fi
}
trap cleanup EXIT

log "=== Starting 4-Node Cluster + Dashboard Test ==="

# Step 1: Clean up old containers
log "Step 1: Cleaning up old containers..."
cd "$ORCHESTRATOR_DIR"
docker rm -f orchestrator-bootstrap orchestrator-worker1 orchestrator-worker2 orchestrator-worker3 2>/dev/null || true
docker network rm orchestrator_orchestrator-net 2>/dev/null || true
success "Cleanup complete"

# Step 2: Start the cluster
log "Step 2: Starting 4-node cluster..."
docker-compose up -d
success "Docker containers started"

# Step 3: Wait for nodes to be healthy
log "Step 3: Waiting for nodes to become healthy..."
MAX_WAIT=60
WAIT_COUNT=0
while [ $WAIT_COUNT -lt $MAX_WAIT ]; do
    HEALTHY=$(docker-compose ps | grep -c "(healthy)" || true)
    if [ "$HEALTHY" -ge 3 ]; then
        break
    fi
    echo -n "."
    sleep 2
    WAIT_COUNT=$((WAIT_COUNT + 2))
done
echo ""

if [ "$HEALTHY" -lt 3 ]; then
    error "Nodes did not become healthy in time"
    docker-compose ps
    exit 1
fi
success "All nodes are healthy"

# Step 4: Verify API endpoints
log "Step 4: Testing API endpoints..."
echo ""

# Test nodes endpoint
NODES_RESPONSE=$(curl -s http://localhost:9090/api/v1/nodes)
NODE_COUNT=$(echo "$NODES_RESPONSE" | grep -o '"id"' | wc -l)
if [ "$NODE_COUNT" -gt 0 ]; then
    success "Nodes API: Found $NODE_COUNT nodes"
    echo "$NODES_RESPONSE" | python3 -m json.tool 2>/dev/null || echo "$NODES_RESPONSE"
else
    warn "Nodes API returned no nodes"
fi
echo ""

# Test cluster status
CLUSTER_RESPONSE=$(curl -s http://localhost:9090/api/v1/cluster/status)
if echo "$CLUSTER_RESPONSE" | grep -q "total_nodes"; then
    success "Cluster Status API working"
    echo "$CLUSTER_RESPONSE" | python3 -m json.tool 2>/dev/null || echo "$CLUSTER_RESPONSE"
else
    warn "Cluster Status API issue"
fi
echo ""

# Test health endpoint
HEALTH_RESPONSE=$(curl -s http://localhost:9090/health)
if echo "$HEALTH_RESPONSE" | grep -q "healthy"; then
    success "Health endpoint working"
else
    warn "Health endpoint issue"
fi
echo ""

# Step 5: Start dashboard (if node_modules exist)
log "Step 5: Checking dashboard..."
if [ -d "$DASHBOARD_DIR/node_modules" ]; then
    log "Starting dashboard on http://localhost:3000 ..."
    cd "$DASHBOARD_DIR"
    # Start dashboard in background with environment variable pointing to orchestrator
    VITE_ORCHESTRATOR_API_URL=http://localhost:9090 \
    VITE_ORCHESTRATOR_WS_URL=ws://localhost:9090/api/v1/events \
    npm run dev &
    DASHBOARD_PID=$!
    success "Dashboard started (PID: $DASHBOARD_PID)"

    # Wait for dashboard to start
    sleep 5

    if curl -s http://localhost:3000 >/dev/null 2>&1; then
        success "Dashboard is running at http://localhost:3000"
    else
        warn "Dashboard may not be fully started yet, check manually"
    fi
else
    warn "Dashboard node_modules not found. Run: cd $DASHBOARD_DIR && npm install"
fi

# Step 6: Test WebSocket connection
log "Step 6: Testing WebSocket..."
# Try to connect and subscribe
WS_TEST=$(curl -s -N \
    -H "Connection: Upgrade" \
    -H "Upgrade: websocket" \
    -H "Sec-WebSocket-Version: 13" \
    -H "Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==" \
    --max-time 2 \
    http://localhost:9090/api/v1/events 2>&1 || true)

if echo "$WS_TEST" | grep -q "101\|Switching"; then
    success "WebSocket upgrade working"
else
    log "WebSocket test (raw response limited to 2s)"
fi

# Summary
echo ""
log "=== Summary ==="
echo ""
success "Cluster Status:"
docker-compose ps
echo ""

echo -e "${BLUE}Endpoints:${NC}"
echo "  - Bootstrap:  http://localhost:9090 (API + WebSocket)"
echo "  - Worker 1:   http://localhost:9091"
echo "  - Worker 2:   http://localhost:9092"
echo "  - Dashboard:  http://localhost:3000"
echo ""
echo -e "${BLUE}API Endpoints:${NC}"
echo "  - Nodes:      http://localhost:9090/api/v1/nodes"
echo "  - Status:     http://localhost:9090/api/v1/cluster/status"
echo "  - Workloads:  http://localhost:9090/api/v1/workloads"
echo "  - Health:     http://localhost:9090/health"
echo "  - WebSocket:  ws://localhost:9090/api/v1/events"
echo ""

if [ -n "$DASHBOARD_PID" ]; then
    echo -e "${GREEN}Dashboard is running. Press Ctrl+C to stop everything.${NC}"
    wait $DASHBOARD_PID
else
    echo -e "${YELLOW}Start dashboard manually:${NC}"
    echo "  cd $DASHBOARD_DIR"
    echo "  VITE_ORCHESTRATOR_API_URL=http://localhost:9090 npm run dev"
    echo ""
    echo "Press Ctrl+C to stop the cluster."
    # Keep script running to maintain cluster
    while true; do sleep 10; done
fi
