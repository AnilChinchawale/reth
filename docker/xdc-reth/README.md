# XDC Reth Docker

Docker images and configuration for running [reth-xdc](https://github.com/XinFinOrg/XDPoS-Chain) - an XDC-compatible execution client built on Reth.

## Overview

This directory contains Docker infrastructure for building and running XDC Reth:

- **Dockerfile**: Multi-stage build (Rust builder + Ubuntu runtime)
- **docker-compose.yml**: Simple configuration for running a node
- **entrypoint.sh**: Initialization and startup script
- **genesis/**: Genesis files for mainnet and Apothem testnet
- **build.sh**: Build script for local and CI usage

## Quick Start

### Using Docker Compose (Recommended)

1. Clone this repository and navigate to the docker directory:
```bash
cd docker/xdc-reth
```

2. Start the node:
```bash
# For mainnet
docker-compose up -d

# For Apothem testnet
NETWORK=apothem docker-compose up -d
```

3. Check logs:
```bash
docker-compose logs -f
```

4. Stop the node:
```bash
docker-compose down
```

### Using Docker Run

```bash
docker run -d \
  --name xdc-reth \
  -p 30303:30303/tcp \
  -p 30303:30303/udp \
  -p 8545:8545 \
  -p 8546:8546 \
  -v $(pwd)/data:/data \
  -e NETWORK=mainnet \
  anilchinchawale/reth-xdc:latest
```

## Building from Source

### Prerequisites

- Docker 20.10+
- Docker Buildx (for multi-arch builds)
- Git

### Build Script Usage

```bash
cd docker/xdc-reth

# Build for current architecture only (fast, for local testing)
./build.sh --single-arch

# Build multi-arch (amd64 + arm64) - requires Docker Buildx
./build.sh

# Build and push to Docker Hub
./build.sh --push

# Build specific version tag
./build.sh --tag v1.0.0 --push

# Show all options
./build.sh --help
```

### Manual Build

```bash
# Build from project root
docker build -f docker/xdc-reth/Dockerfile -t anilchinchawale/reth-xdc:latest .

# Multi-arch build
docker buildx build --platform linux/amd64,linux/arm64 -t anilchinchawale/reth-xdc:latest .
```

## Configuration

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `NETWORK` | `mainnet` | Network to connect to (`mainnet` or `apothem`) |
| `RPC_ADDR` | `0.0.0.0` | RPC HTTP server bind address |
| `RPC_PORT` | `8545` | RPC HTTP server port |
| `WS_ADDR` | `0.0.0.0` | WebSocket server bind address |
| `WS_PORT` | `8546` | WebSocket server port |
| `P2P_PORT` | `30303` | P2P network port |
| `DISCOVERY_PORT` | `30303` | P2P discovery port |
| `MAX_PEERS` | `50` | Maximum peer connections |
| `CUSTOM_BOOTNODES` | - | Comma-separated list of bootnode enode URLs |
| `ENABLE_METRICS` | `false` | Enable Prometheus metrics endpoint |
| `METRICS_PORT` | `9001` | Prometheus metrics port |
| `DATA_DIR` | `/data` | Data directory inside container |

### Volume Mounts

| Host Path | Container Path | Description |
|-----------|----------------|-------------|
| `./data` | `/data` | Blockchain data and node database |

### Network Ports

| Port | Protocol | Description |
|------|----------|-------------|
| 30303 | TCP/UDP | P2P network and discovery |
| 8545 | TCP | HTTP RPC API |
| 8546 | TCP | WebSocket API |
| 9001 | TCP | Metrics (optional) |

## Integration with xdc-node-setup

### Standalone Mode

Use `docker/docker-compose.reth.yml` for running XDC Reth as a standalone service:

```bash
docker-compose -f docker/docker-compose.reth.yml up -d
```

### Override Mode

Use `docker/docker-compose.reth-override.yml` to integrate with existing `xdc-node-setup` infrastructure:

```bash
# From xdc-node-setup directory
docker-compose -f docker-compose.yml -f ../reth-xdc/docker/docker-compose.reth-override.yml up -d
```

## SkyNet Monitoring

The `skynet-reth.conf` file provides configuration for SkyNet monitoring agent:

- Monitors container health and sync status
- Tracks peer count and RPC availability
- Alerts on sync issues or low peer count
- Configurable thresholds and intervals

## Genesis Files

Genesis files are located in `genesis/`:

- `mainnet.json`: XDC Mainnet (Chain ID: 50)
- `apothem.json`: XDC Apothem Testnet (Chain ID: 51)

The genesis hash for mainnet is: `0x4a9d748bd78a8d0385b67788c2435dcdb914f98a96250b68863a1f8b7642d6b1`

## Key Design Decisions

### Ubuntu Base Image

We use **Ubuntu 24.04** as the runtime base (not Alpine) because:
- Reth requires glibc for proper operation
- Learned from Nethermind Docker issues with musl libc

### Root User

The container runs as root for volume permissions (learned from NM Docker):
- Docker creates directories when mount source is missing
- Non-root users often encounter permission issues with bind mounts
- Users can override with `--user` flag if needed

### Genesis Initialization

The entrypoint script automatically:
1. Detects first run (no database exists)
2. Initializes genesis block if needed
3. Configures bootnodes based on network
4. Sets up state root cache directory

### Bootnodes

Bootnodes are **critical** - the node cannot discover peers without them (learned from GP5):
- Mainnet and Apothem have default bootnode lists
- Custom bootnodes can be set via `CUSTOM_BOOTNODES` env var

## Troubleshooting

### Container fails to start

Check logs:
```bash
docker-compose logs xdc-reth
```

### Permission denied on data directory

Ensure the host directory has correct permissions:
```bash
sudo chown -R $(id -u):$(id -g) ./data
```

Or run with proper user ID:
```bash
docker run --user $(id -u):$(id -g) ...
```

### No peers / Stuck syncing

Verify bootnodes are configured:
```bash
# Check environment variable
docker exec xdc-reth-mainnet env | grep BOOTNODES

# Set custom bootnodes
docker run -e CUSTOM_BOOTNODES="enode://...@ip:port,..." ...
```

### Database corruption

If the database becomes corrupted:
```bash
# Stop container
docker-compose down

# Remove data directory (WARNING: This deletes blockchain data!)
rm -rf ./data/db

# Restart - genesis will be re-initialized
docker-compose up -d
```

## Resources

- [XDC Network](https://xdc.org)
- [XDC Reth Documentation](../../XDC-RETH-HOW-TO.md)
- [Reth Book](https://paradigmxyz.github.io/reth/)

## License

See the main project [LICENSE](../../LICENSE) file.

## Contributing

See [CONTRIBUTING.md](../../CONTRIBUTING.md) for guidelines.
