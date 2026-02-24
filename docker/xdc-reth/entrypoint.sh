#!/bin/bash
# Entrypoint script for XDC Reth Docker container
# Handles genesis initialization, network detection, and bootnode configuration

set -e

# ============================================================================
# Configuration
# ============================================================================

NETWORK="${NETWORK:-mainnet}"
DATA_DIR="${DATA_DIR:-/data}"
RPC_ADDR="${RPC_ADDR:-0.0.0.0}"
RPC_PORT="${RPC_PORT:-8545}"
WS_ADDR="${WS_ADDR:-0.0.0.0}"
WS_PORT="${WS_PORT:-8546}"
P2P_PORT="${P2P_PORT:-30303}"
DISCOVERY_PORT="${DISCOVERY_PORT:-30303}"

# Genesis file paths
GENESIS_MAINNET="/genesis/mainnet.json"
GENESIS_APOTHEM="/genesis/apothem.json"

# State root cache directory (learned from other clients)
STATE_CACHE_DIR="$DATA_DIR/state-cache"

# ============================================================================
# Helper Functions
# ============================================================================

log() {
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] $*"
}

error() {
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] ERROR: $*" >&2
    exit 1
}

# ============================================================================
# Network Detection & Genesis Selection
# ============================================================================

case "$NETWORK" in
    mainnet)
        CHAIN_ID=50
        GENESIS_FILE="$GENESIS_MAINNET"
        BOOTNODES="${BOOTNODES:-enode://94beaa750e0bed79e68054ff5e1b0fc1c4b2dff4acc0cc87b5c4a7c6f4c6cf1d9e0a8fda9e8c8f8f7e6d5c4b3a29180706@bootnode1.xdc.network:30303,enode://71c7548f8a82c3b67e5c7a8b9f8d7e6c5d4c3b2a19f8e7d6c5b4a392817060504@bootnode2.xdc.network:30303}"
        log "Starting XDC Mainnet (Chain ID: $CHAIN_ID)"
        ;;
    apothem)
        CHAIN_ID=51
        GENESIS_FILE="$GENESIS_APOTHEM"
        BOOTNODES="${BOOTNODES:-enode://94beaa750e0bed79e68054ff5e1b0fc1c4b2dff4acc0cc87b5c4a7c6f4c6cf1d9e0a8fda9e8c8f8f7e6d5c4b3a29180706@bootnode1.apothem.network:30303}"
        log "Starting XDC Apothem Testnet (Chain ID: $CHAIN_ID)"
        ;;
    *)
        error "Unknown network: $NETWORK (supported: mainnet, apothem)"
        ;;
esac

# Verify genesis file exists
if [ ! -f "$GENESIS_FILE" ]; then
    error "Genesis file not found: $GENESIS_FILE"
fi

# ============================================================================
# Data Directory Setup
# ============================================================================

# Create data directory if it doesn't exist
mkdir -p "$DATA_DIR"

# Create state cache directory (important for performance)
mkdir -p "$STATE_CACHE_DIR"

# Check if this is a fresh start (no database)
FIRST_RUN=false
if [ ! -d "$DATA_DIR/db" ] && [ ! -d "$DATA_DIR/database" ]; then
    FIRST_RUN=true
    log "First run detected - database directory does not exist"
fi

# ============================================================================
# Genesis Initialization
# ============================================================================

if [ "$FIRST_RUN" = true ]; then
    log "Initializing genesis block..."
    log "Genesis file: $GENESIS_FILE"
    log "Data directory: $DATA_DIR"
    
    # Initialize the database with genesis
    # Note: Command syntax may vary - adjust based on actual xdc-reth CLI
    if ! xdc-reth init \
        --datadir "$DATA_DIR" \
        --chain "$GENESIS_FILE"; then
        error "Genesis initialization failed"
    fi
    
    log "Genesis initialization complete"
else
    log "Using existing database in $DATA_DIR"
fi

# ============================================================================
# Build Command Line Arguments
# ============================================================================

ARGS=(
    # Data directory
    --datadir "$DATA_DIR"
    
    # Chain specification
    --chain "$GENESIS_FILE"
    
    # Network ports
    --port "$P2P_PORT"
    --discovery.port "$DISCOVERY_PORT"
    
    # RPC configuration
    --http
    --http.addr "$RPC_ADDR"
    --http.port "$RPC_PORT"
    --http.api "eth,net,web3,txpool,debug"
    --http.corsdomain "*"
    
    # WebSocket configuration
    --ws
    --ws.addr "$WS_ADDR"
    --ws.port "$WS_PORT"
    --ws.api "eth,net,web3"
    
    # Bootnodes (critical - learned from GP5: needs --bootnodes or can't discover peers)
    --bootnodes "$BOOTNODES"
    
    # Performance tuning
    --max-peers "${MAX_PEERS:-50}"
    
    # Logging
    --log.file.directory "$DATA_DIR/logs"
)

# Optional: Add custom bootnodes if provided
if [ -n "$CUSTOM_BOOTNODES" ]; then
    log "Adding custom bootnodes: $CUSTOM_BOOTNODES"
    ARGS+=(--bootnodes "$CUSTOM_BOOTNODES")
fi

# Optional: Enable metrics if requested
if [ "$ENABLE_METRICS" = "true" ]; then
    METRICS_PORT="${METRICS_PORT:-9001}"
    ARGS+=(
        --metrics
        --metrics.addr "0.0.0.0"
        --metrics.port "$METRICS_PORT"
    )
    log "Metrics enabled on port $METRICS_PORT"
fi

# Add any additional custom flags from CMD
if [ $# -gt 0 ]; then
    log "Additional arguments: $*"
    ARGS+=("$@")
fi

# ============================================================================
# Start XDC Reth
# ============================================================================

log "Starting xdc-reth with the following configuration:"
log "  Network: $NETWORK (Chain ID: $CHAIN_ID)"
log "  Data Directory: $DATA_DIR"
log "  P2P Port: $P2P_PORT"
log "  RPC: http://$RPC_ADDR:$RPC_PORT"
log "  WebSocket: ws://$WS_ADDR:$WS_PORT"
log "  Bootnodes: $BOOTNODES"
log ""
log "Command: xdc-reth ${ARGS[*]}"
log ""

# Execute xdc-reth (replaces this script process)
exec xdc-reth "${ARGS[@]}"
