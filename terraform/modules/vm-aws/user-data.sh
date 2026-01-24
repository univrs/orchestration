#!/bin/bash
set -e

# Univrs.io Node Bootstrap Script
# This script runs on first boot to configure the node

echo "Starting Univrs.io node bootstrap..."

# Update system and install Docker
if command -v yum &> /dev/null; then
    # Amazon Linux / RHEL
    yum update -y
    yum install -y docker jq curl
elif command -v apt-get &> /dev/null; then
    # Ubuntu / Debian
    apt-get update -y
    apt-get install -y docker.io jq curl
fi

# Enable and start Docker
systemctl enable docker
systemctl start docker

# Wait for Docker to be ready
sleep 5

# Get public IP for announcements
PUBLIC_IP=$(curl -sf http://169.254.169.254/latest/meta-data/public-ipv4 || echo "")
if [ -z "$PUBLIC_IP" ]; then
    PUBLIC_IP=$(curl -sf https://ifconfig.me || echo "127.0.0.1")
fi

echo "Public IP: $PUBLIC_IP"

# Configure based on node role
%{ if node_role == "bootstrap" }
echo "Configuring as BOOTSTRAP node..."
BOOTSTRAP_FLAG="--bootstrap"
CONNECT_FLAG=""
ORCHESTRATOR_ROLE="bootstrap"
SEED_NODES=""
%{ else }
echo "Configuring as WORKER node..."
BOOTSTRAP_FLAG=""
CONNECT_FLAG="--connect /ip4/${bootstrap_ip}/tcp/9000"
ORCHESTRATOR_ROLE="worker"
SEED_NODES="${bootstrap_ip}:7280"
%{ endif }

# Pull Docker images
echo "Pulling Docker images..."
docker pull ${docker_image_network}
docker pull ${docker_image_orchestrator}

# Stop and remove any existing containers
docker stop univrs-network univrs-orchestrator 2>/dev/null || true
docker rm univrs-network univrs-orchestrator 2>/dev/null || true

# Run univrs-network (P2P layer)
echo "Starting univrs-network..."
docker run -d \
    --name univrs-network \
    --restart unless-stopped \
    -p 8080:8080 \
    -p 9000:9000 \
    -p 9001:9001/udp \
    -v univrs-network-data:/app/data \
    -e RUST_LOG=info \
    ${docker_image_network} \
    $BOOTSTRAP_FLAG $CONNECT_FLAG \
    --name "${node_name}" \
    --port 9000 \
    --http-port 8080

# Wait for network to start
sleep 10

# Run univrs-orchestration (container orchestration)
echo "Starting univrs-orchestration..."
docker run -d \
    --name univrs-orchestrator \
    --restart unless-stopped \
    -p 7280:7280/udp \
    -p 9090:9090 \
    -v univrs-orchestrator-data:/var/lib/orchestrator \
    -e NODE_ROLE=$ORCHESTRATOR_ROLE \
    -e LISTEN_ADDR=0.0.0.0:7280 \
    -e PUBLIC_ADDR=$PUBLIC_IP:7280 \
    -e SEED_NODES="$SEED_NODES" \
    -e CLUSTER_ID="${cluster_id}" \
    -e API_PORT=9090 \
    -e LOG_LEVEL=info \
    -e LOG_JSON=true \
    ${docker_image_orchestrator}

# Wait for services to start
sleep 15

# Health check
echo "Running health checks..."

NETWORK_HEALTH=$(curl -sf http://localhost:8080/health || echo "FAILED")
ORCHESTRATOR_HEALTH=$(curl -sf http://localhost:9090/health || echo "FAILED")

echo "Network health: $NETWORK_HEALTH"
echo "Orchestrator health: $ORCHESTRATOR_HEALTH"

if [ "$NETWORK_HEALTH" != "FAILED" ] && [ "$ORCHESTRATOR_HEALTH" != "FAILED" ]; then
    echo "Univrs.io node bootstrap COMPLETE"
else
    echo "WARNING: Health checks did not pass. Check container logs."
    docker logs univrs-network 2>&1 | tail -50
    docker logs univrs-orchestrator 2>&1 | tail -50
fi

# Log completion
echo "Bootstrap completed at $(date)" >> /var/log/univrs-bootstrap.log
