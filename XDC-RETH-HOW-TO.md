# XDC Network on Reth - Developer Guide

## Overview

This guide explains how to build, configure, and run the XDC Network client based on Reth.

## Prerequisites

### System Requirements
- **OS**: Linux (Ubuntu 22.04+), macOS (12+), Windows (WSL2)
- **RAM**: 32GB minimum (64GB recommended)
- **Disk**: 1TB NVMe SSD (2TB recommended for full archive)
- **CPU**: 8+ cores (16+ recommended)
- **Network**: Stable internet connection for syncing

### Dependencies
```bash
# Rust (latest stable)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env

# System dependencies (Ubuntu)
sudo apt-get update
sudo apt-get install -y \
    build-essential \
    cmake \
    libclang-dev \
    pkg-config \
    libssl-dev

# Verify installation
rustc --version  # Should be 1.75+
cargo --version
```

## Building from Source

### Clone the Repository
```bash
git clone https://github.com/AnilChinchawale/reth-xdc.git
cd reth-xdc
```

### Build Options

#### Full Build (All Features)
```bash
cargo build --release --features xdc
```

#### Build with Debug Symbols
```bash
cargo build --features xdc
```

#### Build Just the XDPoS Consensus
```bash
cargo build -p reth-consensus-xdpos --release
```

#### Build for XDC Mainnet Only
```bash
cargo build --release --features "xdc xdc-mainnet"
```

### Build Artifacts
After building, you'll find:
- Binary: `./target/release/reth`
- Libraries: `./target/release/libreth_*.rlib`

## Running the Client

### Quick Start - XDC Mainnet
```bash
./target/release/reth node \
    --chain xdc-mainnet \
    --datadir ./xdc-data \
    --http \
    --http.api "eth,net,web3,xdc"
```

### Apothem Testnet
```bash
./target/release/reth node \
    --chain xdc-apothem \
    --datadir ./xdc-apothem-data \
    --http \
    --http.api "eth,net,web3,xdc"
```

### Full Configuration Example
```bash
./target/release/reth node \
    --chain xdc-mainnet \
    --datadir /var/lib/xdc-node \
    --http \
    --http.addr 0.0.0.0 \
    --http.port 8545 \
    --http.api "eth,net,web3,xdc,debug" \
    --http.corsdomain "*" \
    --ws \
    --ws.addr 0.0.0.0 \
    --ws.port 8546 \
    --ws.api "eth,net,web3,xdc" \
    --metrics 0.0.0.0:9001 \
    --log.file.directory /var/log/xdc \
    --log.file.max-size 100MB \
    --log.file.max-files 10
```

## Configuration

### Configuration File
Create `xdc-config.toml`:

```toml
[chain]
chain = "xdc-mainnet"

debug = false
log_level = "info"
log_directory = "/var/log/xdc"

[http]
enabled = true
addr = "0.0.0.0"
port = 8545
api = ["eth", "net", "web3", "xdc", "debug"]
corsdomain = ["*"]

[ws]
enabled = true
addr = "0.0.0.0"
port = 8546
api = ["eth", "net", "web3", "xdc"]

[metrics]
enabled = true
addr = "0.0.0.0"
port = 9001

[synchronization]
# Sync mode: full, snap, or light
mode = "full"
# Max peers
max_peers = 50

[xdpos]
# XDPoS-specific settings
epoch = 900
period = 2
gap = 450
# Foundation wallet (mainnet default)
foundation_wallet = "0x7461c..."
# V2 switch block (mainnet default)
v2_switch_block = 56857600
```

Run with config:
```bash
./target/release/reth node --config xdc-config.toml
```

### Environment Variables
```bash
# Log level
export RUST_LOG=info,reth_consensus_xdpos=debug

# Data directory
export RETH_DATA_DIR=/var/lib/xdc-node

# Chain specification
export RETH_CHAIN=xdc-mainnet
```

## Syncing

### Initial Sync
Initial sync from genesis can take 12-24 hours depending on hardware.

```bash
# Start with full sync (default)
./target/release/reth node --chain xdc-mainnet --datadir ./xdc-data
```

### Sync Status
```bash
# Check sync status
curl -X POST -H "Content-Type: application/json" \
    --data '{"jsonrpc":"2.0","method":"eth_syncing","params":[],"id":1}' \
    http://localhost:8545
```

### Check Current Block
```bash
curl -X POST -H "Content-Type: application/json" \
    --data '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}' \
    http://localhost:8545
```

## RPC API

### Standard Ethereum APIs
All standard Ethereum JSON-RPC methods are supported:
- `eth_blockNumber`
- `eth_getBlockByNumber`
- `eth_getBlockByHash`
- `eth_getBalance`
- `eth_sendTransaction`
- `eth_call`
- etc.

### XDC-Specific APIs

#### xdc_getMasternodes
Get the list of current masternodes.
```bash
curl -X POST -H "Content-Type: application/json" \
    --data '{"jsonrpc":"2.0","method":"xdc_getMasternodes","params":["latest"],"id":1}' \
    http://localhost:8545
```

Response:
```json
{
    "jsonrpc": "2.0",
    "id": 1,
    "result": [
        "0x487b5fe5d26dcbede5b73bed424209a",
        "0x..."
    ]
}
```

#### xdc_getEpochInfo
Get current epoch information.
```bash
curl -X POST -H "Content-Type: application/json" \
    --data '{"jsonrpc":"2.0","method":"xdc_getEpochInfo","params":[],"id":1}' \
    http://localhost:8545
```

Response:
```json
{
    "jsonrpc": "2.0",
    "id": 1,
    "result": {
        "current_epoch": 63175,
        "epoch_length": 900,
        "current_block": 56857750,
        "epoch_start_block": 56857500,
        "epoch_end_block": 56858399
    }
}
```

#### xdc_getRoundInfo (V2 only)
Get V2 consensus round information.
```bash
curl -X POST -H "Content-Type: application/json" \
    --data '{"jsonrpc":"2.0","method":"xdc_getRoundInfo","params":["latest"],"id":1}' \
    http://localhost:8545
```

#### xdc_getSnapshot
Get the consensus snapshot at a specific block.
```bash
curl -X POST -H "Content-Type: application/json" \
    --data '{"jsonrpc":"2.0","method":"xdc_getSnapshot","params":["0x100"],"id":1}' \
    http://localhost:8545
```

#### xdc_getCandidates
Get validator candidates.
```bash
curl -X POST -H "Content-Type: application/json" \
    --data '{"jsonrpc":"2.0","method":"xdc_getCandidates","params":["latest"],"id":1}' \
    http://localhost:8545
```

## Monitoring

### Prometheus Metrics
Metrics are available at `http://localhost:9001/metrics` when enabled.

Key XDC metrics:
- `xdpos_current_epoch`: Current epoch number
- `xdpos_current_round`: Current V2 round
- `xdpos_validators_total`: Number of validators
- `xdpos_blocks_proposed_total`: Total blocks proposed
- `xdpos_rewards_distributed_total`: Total rewards distributed

### Health Check
```bash
curl http://localhost:8545/health
```

### Log Monitoring
```bash
# Watch logs
tail -f /var/log/xdc/reth.log

# Filter for consensus logs
tail -f /var/log/xdc/reth.log | grep "XDPoS"
```

## Docker Deployment

### Using Docker
```bash
docker run -d \
    --name xdc-reth \
    -p 8545:8545 \
    -p 8546:8546 \
    -p 30303:30303 \
    -p 30303:30303/udp \
    -p 9001:9001 \
    -v /var/lib/xdc-node:/data \
    xdcnetwork/reth:latest \
    node \
    --chain xdc-mainnet \
    --datadir /data \
    --http \
    --http.addr 0.0.0.0 \
    --http.api "eth,net,web3,xdc"
```

### Docker Compose
```yaml
version: '3.8'

services:
  xdc-reth:
    image: xdcnetwork/reth:latest
    container_name: xdc-reth
    restart: unless-stopped
    ports:
      - "8545:8545"    # HTTP RPC
      - "8546:8546"    # WS RPC
      - "30303:30303"  # P2P TCP
      - "30303:30303/udp"  # P2P UDP
      - "9001:9001"    # Metrics
    volumes:
      - xdc-data:/data
      - ./xdc-config.toml:/config.toml:ro
    command:
      - node
      - --config
      - /config.toml
    environment:
      - RUST_LOG=info
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8545/health"]
      interval: 30s
      timeout: 10s
      retries: 3

volumes:
  xdc-data:
```

Run:
```bash
docker-compose up -d
```

## Troubleshooting

### Build Issues

#### Out of Memory
```bash
# Reduce parallel jobs
cargo build --release --features xdc -j 2

# Or use swap
sudo fallocate -l 16G /swapfile
sudo chmod 600 /swapfile
sudo mkswap /swapfile
sudo swapon /swapfile
```

#### Linker Issues
```bash
# Install lld for faster linking
sudo apt-get install lld

# Add to ~/.cargo/config.toml
[build]
rustflags = ["-C", "link-arg=-fuse-ld=lld"]
```

### Runtime Issues

#### Sync Stuck
```bash
# Check logs for errors
tail -f /var/log/xdc/reth.log | grep -E "(ERROR|WARN)"

# Restart with debug logging
RUST_LOG=debug ./target/release/reth node --chain xdc-mainnet
```

#### State Root Mismatch
If you see state root errors:
1. Check if you're on the correct chain
2. Verify database integrity: `./target/release/reth db --datadir ./xdc-data check`
3. If needed, reset sync: `./target/release/reth node --chain xdc-mainnet --datadir ./xdc-data --full`

#### Low Peer Count
```bash
# Add bootnodes manually
./target/release/reth node \
    --chain xdc-mainnet \
    --bootnodes "enode://...","enode://..."
```

### Database Issues

#### Database Corruption
```bash
# Check database integrity
./target/release/reth db --datadir ./xdc-data check

# Drop database and resync (WARNING: Destroys data!)
rm -rf ./xdc-data
./target/release/reth node --chain xdc-mainnet --datadir ./xdc-data
```

## Development

### Running Tests
```bash
# All tests
cargo test --features xdc

# XDPoS consensus tests only
cargo test -p reth-consensus-xdpos

# Integration tests
cargo test --test xdc_integration --features test-xdc

# With output
cargo test -p reth-consensus-xdpos -- --nocapture
```

### Code Coverage
```bash
cargo tarpaulin -p reth-consensus-xdpos --out Html
```

### Profiling
```bash
# CPU profiling (requires release build with debug symbols)
cargo build --release --features xdc --config 'profile.release.debug = true'
sudo perf record -g ./target/release/reth node --chain xdc-mainnet
perf report
```

### Contributing
1. Fork the repository
2. Create a feature branch: `git checkout -b feature/my-feature`
3. Make changes and add tests
4. Run tests: `cargo test --features xdc`
5. Format code: `cargo fmt`
6. Run clippy: `cargo clippy --features xdc`
7. Commit and push
8. Create Pull Request

## Chain Specifications

### XDC Mainnet
- **Chain ID**: 50
- **Genesis**: 2019-01-01 00:00:00 UTC
- **Block Time**: 2 seconds
- **Epoch**: 900 blocks (~30 minutes)
- **V2 Switch**: Block 56,857,600

### Apothem Testnet
- **Chain ID**: 51
- **Purpose**: Testing and development
- **Faucet**: Available via XDC faucet

## Security Considerations

1. **Firewall**: Only expose necessary ports
2. **API Access**: Restrict HTTP/WS APIs in production
3. **Data Directory**: Ensure proper permissions (700)
4. **Backup**: Regular backups of keystore and data

## Support

- **GitHub Issues**: https://github.com/AnilChinchawale/reth-xdc/issues
- **XDC Documentation**: https://docs.xdc.network
- **Reth Documentation**: https://reth.rs

## License

This project is licensed under the Apache-2.0 License.
