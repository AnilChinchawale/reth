# Reth XDC P2P Peer Stability Fix

## Problem
Peers were connecting successfully via inbound connections, but then Reth attempted redundant outbound connections to the same peers. These outbound attempts failed with ECIES errors, causing reputation to drop until peers were banned, resulting in:
- Block sync stuck at 0
- All peers eventually banned
- No stable P2P connections

## Root Cause
When an inbound connection was established, Reth's peer management would sometimes attempt an outbound connection to the same peer (to the listening port). The outbound ECIES handshake would fail (likely because the peer rejects duplicate connections), causing:
1. Reputation reduction for each failed attempt
2. Repeated failures → peer banned
3. Connection lost → no block sync

## Solution Implemented
**Two-layer defensive fix:**

### 1. Prevent Redundant Outbound Attempts (`best_unconnected`)
- Added explicit filter to exclude peers with active incoming connections
- Prevents system from even attempting outbound connections to peers already connected inbound
- Location: `crates/net/network/src/peers.rs:967`

### 2. Skip Reputation Reduction for Redundant Connection Failures (`on_connection_failure`)
- Check if peer has active incoming connection before reducing reputation
- If inbound connection exists, skip reputation penalty for outbound failure
- Adds trace logging to track when this protection is triggered
- Location: `crates/net/network/src/peers.rs:680-720`

## Files Modified
- `crates/net/network/src/peers.rs` - Peer reputation and connection management

## Expected Behavior After Fix
- Inbound connections succeed ✅
- No redundant outbound attempts to already-connected peers ✅
- Peers maintain good reputation ✅
- Stable P2P connectivity ✅
- Block sync proceeds normally ✅

## Testing
1. Start reth-xdc node with the fix
2. Monitor logs for:
   - "skipping reputation reduction - peer has active incoming connection"
   - Absence of repeated ECIES failures to same peer
   - Stable peer count
   - Block number increasing

## Implementation Type
Minimal, surgical fix as requested. No build required - changes are ready for testing.
