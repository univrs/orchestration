# Multi-stage Dockerfile for AI-Native Orchestrator
# Builds a minimal container with the orchestrator node binary

# Stage 1: Build environment
FROM rust:1.83-slim-bookworm AS builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy workspace files
COPY Cargo.toml Cargo.lock ./

# Copy all crates
COPY orchestrator_shared_types ./orchestrator_shared_types
COPY container_runtime_interface ./container_runtime_interface
COPY cluster_manager_interface ./cluster_manager_interface
COPY scheduler_interface ./scheduler_interface
COPY state_store_interface ./state_store_interface
COPY container_runtime ./container_runtime
COPY cluster_manager ./cluster_manager
COPY mcp_server ./mcp_server
COPY observability ./observability
COPY orchestrator_core ./orchestrator_core

# Build the orchestrator node with cluster and observability support
RUN cargo build --release -p orchestrator_core --features "cluster,observability" --bin orchestrator_node

# Stage 2: Runtime environment
FROM debian:bookworm-slim AS runtime

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd -r -s /bin/false orchestrator

WORKDIR /app

# Copy the binary from builder
COPY --from=builder /app/target/release/orchestrator_node /app/orchestrator_node

# Change ownership
RUN chown orchestrator:orchestrator /app/orchestrator_node

# Switch to non-root user
USER orchestrator

# Expose ports
# 7280: Gossip/cluster communication
# 9090: Health/metrics HTTP API
EXPOSE 7280/udp 9090/tcp

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=10s --retries=3 \
    CMD curl -f http://localhost:9090/health || exit 1

# Default environment variables
ENV NODE_ROLE=worker \
    LISTEN_ADDR=0.0.0.0:7280 \
    API_PORT=9090 \
    NODE_CPU=4.0 \
    NODE_MEMORY_MB=8192 \
    NODE_DISK_MB=102400 \
    LOG_LEVEL=info \
    LOG_JSON=true \
    CLUSTER_ID=orchestrator-cluster

# Run the orchestrator node
ENTRYPOINT ["/app/orchestrator_node"]
