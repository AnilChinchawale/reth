# reth-xdc-wire

XDC Network wire protocol implementation for Reth.

## Overview

This crate provides wire protocol support for XDC Network, including:

- **eth/63**: Legacy Ethereum protocol without request IDs (XDC compatible)
- **eth/100**: XDPoS2 consensus protocol for validator messages

## Features

### eth/63 Protocol

XDC mainnet uses eth/63, which predates EIP-2464 (request IDs). Key differences:

- No request IDs in messages
- No ForkID validation in handshake
- Implicit FIFO request-response matching

```rust
use reth_xdc_wire::{XdcVersion, XdcMessage, GetBlockHeaders63};

let version = XdcVersion::Eth63;
assert!(!version.has_request_ids());

let request = GetBlockHeaders63 {
    origin: HashOrNumber::Number(100),
    amount: 10,
    skip: 0,
    reverse: false,
};
```

### eth/100 Protocol (XDPoS2)

XDC's custom consensus protocol for validator coordination:

```rust
use reth_xdc_wire::{VoteMessage, XdcMessage};

let vote = VoteMessage {
    round: 100,
    block_hash: block_hash,
    signature: bls_signature,
};

let msg = XdcMessage::Vote(vote);
```

## Handshake

XDC handshake does not include ForkID validation:

```rust
use reth_xdc_wire::{Xdc63Status, XdcHandshake};

let status = Xdc63Status {
    protocol_version: 63,
    network_id: 50,  // XDC mainnet
    total_difficulty: td,
    head_hash: head,
    genesis_hash: genesis,
};

let (peer_status, version) = XdcHandshake::execute(
    &mut stream,
    status,
    50,
    genesis,
).await?;
```

## Protocol Negotiation

Reth-XDC advertises multiple protocol versions:

```rust
use reth_xdc_wire::xdc_capabilities;

let capabilities = xdc_capabilities();
// Returns: [eth/63, eth/66, xdpos/100]
```

## Architecture

```
XdcEthStream
├── Eth63Handler    (legacy messages, no request IDs)
├── Eth66Handler    (modern messages, with request IDs)
└── XdposHandler    (consensus messages)
```

## Testing

Run tests:

```bash
cargo test -p reth-xdc-wire
```

Integration tests require mock streams or testnet access.

## See Also

- [DESIGN.md](DESIGN.md) — Full architecture and protocol specification
- [Phase 7 Documentation](../../docs/phase-7-p2p.md)
- [Ethereum Wire Protocol](https://github.com/ethereum/devp2p/blob/master/caps/eth.md)

## License

Licensed under either of:

- Apache License, Version 2.0, ([LICENSE-APACHE](../../../LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](../../../LICENSE-MIT) or http://opensource.org/licenses/MIT)
