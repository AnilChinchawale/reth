# Reth XDC Sync Status - Feb 27, 2026

## Current Status
**Block: 0 (not syncing)**
**Peers: 0 (disconnecting after ~50 seconds)**

## Root Causes Identified

### 1. ✅ FIXED: XDC Header Hash Mismatch
**Problem**: Headers were decoded with empty XDC fields (validators/validator/penalties), causing incorrect hash computation.

**Solution Implemented**:
- Changed `BlockHeaders63` in `xdc-wire/types.rs` to use `Vec<XdcBlockHeader>` (18 fields) instead of `Vec<Header>` (15 fields)
- Added `reth-xdc-primitives` dependency to `xdc-wire`
- Now headers preserve all 18 fields during P2P decode
- Hash computation via `XdcBlockHeader::hash_slow()` includes all fields

**Evidence**: Logs show `validator_len=65` (actual data preserved, not empty)

### 2. ✅ PARTIALLY FIXED: P2P Stability - Trusted Peer Banning
**Problem**: Peers getting banned due to connection failures, even when marked as trusted/static.

**Solution Implemented**:
- Modified `ban_peer()` in `peers.rs` to completely skip banning for trusted/static peers
- Added `FailedToConnect` to reputation change exemptions for trusted/static peers

**Remaining Issue**: Peers still disconnecting after ~50s

### 3. ❌ NOT FIXED: Duplicate Outbound Connection Attempts
**Problem**: 
- INBOUND connections from PROD GP5 work perfectly (XDC handshake succeeds)
- Reth immediately makes OUTBOUND connection attempts to the same peer
- OUTBOUND ECIES handshakes fail with `UnreadableStream` error
- Multiple rapid failures occur within seconds
- Peer gets dropped/disconnected

**Evidence from Logs**:
```
05:46:06 - XDC handshake successful (INBOUND)
05:46:54 - Multiple ECIES failures on OUTBOUND attempts
05:46:54 - add_and_connect_kind called ... kind=Static
05:46:54 - ECIES handshake failed ... UnreadableStream (3x within 1 second)
```

**Why This Happens**:
1. PROD GP5 connects to Reth on port 40303 (INBOUND) ✅
2. Reth successfully completes ECIES + P2P Hello + XDC Status ✅
3. Peer is marked as Static ✅
4. Reth's swarm manager tries to make OUTBOUND connections to the peer ❌
5. OUTBOUND ECIES fails because peer is already connected ❌
6. Reputation drops with each failure ❌
7. After multiple failures, peer disconnects ❌

**Root Cause**: Reth doesn't detect that an INBOUND connection already exists for the same peer before attempting OUTBOUND connections.

## What Works
- ✅ INBOUND P2P connections (ECIES, P2P Hello, XDC Status handshake)
- ✅ XDC header decoding with preserved fields (18 fields including validators)
- ✅ FCU (ForkchoiceUpdated) accepted in SYNCING state
- ✅ Peer marked as Static correctly

## What Doesn't Work
- ❌ Peer stays connected (disconnects after ~50s)
- ❌ Duplicate outbound connection prevention
- ❌ Block sync progression (block stays at 0)

## Next Steps

### Option 1: Prevent Duplicate Outbound Connections (Recommended)
Modify the swarm manager to check if an active session already exists before initiating outbound connections.

**Files to modify**:
- `crates/net/network/src/manager.rs` - swarm connection logic
- `crates/net/network/src/peers.rs` - add check in `dial_outbound` or similar

### Option 2: Make OUTBOUND ECIES Errors Non-Fatal for Static Peers
Treat outbound connection failures to already-connected static peers as warnings, not reputation penalties.

### Option 3: Disable Outbound Connections Entirely (Temporary Workaround)
Force Reth to only accept inbound connections, never initiate outbound.

## Files Modified
1. `crates/xdc/primitives/src/header.rs` - XdcBlockHeader with 18-field hash
2. `crates/net/xdc-wire/src/types.rs` - BlockHeaders63 uses XdcBlockHeader
3. `crates/net/xdc-wire/Cargo.toml` - Added reth-xdc-primitives dependency
4. `crates/net/network/src/peers.rs` - Skip banning trusted/static peers
5. `crates/net/eth-wire-types/src/message.rs` - eth/63 header decode path (now unused due to xdc-wire)

## Testing Commands
```bash
# Deploy
docker stop xdc-node-reth; docker rm -f xdc-node-reth
cp target/release/xdc-reth /root/xdc-node-setup/docker/
cd /root/xdc-node-setup/docker && docker build -f Dockerfile.reth -t anilchinchawale/rethx:latest .

# Run
docker run -d --name xdc-node-reth --network host \
    -v /root/xdc-node-setup/mainnet/reth-db:/work/xdcchain \
    anilchinchawale/rethx:latest \
    xdc-reth node --datadir /work/xdcchain --chain xdc-mainnet \
    --http --http.addr 0.0.0.0 --http.port 7073 \
    --authrpc.addr 0.0.0.0 --authrpc.port 8551 \
    --authrpc.jwtsecret /work/xdcchain/jwt.hex \
    --port 40303 --nat extip:95.217.56.168 \
    --debug.rpc-consensus-ws ws://127.0.0.1:8559

# Add peer from PROD (INBOUND to Reth)
RETH_PUBKEY=$(curl -s -X POST -d '{"jsonrpc":"2.0","method":"admin_nodeInfo","params":[],"id":1}' http://127.0.0.1:7073 | jq -r '.result.enode' | sed 's/enode:\/\///' | sed 's/@.*//')
ssh root@65.21.27.213 -p 12141 "curl -s -X POST -d '{\"method\":\"admin_addPeer\",\"params\":[\"enode://${RETH_PUBKEY}@95.217.56.168:40303\"],\"id\":1}' http://127.0.0.1:8545"

# Monitor
docker logs -f xdc-node-reth 2>&1 | grep -iE "handshake|ECIES|XDC-"
```

## Key Insights
1. **INBOUND-only strategy works** - XDC handshake succeeds when PROD connects to Reth
2. **OUTBOUND attempts are the problem** - Always fail with ECIES UnreadableStream
3. **Headers are correctly preserved** - validator_len=65 confirms XDC fields intact
4. **Ban protection isn't enough** - Peer disconnects even without explicit ban

## Recommendation
**Focus on preventing duplicate outbound connection attempts**. The peer is already connected inbound, so outbound attempts are unnecessary and harmful. This is the simplest fix with the highest chance of success.
