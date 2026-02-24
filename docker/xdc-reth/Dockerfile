# Dockerfile for XDC Reth Client
# Multi-stage build for optimized image size
# Ubuntu base required for glibc compatibility (learned from Nethermind)

# ============================================================================
# Stage 1: Builder - Compile the Rust binary
# ============================================================================
FROM rust:1.82-bookworm AS builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    build-essential \
    libclang-dev \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy the entire project
COPY . .

# Build the xdc-reth binary with XDC-specific features
# Use release profile for optimized performance
RUN cargo build --release --bin xdc-reth --features xdc

# Verify binary exists and is executable
RUN test -f /app/target/release/xdc-reth && \
    chmod +x /app/target/release/xdc-reth

# ============================================================================
# Stage 2: Runtime - Minimal Ubuntu image with just the binary
# ============================================================================
FROM ubuntu:24.04

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    curl \
    jq \
    && rm -rf /var/lib/apt/lists/*

# Copy the compiled binary from builder
COPY --from=builder /app/target/release/xdc-reth /usr/local/bin/xdc-reth

# Copy genesis files
COPY docker/xdc-reth/genesis /genesis

# Copy entrypoint script
COPY docker/xdc-reth/entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh

# Create data directory
RUN mkdir -p /data

# Default environment variables (can be overridden)
ENV NETWORK=mainnet
ENV RPC_ADDR=0.0.0.0
ENV RPC_PORT=8545
ENV WS_ADDR=0.0.0.0
ENV WS_PORT=8546
ENV P2P_PORT=30303
ENV DISCOVERY_PORT=30303
ENV DATA_DIR=/data

# Expose ports
# P2P discovery and communication
EXPOSE 30303/tcp 30303/udp
# RPC port
EXPOSE 8545/tcp
# WebSocket port
EXPOSE 8546/tcp
# Metrics port (optional)
EXPOSE 9001/tcp

# Set working directory
WORKDIR /data

# Health check - verify RPC endpoint is responding
HEALTHCHECK --interval=30s --timeout=10s --start-period=60s --retries=3 \
    CMD curl -f http://localhost:${RPC_PORT}/health || exit 1

# Run as root for volume permissions (learned from NM Docker)
# Alternative: use --user flag in docker run with proper UID/GID

ENTRYPOINT ["/entrypoint.sh"]

# Default: no extra flags (can be overridden in docker-compose or docker run)
CMD []
