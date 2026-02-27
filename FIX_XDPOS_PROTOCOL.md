# Fix: xdpos/100 Protocol Issues

## Issue Summary

1. **Wrong message IDs** (0xe0, 0xe1, 0xe2 instead of 0, 1, 2)
2. **Wrong message count** (22 instead of 3)
3. **No satellite handler** installed for xdpos/100

## Fix 1: Correct xdpos/100 Message IDs

### File: `crates/net/xdc-wire/src/eth100/mod.rs`

```rust
// BEFORE (WRONG):
pub const VOTE_MSG: u8 = 0xe0;
pub const TIMEOUT_MSG: u8 = 0xe1;
pub const SYNCINFO_MSG: u8 = 0xe2;

// AFTER (CORRECT):
pub const VOTE_MSG: u8 = 0x00;
pub const TIMEOUT_MSG: u8 = 0x01;
pub const SYNCINFO_MSG: u8 = 0x02;
```

### File: `crates/net/xdc-wire/src/types.rs`

```rust
// BEFORE (WRONG):
pub enum XdcMessageID {
    // ...
    Vote = 0xe0,
    Timeout = 0xe1,
    SyncInfo = 0xe2,
}

// AFTER (CORRECT):
pub enum XdcMessageID {
    // eth/63 messages stay the same (0x00-0x10)
    // ...
    // xdpos/100 messages use small IDs within xdpos protocol:
    Vote = 0x00,      // First xdpos message
    Timeout = 0x01,   // Second xdpos message  
    SyncInfo = 0x02,  // Third xdpos message
}
```

**Note**: This creates a conflict with Status=0x00 in the same enum. The solution is to either:
- Use separate enums for eth/63 and xdpos/100 messages
- Or interpret IDs contextually based on which protocol is being used

## Fix 2: Correct xdpos/100 Message Count

### File: `crates/net/network/src/config.rs`

```rust
// BEFORE:
Protocol::new(Capability::new_static("xdpos", 100), 22),

// AFTER:
Protocol::new(Capability::new_static("xdpos", 100), 3),  // Only 3 messages: Vote, Timeout, SyncInfo
```

Change in TWO places (lines ~659 and ~679).

## Fix 3: Implement xdpos/100 Satellite Handler (Optional)

If you want to actually handle xdpos/100 messages instead of dropping them:

### Create handler: `crates/net/network/src/xdpos_handler.rs`

```rust
use reth_eth_wire::{
    capability::UnsupportedCapabilityError,
    multiplex::ProtocolConnection,
    protocol::{IntoRlpxSubProtocol, OnNotSupported, RlpxSubProtocol, RlpxSubProtocolHandler},
    Capability, Direction, SharedCapabilities,
};
use reth_network_peers::PeerId;
use futures::Stream;
use bytes::BytesMut;

/// XDPoS2 consensus protocol capability
pub const XDPOS_CAPABILITY: Capability = Capability::new_static("xdpos", 100);

pub struct XdposProtocol;

impl IntoRlpxSubProtocol for XdposProtocol {
    fn into_rlpx_sub_protocol(self) -> RlpxSubProtocol {
        RlpxSubProtocol {
            cap: XDPOS_CAPABILITY.clone(),
            handler: Box::new(XdposHandler),
        }
    }
}

struct XdposHandler;

impl RlpxSubProtocolHandler for XdposHandler {
    fn protocol(&self) -> &Capability {
        &XDPOS_CAPABILITY
    }
    
    fn on_unsupported_by_peer(
        &self,
        _caps: &SharedCapabilities,
        _direction: Direction,
        _peer_id: PeerId,
    ) -> OnNotSupported {
        // xdpos is optional - don't disconnect if peer doesn't support it
        OnNotSupported::Continue
    }
    
    fn into_connection(
        self: Box<Self>,
        _direction: Direction,
        _peer_id: PeerId,
        conn: ProtocolConnection,
    ) -> Box<dyn Stream<Item = BytesMut> + Send + Unpin> {
        // Process xdpos messages here
        Box::new(XdposConnection { conn })
    }
}

struct XdposConnection {
    conn: ProtocolConnection,
}

impl Stream for XdposConnection {
    type Item = BytesMut;
    
    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        use futures::StreamExt;
        match self.conn.poll_next_unpin(cx) {
            std::task::Poll::Ready(Some(msg)) => {
                // Handle xdpos message
                let msg_id = msg.first().copied().unwrap_or(0);
                match msg_id {
                    0 => tracing::debug!("Received xdpos Vote"),
                    1 => tracing::debug!("Received xdpos Timeout"),
                    2 => tracing::debug!("Received xdpos SyncInfo"),
                    _ => tracing::warn!("Unknown xdpos message ID: {}", msg_id),
                }
                // For now, don't produce any outgoing messages
                std::task::Poll::Pending
            }
            std::task::Poll::Ready(None) => std::task::Poll::Ready(None),
            std::task::Poll::Pending => std::task::Poll::Pending,
        }
    }
}
```

### Register the handler in node startup

```rust
// In your node initialization code:
network_config.add_rlpx_sub_protocol(XdposProtocol);
```

## Verification

After fixing, verify that:
1. Reth-XDC can connect to GP5 without instant disconnect
2. eth/63 Status handshake completes successfully
3. xdpos/100 messages are either handled or logged (not silently dropped)

## Debug Steps

If disconnect still occurs, check logs for:
- "No shared eth capability" → GP5 isn't advertising eth/63
- "decode error in XDC handshake" → Status format mismatch
- "MismatchedGenesis" → Wrong genesis hash
- "MismatchedChain" → Wrong network ID (should be 50 or 51)
