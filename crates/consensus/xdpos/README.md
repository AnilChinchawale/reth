# XDPoS Consensus for Reth

This crate implements the XDC Network's XDPoS (XDC Delegated Proof of Stake) consensus algorithm for the Reth Ethereum client.

## Overview

XDPoS is a consensus mechanism used by the XDC Network with the following characteristics:

- **Epoch-based**: 900 blocks per epoch (~30 minutes)
- **Fast finality**: 2-second block time
- **V1**: Delegated Proof of Stake with checkpoint rewards
- **V2**: BFT consensus with Quorum Certificates (QC) and Timeout Certificates (TC)

## Features

- Full XDPoS V1 support (epoch-based consensus)
- Full XDPoS V2 support (BFT with QC/TC)
- Snapshot management for validator tracking
- Reward calculation and distribution
- Block and header validation

## Architecture

```
crates/consensus/xdpos/src/
├── lib.rs           # Public exports and constants
├── xdpos.rs         # Main consensus engine
├── config.rs        # XDPoS configuration types
├── errors.rs        # Error types
├── snapshot.rs      # Validator snapshot management
├── reward.rs        # Reward calculation
├── validation.rs    # Validation utilities
├── v1.rs            # V1 validation logic
└── v2/              # V2 BFT consensus
    ├── mod.rs       # V2 types
    └── engine.rs    # V2 engine implementation
```

## Usage

```rust
use reth_consensus_xdpos::{XDPoSConsensus, XDPoSConfig, xdc_mainnet_config};

// Create consensus engine
let config = xdc_mainnet_config();
let consensus = XDPoSConsensus::new(config);

// Use with Reth node
// ...
```

## Testing

```bash
# Run all tests
cargo test -p reth-consensus-xdpos

# Run with output
cargo test -p reth-consensus-xdpos -- --nocapture
```

## XDC Network Parameters

### Mainnet
- Chain ID: 50
- Epoch: 900 blocks
- Period: 2 seconds
- V2 Switch: Block 56,857,600

### Apothem Testnet
- Chain ID: 51
- Same parameters as mainnet (for testing)

## References

- [XDC Network](https://xinfin.org)
- [XDPoS Documentation](https://docs.xdc.network)
- [Reth Documentation](https://reth.rs)
