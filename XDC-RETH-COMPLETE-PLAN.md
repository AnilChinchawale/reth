}
}

**Task 10.1.2: Implement RPC Handlers** (12 hours)

```rust
// Location: crates/rpc/rpc-xdc/src/handlers.rs

use crate::{XdcApiServer, XdcResult};
use reth_consensus_xdpos::XDPoSConsensus;
use reth_provider::{StateProviderFactory, BlockReader};

pub struct XdcRpcHandler<P, C> {
    provider: P,
    consensus: Arc<XDPoSConsensus<C>>,
}

#[async_trait::async_trait]
impl<P, C> XdcApiServer for XdcRpcHandler<P, C>
where
    P: StateProviderFactory + BlockReader + Send + Sync,
    C: XdcChainSpec + Send + Sync,
{
    async fn get_masternodes(
        &self,
        block_number: Option<BlockNumber>,
    ) -> RpcResult<Vec<Address>> {
        let block_num = block_number.unwrap_or(BlockNumberOrTag::Latest);
        let header = self.provider
            .header_by_number_or_tag(block_num)
            .map_err(|e| RpcError::InternalError(format!("Header lookup failed: {}", e)))?
            .ok_or(RpcError::UnknownBlock)?;
        
        // Get snapshot for this block
        let snapshot = self.consensus
            .get_snapshot(&header)
            .await
            .map_err(|e| RpcError::InternalError(format!("Snapshot failed: {}", e)))?;
        
        Ok(snapshot.signers.into_iter().collect())
    }

    async fn get_epoch_info(&self) -> RpcResult<EpochInfo> {
        let latest = self.provider
            .last_block_number()
            .map_err(|e| RpcError::InternalError(e.to_string()))?;
        
        let config = self.consensus.config();
        let epoch_number = latest / config.epoch;
        let blocks_until_epoch = config.epoch - (latest % config.epoch);
        let blocks_until_gap = config.gap.saturating_sub(latest % config.epoch);
        
        Ok(EpochInfo {
            epoch: config.epoch,
            epoch_length: config.epoch,
            current_epoch_number: epoch_number,
            blocks_until_epoch,
            gap: config.gap,
            blocks_until_gap,
        })
    }
}
```

### 10.2 eth_* Compatibility

**Task 10.2.1: XDC Address Prefix Handling** (4 hours)

XDC uses "xdc..." prefix instead of "0x..." for addresses in user interfaces. The RPC layer should handle both:

```rust
// Location: crates/rpc/rpc-xdc/src/address.rs

/// Convert between XDC and Ethereum address formats
pub fn normalize_address(addr: &str) -> Result<Address, AddressError> {
    let normalized = if addr.starts_with("xdc") {
        addr.replacen("xdc", "0x", 1)
    } else if addr.starts_with("0x") {
        addr.to_string()
    } else {
        return Err(AddressError::InvalidPrefix);
    };
    
    normalized.parse::<Address>()
        .map_err(AddressError::from)
}

pub fn to_xdc_format(addr: Address) -> String {
    format!("xdc{}", hex::encode(addr.as_slice()))
}
```

### 10.3 Estimated Hours Summary

| Task | Hours |
|------|-------|
| 10.1.1 RPC trait definition | 16 |
| 10.1.2 RPC handlers | 12 |
| 10.1.3 XDC namespace registration | 4 |
| 10.2.1 Address format handling | 4 |
| 10.3 Error handling | 4 |
| **Phase 10 Total** | **40** |

---

## Phase 11: Docker & Deployment (Week 11-12)

### 11.1 Dockerfile

**Task 11.1.1: Multi-Stage Docker Build** (8 hours)

```dockerfile
# Location: Dockerfile

# Stage 1: Builder
FROM rust:1.82-bookworm AS builder

WORKDIR /app

# Install dependencies
RUN apt-get update && apt-get install -y \
    libclang-dev \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy manifests
COPY Cargo.toml Cargo.lock ./
COPY crates/ ./crates/

# Build release binary
RUN cargo build --release --features xdc --bin reth

# Stage 2: Runtime
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    curl \
    jq \
    && rm -rf /var/lib/apt/lists/*

# Copy binary from builder
COPY --from=builder /app/target/release/reth /usr/local/bin/reth

# Create data directory
RUN mkdir -p /data/xdc

# XDC-specific environment
ENV RETH_DATADIR=/data/xdc
ENV RETH_CHAIN=xdcmainnet
ENV RETH_RPC_HTTP=true
ENV RETH_RPC_HTTP_ADDR=0.0.0.0
ENV RETH_RPC_HTTP_PORT=8545
ENV RETH_RPC_WS=true
ENV RETH_RPC_WS_ADDR=0.0.0.0
ENV RETH_RPC_WS_PORT=8546
ENV RETH_P2P_ADDR=0.0.0.0
ENV RETH_P2P_PORT=30303

# Healthcheck
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:8545 -X POST \
        -H "Content-Type: application/json" \
        -d '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}' \
        || exit 1

EXPOSE 8545 8546 30303/tcp 30303/udp

ENTRYPOINT ["reth"]
CMD ["node", "--full"]
```

### 11.2 xdc-node-setup Integration

**Task 11.2.1: Add Reth to CLI Setup** (8 hours)

```bash
# Location: xdc-node-setup/cli/commands/start.sh

start_reth() {
    local network="${1:-mainnet}"
    local datadir="/mnt/data/${network}/reth"
    
    docker run -d \
        --name "xdc-reth-${network}" \
        --restart unless-stopped \
        -v "${datadir}:/data/xdc" \
        -p 8545:8545 \
        -p 8546:8546 \
        -p 30303:30303 \
        -p 30303:30303/udp \
        -e RETH_CHAIN="xdc${network}" \
        anilchinchawale/reth-xdc:latest \
        node --full
}
```

**Task 11.2.2: Docker Compose** (4 hours)

```yaml
# Location: docker-compose.reth.yml

services:
  xdc-reth:
    image: anilchinchawale/reth-xdc:latest
    container_name: xdc-reth-${NETWORK:-mainnet}
    restart: unless-stopped
    volumes:
      - /mnt/data/${NETWORK}/reth:/data/xdc
      - /etc/xdc-node/skynet.conf:/etc/xdc-node/skynet.conf:ro
    ports:
      - "8545:8545"
      - "8546:8546"
      - "30303:30303"
      - "30303:30303/udp"
    environment:
      - RETH_CHAIN=xdcmainnet
      - RETH_RPC_HTTP=true
      - RETH_RPC_WS=true
      - RETH_P2P_PORT=30303
    networks:
      - xdc-network
    labels:
      - "skynet.agent=true"
      - "skynet.network=${NETWORK:-mainnet}"

  skynet-agent:
    image: anilchinchawale/xdc-agent:latest
    container_name: xdc-agent-reth-${NETWORK:-mainnet}
    network_mode: host
    volumes:
      - /var/run/docker.sock:/var/run/docker.sock:ro
      - /etc/xdc-node:/etc/xdc-node:ro
    environment:
      - SKYNET_CONF=/etc/xdc-node/skynet.conf
      - RPC_URL=http://localhost:8545
      - XDC_CONTAINER_NAME=xdc-reth-${NETWORK:-mainnet}
      - HEARTBEAT_INTERVAL=30
    depends_on:
      - xdc-reth

networks:
  xdc-network:
    driver: bridge
```

### 11.3 SkyOne Agent Compatibility

The agent was built with multi-client support. For Reth specifically:

- Client type detection via `web3_clientVersion` RPC
- Expected version string: `reth/vX.Y.Z-dev/linux-amd64/rustc`
- Maps to display name: "Reth" in SkyNet dashboard
- Uses `net_peerCount` for peer monitoring
- Uses `eth_syncing` for sync status
- No `admin_*` RPC (like Erigon), so peer injection not possible directly

### 11.4 SkyNet Registration

Auto-registration works identically to other clients:

```bash
# Agent generates smart name: reth-v0.1.0-fullnode-{IP}-{network}
# Registers with SkyNet API at /nodes/register
# Sends heartbeats every 30 seconds
```

### 11.5 Cross-Client Peering Setup

Since Reth lacks `admin_addPeer`:

1. GP5/Nethermind nodes must connect TO Reth's eth/63 sentry
2. Add Reth as static peer in GP5/Nethermind configs
3. Reth can only listen; cannot initiate peering

### 11.6 Estimated Hours Summary

| Task | Hours |
|------|-------|
| 11.1.1 Dockerfile | 8 |
| 11.2.1 CLI integration | 8 |
| 11.2.2 Docker Compose | 4 |
| 11.3 Agent compatibility | 4 |
| 11.4 Documentation | 4 |
| **Phase 11 Total** | **28** |

---

## Phase 12: Testing & Validation (Week 12-14)

### 12.1 Unit Tests

**Task 12.1.1: Comprehensive Test Suite** (40 hours)

```rust
// Location: crates/consensus/xdpos/src/tests/

#[cfg(test)]
mod tests {
    use super::*;
    use reth_chainspec::xdcmainnet;
    
    // V1 Tests
    #[test]
    fn test_v1_header_validation() {
        let consensus = XDPoSConsensus::new(xdcmainnet());
        let header = create_v1_test_header();
        
        assert!(consensus.validate_header(&header).is_ok());
    }
    
    #[test]
    fn test_seal_verification() {
        let consensus = XDPoSConsensus::new(xdcmainnet());
        let header = load_mainnet_header(1800);
        
        assert!(consensus.verify_seal(&header).is_ok());
    }
    
    #[test]
    fn test_checkpoint_validation() {
        let consensus = XDPoSConsensus::new(xdcmainnet());
        let header = load_mainnet_header(900); // First checkpoint
        
        // Should validate masternode list
        assert!(consensus.validate_checkpoint(&header).is_ok());
    }
    
    // V2 Tests
    #[test]
    fn test_v2_qc_verification() {
        let v2_engine = XDPoSV2Engine::new(v2_mainnet_config());
        let header = load_mainnet_header(56_857_601); // First V2 block
        let qc = extract_qc(&header);
        
        assert!(v2_engine.verify_qc(&qc, &header).is_ok());
    }
    
    #[test]
    fn test_vote_signature() {
        let v2_engine = XDPoSV2Engine::new(v2_mainnet_config());
        let vote = create_test_vote();
        let masternodes = test_masternodes();
        
        assert!(v2_engine.verify_vote_signature(&vote, &masternodes).is_ok());
    }
    
    // Reward Tests
    #[test]
    fn test_reward_calculation_checkpoint_1800() {
        let consensus = XDPoSConsensus::new(xdcmainnet());
        let header = load_mainnet_header(1800);
        
        let rewards = consensus.calculate_rewards(&header).unwrap();
        
        // Verify against known mainnet values
        assert_eq!(rewards.total, expected_reward_1800());
        assert!(rewards.foundation > U256::ZERO);
    }
    
    // Special Transaction Tests
    #[test]
    fn test_tipsigning_gas_exemption() {
        let processor = XdcTransactionProcessor::new(xdcmainnet());
        let tx = create_blocksigner_tx_after_3m();
        
        let result = processor.process_transaction(tx, BlockNumber::from(3_000_001));
        
        assert_eq!(result.gas_used, 0);
    }
    
    // State Root Tests
    #[test]
    fn test_state_root_cache() {
        let cache = XdcStateRootCache::new();
        
        cache.insert(1800, remote_root_1800(), local_root_1800());
        
        let mapping = cache.get_mapping(1800);
        assert_eq!(mapping.remote, remote_root_1800());
        assert_eq!(mapping.local, local_root_1800());
    }
    
    #[test]
    fn test_state_root_bypass() {
        let consensus = XDPoSConsensus::new(xdcmainnet());
        let header = load_mainnet_header(1800);
        
        // Should accept block even with divergent state root
        assert!(consensus.validate_state_root_with_bypass(&header).is_ok());
    }
}
```

### 12.2 Integration Tests

**Task 12.2.1: Sync Tests** (24 hours)

```rust
// Location: crates/consensus/xdpos/tests/sync_tests.rs

#[tokio::test]
async fn test_sync_from_genesis_mainnet() {
    let node = XdcNode::test_node(xdcmainnet()).await;
    
    // Sync first 2000 blocks (genesis through first checkpoint)
    for block_num in 0..2000 {
        let block = fetch_mainnet_block(block_num).await;
        let result = node.import_block(block).await;
        
        assert!(result.is_ok(), "Block {} import failed: {:?}", block_num, result);
    }
    
    // Verify state roots match at checkpoint
    let checkpoint = node.get_header(1800);
    assert_eq!(checkpoint.state_root, expected_checkpoint_1800_root());
}

#[tokio::test]
async fn test_sync_v2_transition() {
    let node = XdcNode::test_node(xdcmainnet()).await;
    
    // Fast-forward to V2 transition
    let v2_block = 56_857_600;
    let header = fetch_mainnet_block(v2_block).await;
    
    // Should validate V2 QC
    assert!(node.import_block(header).await.is_ok());
    
    // Verify round info
    let round = node.consensus().get_current_round();
    assert!(round > 0);
}
```

### 12.3 Apothem Testnet Validation

**Task 12.3.1: Apothem Sync to Tip** (16 hours)

1. Deploy Reth-XDC node on Apothem testnet
2. Sync from genesis to current tip (~79M blocks as of Feb 2026)
3. Monitor for:
   - State root mismatches
   - Block validation failures
   - Peer connection issues
   - Memory/performance

Expected timeline: 3-5 days for full Apothem sync

### 12.4 Mainnet Validation

**Task 12.4.1: Mainnet Sync Validation** (32 hours)

1. Deploy on TEST server alongside existing GP5
2. Sync from genesis, comparing state roots at checkpoints:
   - Block 1800 (first checkpoint)
   - Block 9000
   - Block 18000
   - Every 900 blocks thereafter
3. Document any divergences
4. Fix and iterate

### 12.5 Cross-Client Peering

**Task 12.5.1: Verify Peering** (8 hours)

1. Ensure Reth can peer with:
   - GP5 nodes (via eth/100, eth/62, eth/63)
   - Nethermind nodes (via eth/100)
   - Erigon nodes (via eth/63)
2. Verify block propagation
3. Test during V1→V2 transition blocks

### 12.6 Performance Benchmarks

**Task 12.6.1: Benchmark Suite** (12 hours)

| Metric | Target | Test |
|--------|--------|------|
| Block processing rate | >500 blocks/sec | Time to process 10K blocks |
| State root computation | <100ms | Average across 100 blocks |
| Memory usage | <8GB | Peak during sync |
| Peer connections | >25 peers | After 1 hour uptime |
| RPC latency | <50ms | eth_blockNumber p99 |

### 12.7 Estimated Hours Summary

| Task | Hours |
|------|-------|
| 12.1.1 Unit tests | 40 |
| 12.2.1 Integration tests | 24 |
| 12.3.1 Apothem validation | 16 |
| 12.4.1 Mainnet validation | 32 |
| 12.5.1 Cross-client peering | 8 |
| 12.6.1 Performance benchmarks | 12 |
| **Phase 12 Total** | **132** |

---

## Risk Matrix

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| **V2 QC verification complexity** | High | Critical | Extensive testing with mainnet data; use reference vectors from GP5/Erigon |
| **State root mismatch** | High | Critical | XdcStateRootCache with disk persistence; verified bypass for known blocks |
| **P2P protocol changes** | Medium | High | Follow existing XDC wire protocol exactly; dual sentry architecture |
| **Reward calculation errors** | Medium | High | Unit tests with known checkpoint data; compare outputs byte-for-byte |
| **Performance issues** | Medium | Medium | Profile hot paths; optimize RLP encoding; benchmark continuously |
| **BigBalance overflow** | Low | Critical | Use U256 with overflow checks; test with Apothem genesis |
| **TIPSigning implementation** | Medium | High | Test both legacy and staged sync paths; verify gas at 3M+ blocks |
| **EIP-158 side effects** | Medium | Medium | Explicitly disable; test state after empty account creation |
| **MDBX state root cache** | Medium | Critical | Backup/restore logic; fallback to in-memory if DB unavailable |
| **Rust trait compatibility** | Medium | High | Frequent rebase with upstream Reth; maintain compatibility layer |

---

## Dependency Graph

```
Phase 1 (Foundation) → ALL PHASES

Phase 2 (V1 Consensus) → Phase 3, 4, 5, 6, 8
    ↓
Phase 3 (Rewards) → Phase 6, 8, 12
    ↓
Phase 4 (V2 BFT) → Phase 7, 12
    ↓
Phase 5 (Special TX) → Phase 6, 8, 12
    ↓
Phase 6 (State Root) → Phase 8, 12
    ↓
Phase 7 (P2P) → Phase 8, 11, 12
    ↓
Phase 8 (Sync Engine) → Phase 9, 11, 12
    ↓
Phase 9 (Chain Spec) → Phase 10, 11, 12
    ↓
Phase 10 (RPC) → Phase 11, 12
    ↓
Phase 11 (Docker) → Phase 12
    ↓
Phase 12 (Testing)
```

**Critical Path**: 1 → 2 → 3 → 6 → 8 → 9 → 12 (minimum viable sync)

---

## File-by-File Implementation Checklist

| File | Phase | Status |
|------|-------|--------|
| `crates/consensus/xdpos/Cargo.toml` | 1 | ⬜ |
| `crates/consensus/xdpos/src/lib.rs` | 1 | ⬜ |
| `crates/consensus/xdpos/src/config.rs` | 1 | ⬜ |
| `crates/consensus/xdpos/src/errors.rs` | 1 | ⬜ |
| `crates/consensus/xdpos/src/snapshot.rs` | 2 | ⬜ |
| `crates/consensus/xdpos/src/v1.rs` | 2 | ⬜ |
| `crates/consensus/xdpos/src/v2/mod.rs` | 4 | ⬜ |
| `crates/consensus/xdpos/src/v2/engine.rs` | 4 | ⬜ |
| `crates/consensus/xdpos/src/v2/types.rs` | 4 | ⬜ |
| `crates/consensus/xdpos/src/v2/verification.rs` | 4 | ⬜ |
| `crates/consensus/xdpos/src/v2/epoch_switch.rs` | 4 | ⬜ |
| `crates/consensus/xdpos/src/reward.rs` | 3 | ⬜ |
| `crates/consensus/xdpos/src/validation.rs` | 2 | ⬜ |
| `crates/consensus/xdpos/src/state_root_cache.rs` | 6 | ⬜ |
| `crates/consensus/xdpos/src/special_tx.rs` | 5 | ⬜ |
| `crates/chainspec/src/xdc/mod.rs` | 2, 9 | ⬜ |
| `crates/chainspec/src/xdc/mainnet.rs` | 9 | ⬜ |
| `crates/chainspec/src/xdc/apothem.rs` | 9 | ⬜ |
| `crates/evm/src/xdc/transaction_processor.rs` | 5 | ⬜ |
| `crates/evm/src/xdc/processor.rs` | 5 | ⬜ |
| `crates/net/p2p/src/xdc/protocol.rs` | 7 | ⬜ |
| `crates/net/p2p/src/xdc/sentry.rs` | 7 | ⬜ |
| `crates/net/p2p/src/xdc/handlers.rs` | 7 | ⬜ |
| `crates/sync/src/stages/execution_xdc.rs` | 8 | ⬜ |
| `crates/sync/src/xdc/orphaned_blocks.rs` | 8 | ⬜ |
| `crates/rpc/rpc-xdc/src/lib.rs` | 10 | ⬜ |
| `crates/rpc/rpc-xdc/src/handlers.rs` | 10 | ⬜ |
| `crates/rpc/rpc-xdc/src/address.rs` | 10 | ⬜ |
| `Dockerfile` | 11 | ⬜ |
| `docker-compose.reth.yml` | 11 | ⬜ |

---

## Test Vectors (Mainnet Data)

### Checkpoint 1800 (First Reward Block)
- **Hash**: `0x...`
- **State Root**: (from geth: 0x..., expected Reth: 0x...)
- **Foundation Reward**: 250 XDC
- **Validator Count**: ~18 masternodes

### V2 Transition Block 56,857,600
- **Hash**: `0x...`
- **Round**: 1
- **QC Signatures**: ~18 validators
- **Parent Hash**: Block 56,857,599

### TIPSigning Block 3,000,000
- **BlockSigner TX**: 0x89... with gas = 0
- **Gas Used**: 0 (not actual computation)

### Apothem Genesis
- **Hash**: (differs from mainnet due to BigBalance)
- **Account 0x...**: Balance = 2^256 - 1 (max U256)

---

## Feature Comparison Matrix

| Feature | v2.6.8 (Ref) | GP5 | Erigon | Nethermind | **Reth (Target)** |
|---------|--------------|-----|--------|------------|-------------------|
| **V1 Consensus** | ✅ | ✅ | ✅ | ✅ | ⬜ |
| **V2 BFT** | ✅ | ✅ | ✅ | ❌ | ⬜ |
| **Proportional Rewards** | ✅ | ✅ | ~80% | ✅ | ⬜ |
| **eth/100 Protocol** | ✅ | ✅ | ❌ | ✅ | ⬜ |
| **eth/62 Support** | ✅ | ✅ | ✅ | ❌ | ⬜ |
| **eth/63 Support** | ✅ | ✅ | ✅ | ❌ | ⬜ |
| **TIPSigning (3M+)** | ✅ | ✅ | ✅ | ✅ | ⬜ |
| **State Root Cache** | N/A | ✅ | ✅ | ✅ | ⬜ |
| **BigBalance (Apothem)** | ✅ | ✅ | ❌ | ❌ | ⬜ |
| **Snap Sync** | ❌ | ❌ | N/A | ❌ | ⬜ |
| **Block Signing TX** | ✅ | ✅ | ~50% | ✅ | ⬜ |
| **0x88 Contract** | ✅ | ✅ | ✅ | ✅ | ⬜ |
| **0x89 Contract** | ✅ | ✅ | ✅ | ✅ | ⬜ |
| **0x90 Contract** | ✅ | ✅ | ✅ | ✅ | ⬜ |
| **ForkID Bypass** | N/A | ✅ | ✅ | ✅ | ⬜ |
| **EIP-158 Disabled** | ✅ | ✅ | ✅ | ✅ | ⬜ |
| **Genesis Typo Support** | ✅ | ✅ | ✅ | ✅ | ⬜ |
| **V2 Auto-Detection** | ✅ | ✅ | ✅ | ✅ | ⬜ |
| **Dual P2P Sentry** | N/A | N/A | ✅ | N/A | ⬜ |
| **State Root Bypass** | N/A | ✅ | ✅ | ✅ | ⬜ |

---

## Total Estimated Hours

| Phase | Hours |
|-------|-------|
| Phase 1: Foundation | 64 |
| Phase 2: Core Consensus V1 | 60 |
| Phase 3: Reward Calculator | 48 |
| Phase 4: V2 BFT | 64 |
| Phase 5: Special TX Handling | 42 |
| Phase 6: State Root Compatibility | 56 |
| Phase 7: P2P Protocol | 52 |
| Phase 8: Sync Engine | 56 |
| Phase 9: Chain Specification | 18 |
| Phase 10: RPC Extensions | 40 |
| Phase 11: Docker & Deployment | 28 |
| Phase 12: Testing & Validation | 132 |
| **TOTAL** | **660** |

**Contingency (+20%)**: ~130 hours

**Grand Total**: **~800 engineer hours** (approximately 20 engineer-weeks)

---

## Quick Reference

### Key Block Numbers
- Genesis: 0
- First Checkpoint: 900
- First Reward: 1800
- TIPSigning Start: 3,000,000
- Mainnet V2 Switch: 56,857,600
- Apothem V2 Switch: 23,556,600

### Critical Config Values
- Epoch: 900 blocks
- Period: 2 seconds
- Gap: 450 blocks
- Block Reward: 250 XDC
- Chain ID Mainnet: 50
- Chain ID Apothem: 51

### System Contracts
- Validator: `0x0000000000000000000000000000000000000088`
- BlockSigners: `0x0000000000000000000000000000000000000089`
- Randomize: `0x0000000000000000000000000000000000000090`

### Important Files to Study
- GP5: `consensus/XDPoS/reward.go` (signing tx counting)
- GP5: `core/blockchain.go` (XdcStateRootCache)
- Erigon: `consensus/xdpos/xdpos.go` (V1/V2 routing)
- Nethermind: `Xdc/XdcStateRootCache.cs` (disk persistence)
- Nethermind: `Xdc/XdcBlockProcessor.cs` (header preservation)

---

*Document Version: 2.0*
*Last Updated: 2026-02-24*
*Status: Complete — Ready for Implementation*

This document supersedes `XDC-RETH-PORTING-PLAN.md` and `XDC-RETH-HOW-TO.md`.

For questions or clarifications, refer to the working implementations in:
- `github.com/AnilChinchawale/go-ethereum` (GP5 reference)
- `github.com/AnilChinchawale/erigon-xdc` (Erigon reference)
- `github.com/AnilChinchawale/nethermind` (Nethermind reference)
