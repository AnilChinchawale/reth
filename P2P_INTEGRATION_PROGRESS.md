# XDC P2P Integration Progress - Phase B

**Date:** February 24, 2026  
**Goal:** Integrate eth/63 P2P protocol support for XDC Network

## What We've Done

### 1. ✅ Added eth/63 Support to Reth's eth-wire-types

**File:** `crates/net/eth-wire-types/src/version.rs`

- Added `Eth63 = 63` variant to `EthVersion` enum
- Added `is_eth63()` helper method
- Added `has_request_ids()` method (returns `false` for eth/63)
- Updated all `TryFrom<&str>`, `TryFrom<u8>`, and `From<EthVersion>` implementations

**Why:** XDC uses eth/63 which doesn't have request IDs (unlike eth/66+).

### 2. ✅ Added StatusEth63 (No ForkID)

**File:** `crates/net/eth-wire-types/src/status.rs`

- Created `StatusEth63` struct without ForkID field
- Added `StatusMessage::Eth63` variant
- Updated `UnifiedStatus`:
  - `into_eth63()` - convert to eth/63 status
  - `into_message()` - chooses Eth63 variant when version is eth/63
  - `from_message()` - handles Eth63 variant
- Updated all `StatusMessage` methods to handle Eth63 case

**Why:** XDC's eth/63 doesn't include EIP-2124 ForkID in the Status message.

### 3. ✅ Created XDC Network Builder

**File:** `crates/xdc/node/src/network.rs`

**Features:**
- `XdcNetworkBuilder` - custom network builder for XDC
- Chain detection (`is_xdc_chain()`) for chain IDs 50 (mainnet) and 51 (Apothem)
- XDC mainnet and Apothem testnet bootnode lists
- `handshake::should_skip_forkid_validation()` - flag for skipping ForkID checks
- `handshake::protocol_version_for_chain()` - returns Eth63 for XDC chains

**Why:** XDC needs custom P2P networking:
- eth/63 protocol (no request IDs)
- Skip ForkID validation during handshake
- Use XDC-specific bootnodes

### 4. ✅ Integrated XdcNetworkBuilder into Node

**Files:**
- `crates/xdc/node/src/lib.rs` - Added network module, replaced EthereumNetworkBuilder with XdcNetworkBuilder
- `crates/xdc/node/src/network.rs` - Comprehensive network configuration

**Integration Points:**
- XDC node now uses `XdcNetworkBuilder` instead of `EthereumNetworkBuilder`
- Network builder detects XDC chain and configures appropriately
- Logs indicate when eth/63 mode is active

## What's Still Needed

### 1. 🔴 Modify Handshake Logic to Skip ForkID Validation

**Where:** `crates/net/eth-wire/src/handshake.rs`

The current handshake (`EthereumEthHandshake::eth_handshake`) always validates ForkID:

```rust
// Fork validation
if let Err(err) = fork_filter
    .validate(their_status_message.forkid())
    .map_err(EthHandshakeError::InvalidFork)
{
    unauth.disconnect(DisconnectReason::ProtocolBreach).await?;
    return Err(err.into());
}
```

**Solution:** Add a parameter or trait method to skip ForkID validation for XDC chains.

### 2. 🔴 Handle eth/63 Message Encoding/Decoding

**Where:** `crates/net/eth-wire-types/src/message.rs`

eth/63 messages don't have request IDs. Need to:
- Check if `EthMessage` encoding already handles this
- Update message decoders to work with eth/63 format
- Potentially use `crates/net/xdc-wire/` message types

### 3. 🔴 Add XDC Bootnodes Properly

**Where:** `crates/xdc/node/src/network.rs`

Currently we just log that we *should* add bootnodes. Need to:
- Parse enode URLs
- Add them to NetworkConfig before starting network

### 4. 🔴 Test P2P Connection

Once the above are done:
1. Get valid XDC mainnet bootnodes
2. Launch `xdc-reth node`
3. Check logs for peer connections
4. Verify handshake succeeds with XDC peers

## Architecture Decisions

### ✅ Modify eth-wire vs. Use xdc-wire Separately

**Decision:** Modify Reth's eth-wire-types minimally, keep xdc-wire for reference

**Rationale:**
- eth/63 is a legitimate Ethereum protocol version (just old)
- Adding Eth63 variant is clean and backward-compatible
- Reth's message handling can accommodate it with small changes
- Less invasive than hooking in a completely separate wire crate

### ✅ Network Builder Pattern

**Decision:** Custom `XdcNetworkBuilder` following BSC/OP Stack pattern

**Rationale:**
- Consistent with how other Reth extensions work (BSC, Gnosis)
- Clean separation of XDC-specific network logic
- Easy to maintain and test

## Next Steps

1. **Modify handshake.rs** to conditionally skip ForkID validation
2. **Test eth/63 message handling** (may already work if properly flagged)
3. **Add bootnode parsing** and injection
4. **Build and test** against XDC mainnet

## Build Status

```bash
cd /root/.openclaw/workspace/reth-xdc
. "$HOME/.cargo/env"
cargo build --release -p xdc-reth
```

Currently building to verify no compilation errors...

## References

- **Our xdc-wire crate:** `crates/net/xdc-wire/src/`
- **Reth network integration analysis:** `RETH-NETWORK-INTEGRATIONS-ANALYSIS.md`
- **XDC chain IDs:** 50 (mainnet), 51 (Apothem testnet)

## Commit

```bash
git add -A
git commit -m "feat(p2p): Wire eth/63 into Reth network layer for XDC

- Add Eth63 variant to EthVersion enum (crates/net/eth-wire-types)
- Add StatusEth63 message type without ForkID (eth/63 compatible)
- Create XdcNetworkBuilder with XDC chain detection and bootnodes
- Update XdcNode to use custom network builder
- Prepare handshake for ForkID validation skip (TODO)

This enables XDC Network to use eth/63 protocol which:
- Has no request IDs (unlike eth/66+)
- Has no ForkID in Status message (unlike eth/64+)
- Requires different handshake validation logic

Still TODO:
- Modify handshake.rs to skip ForkID validation for XDC
- Verify eth/63 message encoding/decoding
- Add bootnode parsing and injection
"
```

**Author:** anilcinchawale <anil24593@gmail.com>
