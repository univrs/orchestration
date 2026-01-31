# Multi-stage Dockerfile for AI-Native Orchestrator
# Builds a minimal container with the orchestrator node binary
#
# Build options:
#   --build-arg RUNTIME=mock     - Use mock runtime (default, no youki)
#   --build-arg RUNTIME=youki    - Use youki runtime (includes youki binary)

ARG RUNTIME=mock

# Stage 1: Build environment
FROM rust:1.85-slim-bookworm AS builder

ARG RUNTIME

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Download youki binary if using youki runtime
RUN if [ "$RUNTIME" = "youki" ]; then \
        YOUKI_VERSION="0.4.1" && \
        curl -fsSL "https://github.com/containers/youki/releases/download/v${YOUKI_VERSION}/youki_${YOUKI_VERSION}_linux_amd64.tar.gz" \
            -o /tmp/youki.tar.gz && \
        tar -xzf /tmp/youki.tar.gz -C /usr/local/bin && \
        chmod +x /usr/local/bin/youki && \
        rm /tmp/youki.tar.gz; \
    fi

WORKDIR /app

# Copy external dependencies first (from repo root context or stubs)
COPY univrs-identity /univrs-identity
COPY univrs-state /univrs-state

# Copy workspace files and fix external dependency paths for Docker context
COPY orchestrator/Cargo.toml orchestrator/Cargo.lock ./
RUN sed -i 's|path = "../../univrs-identity"|path = "/univrs-identity"|g' Cargo.toml && \
    sed -i 's|path = "../../univrs-state"|path = "/univrs-state"|g' Cargo.toml

# Copy all crates
COPY orchestrator/orchestrator_shared_types ./orchestrator_shared_types
COPY orchestrator/container_runtime_interface ./container_runtime_interface
COPY orchestrator/cluster_manager_interface ./cluster_manager_interface
COPY orchestrator/scheduler_interface ./scheduler_interface
COPY orchestrator/state_store_interface ./state_store_interface
COPY orchestrator/container_runtime ./container_runtime
COPY orchestrator/cluster_manager ./cluster_manager
COPY orchestrator/mcp_server ./mcp_server
COPY orchestrator/observability ./observability
COPY orchestrator/orchestrator_core ./orchestrator_core
COPY orchestrator/user_config ./user_config
COPY orchestrator/ui_cli ./ui_cli

# Build the orchestrator node with appropriate features based on runtime
RUN if [ "$RUNTIME" = "youki" ]; then \
        cargo build --release -p orchestrator_core --features "cluster,youki-runtime,observability,rest-api" --bin orchestrator_node; \
    else \
        cargo build --release -p orchestrator_core --features "cluster,observability,rest-api" --bin orchestrator_node; \
    fi

# Stage 2: Runtime environment
FROM debian:bookworm-slim AS runtime

ARG RUNTIME

# Install runtime dependencies
# Note: For youki runtime, we need libseccomp for container security
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    curl \
    $(if [ "$RUNTIME" = "youki" ]; then echo "libseccomp2"; fi) \
    && rm -rf /var/lib/apt/lists/*

# Create orchestrator user and directories
# Note: For youki runtime, containers need special capabilities
RUN useradd -r -s /bin/false orchestrator && \
    mkdir -p /var/lib/orchestrator/bundles && \
    mkdir -p /var/lib/orchestrator/images && \
    mkdir -p /run/orchestrator && \
    chown -R orchestrator:orchestrator /var/lib/orchestrator /run/orchestrator

WORKDIR /app

# Copy the binary from builder
COPY --from=builder /app/target/release/orchestrator_node /app/orchestrator_node

# Copy youki binary if using youki runtime
# We use a wildcard pattern that matches youki if it exists
COPY --from=builder /usr/local/bin/youk* /usr/local/bin/

# Change ownership
RUN chown orchestrator:orchestrator /app/orchestrator_node

# Note: For youki runtime, container requires root or appropriate capabilities
# For production with real containers, run as root or use rootless containers
# For mock runtime, run as non-root user
# USER orchestrator  # Commented out - enable for mock runtime only

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
    CLUSTER_ID=orchestrator-cluster \
    BUNDLE_ROOT=/var/lib/orchestrator/bundles \
    STATE_ROOT=/run/orchestrator \
    YOUKI_BINARY=/usr/local/bin/youki

# Run the orchestrator node
ENTRYPOINT ["/app/orchestrator_node"]
