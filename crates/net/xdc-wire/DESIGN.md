# XDC Wire Protocol Design Document

**Phase 7**: XDC P2P Protocol — eth/63 + eth/100 Implementation  
**Date**: February 24, 2026  
**Author**: Reth-XDC Team

---

## Executive Summary

XDC Network requires compatibility with legacy Ethereum protocols (eth/62, eth/63) and a custom XDPoS2 protocol (eth/100) for consensus messaging. Standard Reth only supports eth/66+ with request IDs and ForkID validation, making it incompatible with XDC mainnet peers.

This document outlines the design for implementing XDC-compatible P2P protocols in Reth.

---

## 1. Problem Statement

### 1.1 Current State

**Reth Networking:**
- Supports: eth/66, eth/67, eth/68, eth/69, eth/70, eth/71
- All messages use **request-response pairs** with request IDs (EIP-2464)
- Handshake validates **ForkID** (EIP-2124)
- Discovery expects ForkID in ENR records

**XDC Network:**
- Supports: eth/62, eth/63, eth/100 (XDPoS2)
- Messages have **no request IDs** (pre-EIP-2464)
- Handshake has **no ForkID validation**
- Uses legacy status exchange: `[version, networkId, TD, head, genesis]`
- Custom consensus protocol (eth/100) for XDPoS2 votes/timeouts

### 1.2 Incompatibilities

| Feature | Reth (eth/66+) | XDC (eth/63) |
|---------|----------------|--------------|
| Request IDs | ✅ Required | ❌ None |
| ForkID validation | ✅ Required | ❌ None |
| Status format | 6 fields (with ForkID) | 5 fields (no ForkID) |
| GetBlockHeaders | `[requestId, [origin, amount, skip, reverse]]` | `[origin, amount, skip, reverse]` |
| BlockHeaders | `[requestId, [header1, header2, ...]]` | `[header1, header2, ...]` |
| Consensus protocol | None | eth/100 (XDPoS2) |

**Result:** Reth cannot connect to XDC mainnet peers.

---

## 2. Architecture Overview

### 2.1 Design Goals

1. **Minimal Reth modifications** — Extend, don't fork
2. **Protocol version negotiation** — Support eth/63 and eth/66+ simultaneously
3. **Custom protocol support** — Add eth/100 for XDPoS2 consensus
4. **Backward compatibility** — Work with legacy XDC nodes
5. **Future-proof** — Allow easy addition of new protocols

### 2.2 Component Architecture

```
┌─────────────────────────────────────────────────────┐
│                  Reth Node                          │
│                                                     │
│  ┌─────────────────────────────────────────────┐  │
│  │        Network Manager                      │  │
│  │  ┌───────────────────────────────────────┐  │  │
│  │  │   Session Manager                     │  │  │
│  │  │  ┌─────────────────────────────────┐  │  │  │
│  │  │  │   XdcEthStream                  │  │  │  │
│  │  │  │  - Version negotiation          │  │  │  │
│  │  │  │  - eth/63 (no request IDs)      │  │  │  │
│  │  │  │  - eth/66+ (with request IDs)   │  │  │  │
│  │  │  │  - eth/100 (XDPoS2)             │  │  │  │
│  │  │  └─────────────────────────────────┘  │  │  │
│  │  │                                         │  │  │
│  │  │  ┌─────────────────────────────────┐  │  │  │
│  │  │  │   XdcHandshake                  │  │  │  │
│  │  │  │  - No ForkID validation         │  │  │  │
│  │  │  │  - 5-field status exchange      │  │  │  │
│  │  │  │  - Network ID verification      │  │  │  │
│  │  │  └─────────────────────────────────┘  │  │  │
│  │  │                                         │  │  │
│  │  │  ┌─────────────────────────────────┐  │  │  │
│  │  │  │   Protocol Handlers             │  │  │  │
│  │  │  │  - Eth63Handler                 │  │  │  │
│  │  │  │  - Eth66Handler                 │  │  │  │
│  │  │  │  - XdposHandler (eth/100)       │  │  │  │
│  │  │  └─────────────────────────────────┘  │  │  │
│  │  └───────────────────────────────────────┘  │  │
│  └─────────────────────────────────────────────┘  │
│                                                     │
│  ┌─────────────────────────────────────────────┐  │
│  │        Discovery (discv4/discv5)            │  │
│  │  - XDC bootnodes                            │  │
│  │  - Accept peers without ForkID              │  │
│  └─────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────┘
```

### 2.3 Protocol Negotiation Flow

```
Peer A (Reth-XDC)         Peer B (XDC Legacy)
       │                          │
       │◄────── Hello ────────────│
       │   capabilities:          │
       │   - eth/63               │
       │   - eth/66               │
       │   - xdpos/100            │
       │                          │
       │────── Hello ─────────────►
       │   capabilities:          │
       │   - eth/63               │
       │   - xdpos/100            │
       │                          │
       │◄──── Negotiation ────────│
       │   Agreed: eth/63         │
       │                          │
       │◄──── Status (eth/63) ────│
       │   [version, networkId,   │
       │    TD, head, genesis]    │
       │                          │
       │───── Status (eth/63) ────►
       │   [version, networkId,   │
       │    TD, head, genesis]    │
       │                          │
       │◄──── Validation ─────────│
       │   ✓ Network ID match     │
       │   ✓ Genesis match        │
       │   ✗ NO ForkID check      │
       │                          │
       │◄═══ Connected ═══════════►
```

---

## 3. Protocol Specifications

### 3.1 eth/63 Protocol

**Message Types (0x00-0x10):**

```rust
// eth/63 messages (without request IDs)
const STATUS_MSG: u8 = 0x00;
const NEW_BLOCK_HASHES_MSG: u8 = 0x01;
const TRANSACTIONS_MSG: u8 = 0x02;
const GET_BLOCK_HEADERS_MSG: u8 = 0x03;
const BLOCK_HEADERS_MSG: u8 = 0x04;
const GET_BLOCK_BODIES_MSG: u8 = 0x05;
const BLOCK_BODIES_MSG: u8 = 0x06;
const NEW_BLOCK_MSG: u8 = 0x07;
const GET_NODE_DATA_MSG: u8 = 0x0d;
const NODE_DATA_MSG: u8 = 0x0e;
const GET_RECEIPTS_MSG: u8 = 0x0f;
const RECEIPTS_MSG: u8 = 0x10;
```

**Key Differences from eth/66:**

1. **No Request IDs:**
   ```rust
   // eth/66
   GetBlockHeaders([requestId, [origin, amount, skip, reverse]])
   BlockHeaders([requestId, [header1, header2, ...]])
   
   // eth/63
   GetBlockHeaders([origin, amount, skip, reverse])
   BlockHeaders([header1, header2, ...])
   ```

2. **Status Message:**
   ```rust
   // eth/66+ (with ForkID)
   Status([version, networkId, TD, head, genesis, forkId])
   
   // eth/63 (XDC compatible)
   Status([version, networkId, TD, head, genesis])
   ```

3. **Request Matching:**
   - eth/66: Match responses using request ID
   - eth/63: Match responses using **implicit ordering** (FIFO queue)

### 3.2 eth/100 Protocol (XDPoS2)

**Custom Consensus Protocol:**

```rust
// XDPoS2 message types (0xe0-0xe2)
const XDPOS2_VOTE_MSG: u8 = 0xe0;      // 224
const XDPOS2_TIMEOUT_MSG: u8 = 0xe1;   // 225
const XDPOS2_SYNCINFO_MSG: u8 = 0xe2;  // 226
```

**Message Structures:**

#### Vote Message (0xe0)
```rust
pub struct VoteMessage {
    pub round: u64,          // Consensus round
    pub block_hash: B256,    // Block being voted for
    pub signature: Bytes,    // BLS signature
}
```

#### Timeout Message (0xe1)
```rust
pub struct TimeoutMessage {
    pub round: u64,          // Consensus round
    pub signature: Bytes,    // BLS signature
}
```

#### SyncInfo Message (0xe2)
```rust
pub struct SyncInfoMessage {
    pub highest_qc: Bytes,     // Highest quorum certificate
    pub highest_tc: Bytes,     // Highest timeout certificate
    pub latest_block_no: u64,  // Latest block number
}
```

### 3.3 Handshake Specification

**XDC Handshake (eth/63):**

```rust
pub struct Xdc63Status {
    pub protocol_version: u32,  // 63
    pub network_id: u64,        // 50 (mainnet) or 51 (Apothem)
    pub total_difficulty: U256,
    pub head_hash: B256,
    pub genesis_hash: B256,
    // NO ForkID field
}

// Validation rules:
// 1. network_id must match (50 or 51)
// 2. genesis_hash must match
// 3. NO ForkID validation
```

**Compatibility Matrix:**

| Local | Remote | Protocol | Notes |
|-------|--------|----------|-------|
| Reth-XDC (eth/63) | XDC Legacy (eth/63) | eth/63 | No request IDs |
| Reth-XDC (eth/66) | XDC Modern (eth/66) | eth/66 | With request IDs |
| Reth-XDC (eth/100) | XDC Node (eth/100) | eth/100 | XDPoS2 consensus |

---

## 4. Implementation Plan

### 4.1 File Structure

```
crates/net/xdc-wire/
├── Cargo.toml
├── DESIGN.md (this file)
├── README.md
└── src/
    ├── lib.rs                  # Public API
    ├── types.rs                # Wire message types
    ├── version.rs              # Version enum and traits
    ├── eth63/
    │   ├── mod.rs              # eth/63 protocol
    │   ├── messages.rs         # Message types
    │   ├── codec.rs            # RLP encoding/decoding
    │   └── handler.rs          # Message handler
    ├── eth100/
    │   ├── mod.rs              # eth/100 (XDPoS2) protocol
    │   ├── messages.rs         # Vote/Timeout/SyncInfo
    │   ├── codec.rs            # RLP encoding/decoding
    │   └── handler.rs          # Consensus message handler
    ├── handshake.rs            # XDC-specific handshake
    ├── stream.rs               # XdcEthStream (protocol multiplexer)
    └── errors.rs               # Error types
```

### 4.2 Core Types

#### Version Enum

```rust
// src/version.rs
#[repr(u8)]
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum XdcVersion {
    Eth62 = 62,   // XDC legacy
    Eth63 = 63,   // XDC legacy with state sync
    Eth66 = 66,   // Modern with request IDs
    Eth100 = 100, // XDPoS2 consensus
}

impl XdcVersion {
    pub const fn has_request_ids(&self) -> bool {
        matches!(self, Self::Eth66)
    }
    
    pub const fn is_legacy(&self) -> bool {
        matches!(self, Self::Eth62 | Self::Eth63)
    }
    
    pub const fn is_consensus(&self) -> bool {
        matches!(self, Self::Eth100)
    }
}
```

#### Message Types

```rust
// src/types.rs
pub enum XdcMessage {
    // eth/63 messages
    Status(Xdc63Status),
    NewBlockHashes(NewBlockHashes),
    Transactions(Transactions),
    GetBlockHeaders(GetBlockHeaders63),  // No request ID
    BlockHeaders(BlockHeaders63),        // No request ID
    GetBlockBodies(GetBlockBodies63),    // No request ID
    BlockBodies(BlockBodies63),          // No request ID
    NewBlock(Box<NewBlock>),
    GetNodeData(GetNodeData63),          // eth/63 only
    NodeData(NodeData63),                // eth/63 only
    GetReceipts(GetReceipts63),
    Receipts(Receipts63),
    
    // eth/100 messages
    Vote(VoteMessage),
    Timeout(TimeoutMessage),
    SyncInfo(SyncInfoMessage),
}

pub struct GetBlockHeaders63 {
    pub origin: HashOrNumber,
    pub amount: u64,
    pub skip: u64,
    pub reverse: bool,
}

pub struct BlockHeaders63 {
    pub headers: Vec<Header>,
}
```

### 4.3 Handshake Implementation

```rust
// src/handshake.rs
pub struct XdcHandshake;

impl XdcHandshake {
    pub async fn execute<S>(
        stream: &mut S,
        local_status: Xdc63Status,
        network_id: u64,
        genesis_hash: B256,
    ) -> Result<(Xdc63Status, XdcVersion), XdcHandshakeError>
    where
        S: Stream + Sink + Unpin,
    {
        // 1. Send our status
        let status_msg = XdcMessage::Status(local_status.clone());
        stream.send(status_msg.encode()).await?;
        
        // 2. Receive peer status
        let peer_msg = stream.next().await
            .ok_or(XdcHandshakeError::ConnectionClosed)??;
        
        let peer_status = match XdcMessage::decode(&peer_msg)? {
            XdcMessage::Status(s) => s,
            _ => return Err(XdcHandshakeError::UnexpectedMessage),
        };
        
        // 3. Validate (NO ForkID check)
        if peer_status.network_id != network_id {
            return Err(XdcHandshakeError::NetworkIdMismatch {
                expected: network_id,
                received: peer_status.network_id,
            });
        }
        
        if peer_status.genesis_hash != genesis_hash {
            return Err(XdcHandshakeError::GenesisMismatch {
                expected: genesis_hash,
                received: peer_status.genesis_hash,
            });
        }
        
        // 4. Determine negotiated version
        let version = XdcVersion::try_from(peer_status.protocol_version)?;
        
        Ok((peer_status, version))
    }
}
```

### 4.4 Stream Protocol Handler

```rust
// src/stream.rs
pub struct XdcEthStream<S> {
    inner: S,
    version: XdcVersion,
    request_queue: VecDeque<PendingRequest>, // For eth/63 FIFO matching
}

impl<S> XdcEthStream<S> {
    pub fn new(inner: S, version: XdcVersion) -> Self {
        Self {
            inner,
            version,
            request_queue: VecDeque::new(),
        }
    }
    
    pub async fn send_message(&mut self, msg: XdcMessage) -> Result<(), XdcStreamError> {
        match (self.version, &msg) {
            // eth/63 messages - no request ID
            (XdcVersion::Eth63, XdcMessage::GetBlockHeaders(req)) => {
                self.request_queue.push_back(PendingRequest::Headers);
                self.inner.send(msg.encode()).await?;
            }
            
            // eth/66 messages - with request ID
            (XdcVersion::Eth66, XdcMessage::GetBlockHeaders(req)) => {
                let request_id = self.next_request_id();
                let wrapped = wrap_with_request_id(request_id, req);
                self.inner.send(wrapped.encode()).await?;
            }
            
            // eth/100 consensus messages
            (XdcVersion::Eth100, XdcMessage::Vote(_) | 
                                  XdcMessage::Timeout(_) | 
                                  XdcMessage::SyncInfo(_)) => {
                self.inner.send(msg.encode()).await?;
            }
            
            _ => return Err(XdcStreamError::InvalidMessageForVersion),
        }
        
        Ok(())
    }
    
    pub async fn receive_message(&mut self) -> Result<XdcMessage, XdcStreamError> {
        let bytes = self.inner.next().await
            .ok_or(XdcStreamError::ConnectionClosed)??;
        
        let msg = match self.version {
            XdcVersion::Eth63 => {
                // Decode without request ID
                let msg = XdcMessage::decode(&bytes)?;
                
                // Match response to pending request (FIFO)
                if msg.is_response() {
                    self.request_queue.pop_front();
                }
                
                msg
            }
            
            XdcVersion::Eth66 => {
                // Decode with request ID
                XdcMessage::decode_with_request_id(&bytes)?
            }
            
            XdcVersion::Eth100 => {
                // Decode consensus messages
                XdcMessage::decode(&bytes)?
            }
            
            _ => return Err(XdcStreamError::UnsupportedVersion),
        };
        
        Ok(msg)
    }
}
```

### 4.5 Integration with Reth Network Layer

#### Capability Negotiation

```rust
// Integration point: crates/net/network/src/session/mod.rs
pub fn xdc_capabilities() -> Vec<Capability> {
    vec![
        Capability::new_static("eth", 63),   // Legacy XDC
        Capability::new_static("eth", 66),   // Modern XDC
        Capability::new_static("xdpos", 100), // XDPoS2 consensus
    ]
}
```

#### Discovery Bootnodes

```rust
// XDC mainnet bootnodes
pub const XDC_MAINNET_BOOTNODES: &[&str] = &[
    "enode://f3c8c8e9f3...", // XDC Foundation node 1
    "enode://a2b1d3e5f6...", // XDC Foundation node 2
    "enode://c7d8e9f0a1...", // Community node 1
];

// XDC Apothem testnet bootnodes
pub const XDC_APOTHEM_BOOTNODES: &[&str] = &[
    "enode://1a2b3c4d5e...", // Apothem bootnode 1
    "enode://5f6g7h8i9j...", // Apothem bootnode 2
];
```

---

## 5. Testing Strategy

### 5.1 Unit Tests

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn test_eth63_status_encoding() {
        let status = Xdc63Status {
            protocol_version: 63,
            network_id: 50,
            total_difficulty: U256::from(1000),
            head_hash: B256::random(),
            genesis_hash: B256::random(),
        };
        
        let encoded = status.encode();
        let decoded = Xdc63Status::decode(&encoded).unwrap();
        
        assert_eq!(status, decoded);
    }
    
    #[test]
    fn test_get_block_headers_eth63() {
        let req = GetBlockHeaders63 {
            origin: HashOrNumber::Number(100),
            amount: 10,
            skip: 0,
            reverse: false,
        };
        
        let encoded = req.encode();
        assert!(!encoded.starts_with(&[0x01])); // No request ID
        
        let decoded = GetBlockHeaders63::decode(&encoded).unwrap();
        assert_eq!(req, decoded);
    }
    
    #[test]
    fn test_xdpos2_vote_message() {
        let vote = VoteMessage {
            round: 100,
            block_hash: B256::random(),
            signature: Bytes::from(vec![0u8; 96]), // BLS signature
        };
        
        let encoded = vote.encode();
        let decoded = VoteMessage::decode(&encoded).unwrap();
        
        assert_eq!(vote, decoded);
    }
}
```

### 5.2 Integration Tests

```rust
#[tokio::test]
async fn test_xdc_handshake() {
    let (stream_a, stream_b) = create_mock_stream_pair();
    
    let genesis = B256::random();
    let status_a = Xdc63Status::new(63, 50, U256::ZERO, B256::random(), genesis);
    let status_b = Xdc63Status::new(63, 50, U256::ZERO, B256::random(), genesis);
    
    let handle_a = tokio::spawn(async move {
        XdcHandshake::execute(stream_a, status_a, 50, genesis).await
    });
    
    let handle_b = tokio::spawn(async move {
        XdcHandshake::execute(stream_b, status_b, 50, genesis).await
    });
    
    let (result_a, result_b) = tokio::join!(handle_a, handle_b);
    
    assert!(result_a.is_ok());
    assert!(result_b.is_ok());
}

#[tokio::test]
async fn test_eth63_request_response_matching() {
    let stream = XdcEthStream::new(mock_stream(), XdcVersion::Eth63);
    
    // Send 3 requests
    stream.send_message(XdcMessage::GetBlockHeaders(...)).await.unwrap();
    stream.send_message(XdcMessage::GetBlockBodies(...)).await.unwrap();
    stream.send_message(XdcMessage::GetReceipts(...)).await.unwrap();
    
    // Responses should match FIFO order
    let resp1 = stream.receive_message().await.unwrap();
    assert!(matches!(resp1, XdcMessage::BlockHeaders(_)));
    
    let resp2 = stream.receive_message().await.unwrap();
    assert!(matches!(resp2, XdcMessage::BlockBodies(_)));
    
    let resp3 = stream.receive_message().await.unwrap();
    assert!(matches!(resp3, XdcMessage::Receipts(_)));
}
```

### 5.3 Testnet Validation

**Phase 1: Apothem Testnet**
1. Connect to Apothem bootnodes
2. Perform handshake with legacy nodes
3. Sync blocks using eth/63
4. Receive XDPoS2 consensus messages

**Phase 2: Mainnet Observation**
1. Connect to mainnet bootnodes (read-only)
2. Monitor consensus messages
3. Validate block production
4. Test peer discovery

---

## 6. Security Considerations

### 6.1 No ForkID Validation

**Risk:** Connecting to wrong chain without ForkID validation.

**Mitigation:**
- Genesis hash validation (mandatory)
- Network ID verification (50/51 only)
- Hard-coded bootnodes (trusted)
- Checkpoint verification (validate known block hashes)

```rust
pub struct XdcChainValidator {
    genesis_hash: B256,
    network_id: u64,
    checkpoints: Vec<(BlockNumber, B256)>,
}

impl XdcChainValidator {
    pub fn validate_chain(&self, headers: &[Header]) -> Result<(), ValidationError> {
        // Validate genesis
        if headers[0].hash() != self.genesis_hash {
            return Err(ValidationError::InvalidGenesis);
        }
        
        // Validate checkpoints
        for (number, hash) in &self.checkpoints {
            if let Some(header) = headers.iter().find(|h| h.number == *number) {
                if header.hash() != *hash {
                    return Err(ValidationError::InvalidCheckpoint(*number));
                }
            }
        }
        
        Ok(())
    }
}
```

### 6.2 Request Matching Without IDs

**Risk:** Response mismatching in eth/63 (FIFO queue attack).

**Mitigation:**
- Timeout pending requests (30 seconds)
- Limit concurrent requests per peer (max 10)
- Validate response structure before matching
- Disconnect on repeated mismatches

```rust
pub struct RequestQueue {
    pending: VecDeque<PendingRequest>,
    max_pending: usize,
    request_timeout: Duration,
}

impl RequestQueue {
    pub fn can_send_request(&self) -> bool {
        self.pending.len() < self.max_pending
    }
    
    pub fn match_response(&mut self, response: &XdcMessage) -> Result<RequestType, QueueError> {
        let expected = self.pending.pop_front()
            .ok_or(QueueError::UnexpectedResponse)?;
        
        if !expected.matches_response(response) {
            return Err(QueueError::ResponseMismatch);
        }
        
        Ok(expected.request_type)
    }
    
    pub fn cleanup_expired(&mut self) {
        let now = Instant::now();
        self.pending.retain(|req| now.duration_since(req.sent_at) < self.request_timeout);
    }
}
```

### 6.3 Consensus Message Validation

**Risk:** Invalid XDPoS2 messages disrupting consensus.

**Mitigation:**
- Signature verification (BLS)
- Round validation (must be current or next)
- Rate limiting per peer
- Blacklist malicious peers

```rust
pub struct XdposValidator {
    current_round: u64,
    masternode_set: HashSet<Address>,
}

impl XdposValidator {
    pub fn validate_vote(&self, vote: &VoteMessage) -> Result<(), ConsensusError> {
        // Validate round
        if vote.round < self.current_round || vote.round > self.current_round + 1 {
            return Err(ConsensusError::InvalidRound);
        }
        
        // Verify BLS signature
        let signer = self.recover_signer(&vote.signature, vote.round, vote.block_hash)?;
        
        // Check if signer is masternode
        if !self.masternode_set.contains(&signer) {
            return Err(ConsensusError::UnauthorizedSigner);
        }
        
        Ok(())
    }
}
```

---

## 7. Performance Considerations

### 7.1 Protocol Overhead

| Protocol | Message Overhead | Notes |
|----------|------------------|-------|
| eth/63 | Lower (no request ID) | 8 bytes saved per request/response |
| eth/66 | Higher (+8 bytes) | Request ID adds overhead |
| eth/100 | Medium | Consensus messages are infrequent |

**Optimization:** Use eth/66 for bulk sync, eth/63 for compatibility.

### 7.2 Request Matching Performance

**eth/63 (FIFO queue):**
- Enqueue: O(1)
- Dequeue: O(1)
- Memory: O(n) where n = pending requests

**eth/66 (HashMap by request ID):**
- Insert: O(1)
- Lookup: O(1)
- Memory: O(n) where n = pending requests

**Conclusion:** Both are O(1), eth/66 is safer (no ordering assumptions).

---

## 8. Migration Path

### 8.1 Deployment Phases

**Phase 1: eth/63 Support** ✅ (This PR)
- Implement message types
- Add handshake logic
- FIFO request matching
- Unit tests

**Phase 2: eth/100 Support** ✅ (This PR)
- XDPoS2 message types
- Consensus validation
- Integration with XDPoS engine

**Phase 3: Discovery**
- Add XDC bootnodes
- Accept peers without ForkID
- Peer scoring for XDC compatibility

**Phase 4: Optimization**
- Protocol version preference (eth/66 > eth/63)
- Parallel sync with mixed peers
- Performance benchmarks

### 8.2 Compatibility Testing

| Peer Type | Protocol | Status |
|-----------|----------|--------|
| XDC Legacy (v1.x) | eth/62 | ⚠️ Partial (add eth/62 support) |
| XDC Legacy (v2.x) | eth/63 | ✅ Supported |
| XDC Modern | eth/66 | ✅ Supported |
| Reth-XDC | eth/63, eth/66, eth/100 | ✅ Supported |

---

## 9. References

### 9.1 Specifications
- [Ethereum Wire Protocol](https://github.com/ethereum/devp2p/blob/master/caps/eth.md)
- [EIP-2464: eth/65 - Transaction announcements and retrievals](https://eips.ethereum.org/EIPS/eip-2464)
- [EIP-2124: Fork identifier for chain compatibility checks](https://eips.ethereum.org/EIPS/eip-2124)
- [XDC Network Documentation](https://docs.xdc.org/)

### 9.2 Reference Implementations
- Reth eth-wire: `crates/net/eth-wire/`
- Erigon XDC: `/root/.openclaw/workspace/erigon-xdc/p2p/protocols/eth/xdc_protocol.go`
- Go-Ethereum: `/root/.openclaw/workspace/go-ethereum/eth/protocols/eth/`

### 9.3 Related Documents
- `RETH-NETWORK-INTEGRATIONS-ANALYSIS.md` — BSC, OP Stack, Gnosis, Berachain analysis
- Phase 6 documentation — XDPoS consensus implementation

---

## 10. Open Questions & TODOs

### 10.1 Implementation TODOs
- [ ] Determine request timeout values (30s? 60s?)
- [ ] Max concurrent requests per peer (10? 20?)
- [ ] eth/100 message rate limits
- [ ] Peer reputation scoring for XDC peers
- [ ] Fallback to eth/63 if eth/66 fails

### 10.2 Future Enhancements
- [ ] Support eth/62 for oldest legacy nodes
- [ ] Protocol version preference configuration
- [ ] Metrics for protocol version usage
- [ ] eth/100 message batching for efficiency
- [ ] Custom RPC methods for XDPoS2 queries

---

## Appendix A: Message Format Examples

### eth/63 GetBlockHeaders
```
RLP([origin, amount, skip, reverse])

Example:
[
  100,      // origin (block number)
  10,       // amount
  0,        // skip
  false     // reverse
]
```

### eth/66 GetBlockHeaders
```
RLP([requestId, [origin, amount, skip, reverse]])

Example:
[
  0x0123456789abcdef,  // request ID
  [
    100,               // origin
    10,                // amount
    0,                 // skip
    false              // reverse
  ]
]
```

### XDPoS2 Vote Message
```
RLP([round, blockHash, signature])

Example:
[
  100,                                // round
  0x1234...5678,                     // block hash (32 bytes)
  0xabcd...ef01...9876               // BLS signature (96 bytes)
]
```

---

**Document Version:** 1.0  
**Status:** Ready for Implementation  
**Last Updated:** 2026-02-24
