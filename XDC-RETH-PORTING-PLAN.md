# XDC Network Reth Porting Plan

## Executive Summary

This document outlines the comprehensive plan to port XDC Network support to Reth, a Rust-based Ethereum execution client. The XDC Network uses XDPoS (XDC Delegated Proof of Stake) consensus with two versions:
- **V1**: Epoch-based consensus with checkpoint rewards
- **V2**: BFT consensus with Quorum Certificates (QC) and Timeout Certificates (TC)

## Current State Analysis

### Reference Implementations

| Client | Language | XDPoS Status | Location |
|--------|----------|--------------|----------|
| XDC-Geth (GP5) | Go | Production | `eth/consensus/XDPoS/` |
| Erigon-XDC | Go | Production | `consensus/xdpos/` |
| Nethermind-XDC | C# | Production | `Nethermind.Xdc/` |
| Reth | Rust | **Target** | This project |

### Key XDPoS Differences from Ethereum

1. **Consensus**: XDPoS instead of Proof of Stake
2. **Block Time**: 2 seconds (vs Ethereum's 12 seconds)
3. **Epoch Length**: 900 blocks
4. **Gap Blocks**: 450 blocks before epoch switch
5. **No Uncles**: XDPoS doesn't allow uncle blocks
6. **Extra Data**: Contains validator list and signatures
7. **V2 Switch**: Mainnet switches to V2 at block 56,857,600

## Architecture Comparison

### Reth Consensus Trait
```rust
pub trait Consensus<B: Block>: HeaderValidator<B::Header> {
    fn validate_body_against_header(&self, body: &B::Body, header: &SealedHeader<B::Header>) -> Result<(), ConsensusError>;
    fn validate_block_pre_execution(&self, block: &SealedBlock<B>) -> Result<(), ConsensusError>;
    fn validate_block_post_execution(&self, block: &RecoveredBlock<N::Block>, result: &BlockExecutionResult<N::Receipt>, ...) -> Result<(), ConsensusError>;
}

pub trait HeaderValidator<H = Header>: Debug + Send + Sync {
    fn validate_header(&self, header: &SealedHeader<H>) -> Result<(), ConsensusError>;
    fn validate_header_against_parent(&self, header: &SealedHeader<H>, parent: &SealedHeader<H>) -> Result<(), ConsensusError>;
}
```

### XDPoS Engine Interface (Erigon Reference)
```go
type XDPoS struct {
    config *chain.XDPoSConfig
    recents *lru.Cache[common.Hash, *Snapshot]
    signatures *lru.Cache[common.Hash, accounts.Address]
    v2Engine *engine_v2.XDPoS_v2
}

func (c *XDPoS) VerifyHeader(chain rules.ChainHeaderReader, header *types.Header, seal bool) error
func (c *XDPoS) Prepare(chain rules.ChainHeaderReader, header *types.Header, state *state.IntraBlockState) error
func (c *XDPoS) Finalize(config *chain.Config, header *types.Header, ...)
func (c *XDPoS) Seal(chain rules.ChainHeaderReader, block *types.Block, ...)
```

## Implementation Phases

### Phase 1: Foundation (Weeks 1-2)

#### 1.1 Create XDPoS Consensus Crate

**Location**: `crates/consensus/xdpos/`

**Files to Create**:
```
crates/consensus/xdpos/
├── Cargo.toml
├── src/
│   ├── lib.rs              # Public exports
│   ├── xdpos.rs            # Main consensus engine
│   ├── config.rs           # XDPoS configuration
│   ├── snapshot.rs         # Voting snapshot management
│   ├── v1.rs               # XDPoS V1 validation
│   ├── v2/
│   │   ├── mod.rs          # V2 module
│   │   ├── engine.rs       # V2 BFT engine
│   │   ├── verification.rs # QC/TC verification
│   │   ├── types.rs        # V2 types (BlockInfo, QC, TC)
│   │   └── epoch_switch.rs # Epoch switch detection
│   ├── reward.rs           # Reward calculation
│   ├── validation.rs       # Header validation
│   └── errors.rs           # XDPoS-specific errors
```

#### 1.2 Core Data Structures

**XDPoS Config**:
```rust
pub struct XDPoSConfig {
    pub epoch: u64,                    // Epoch length (900)
    pub period: u64,                   // Block period in seconds (2)
    pub gap: u64,                      // Gap before epoch switch (450)
    pub reward: u64,                   // Block reward
    pub reward_checkpoint: u64,        // Reward checkpoint frequency
    pub foundation_wallet: Address,    // Foundation wallet address
    pub v2: Option<V2Config>,          // V2 configuration
}

pub struct V2Config {
    pub switch_block: u64,             // V2 activation block
    pub mine_period: u64,              // Mine period for V2 (2s)
    pub timeout_period: u64,           // Timeout period (10s)
    pub cert_threshold: u64,           // Certificate threshold (67%)
}
```

**V2 Types**:
```rust
pub type Round = u64;
pub type Signature = Vec<u8>;

pub struct BlockInfo {
    pub hash: B256,
    pub round: Round,
    pub number: u64,
}

pub struct QuorumCert {
    pub proposed_block_info: BlockInfo,
    pub signatures: Vec<Signature>,
    pub gap_number: u64,
}

pub struct TimeoutCert {
    pub round: Round,
    pub signatures: Vec<Signature>,
    pub gap_number: u64,
}

pub struct ExtraFieldsV2 {
    pub round: Round,
    pub quorum_cert: Option<QuorumCert>,
}
```

**Snapshot**:
```rust
pub struct Snapshot {
    pub number: u64,
    pub hash: B256,
    pub signers: HashSet<Address>,
    pub recents: HashMap<u64, Address>, // Recent signers for anti-spam
    pub votes: Vec<Vote>,
    pub tally: HashMap<Address, Tally>,
}

pub struct Vote {
    pub signer: Address,
    pub block: u64,
    pub address: Address, // Voted validator
    pub authorize: bool,  // Add or remove
}

pub struct Tally {
    pub authorize: bool,
    pub votes: usize,
}
```

### Phase 2: Chain Specification (Week 2)

#### 2.1 Create XDC Chain Specs

**Location**: `crates/chainspec/src/xdc/`

**Files**:
```
crates/chainspec/src/xdc/
├── mod.rs
├── mainnet.rs      # XDC Mainnet (chainId 50)
├── apothem.rs      # Apothem Testnet (chainId 51)
└── genesis/
    ├── mainnet.json
    └── apothem.json
```

**XDC Mainnet Configuration**:
```rust
pub static XDC_MAINNET: LazyLock<Arc<ChainSpec>> = LazyLock::new(|| {
    let genesis = serde_json::from_str(include_str!("../res/genesis/xdc_mainnet.json"))
        .expect("Can't deserialize XDC mainnet genesis");
    
    let hardforks = XdcHardfork::mainnet().into();
    
    ChainSpec {
        chain: Chain::from(50),  // XDC Mainnet
        genesis_header: SealedHeader::new(
            make_genesis_header(&genesis, &hardforks),
            XDC_MAINNET_GENESIS_HASH,
        ),
        genesis,
        hardforks,
        xdpos_config: Some(XDPoSConfig {
            epoch: 900,
            period: 2,
            gap: 450,
            reward: 250_000_000_000_000_000_000, // 250 XDC
            reward_checkpoint: 900,
            foundation_wallet: address!("0x7461c..."),
            v2: Some(V2Config {
                switch_block: 56_857_600,
                mine_period: 2,
                timeout_period: 10,
                cert_threshold: 67,
            }),
        }),
        bootnodes: xdc_mainnet_bootnodes(),
        ..Default::default()
    }.into()
});
```

**Genesis Hash**: `0x4a9d748bd78a8d0385b67788c2435dcdb914f98a96250b68863a1f8b7642d6b1`

### Phase 3: Consensus Implementation (Weeks 3-4)

#### 3.1 XDPoS Consensus Engine

```rust
pub struct XDPoSConsensus<ChainSpec> {
    chain_spec: Arc<ChainSpec>,
    config: XDPoSConfig,
    recents: LruCache<B256, Snapshot>,
    signatures: LruCache<B256, Address>,
    v2_engine: Option<XDPoSV2Engine>,
}

impl<ChainSpec: XdcChainSpec, N: NodePrimitives> FullConsensus<N> for XDPoSConsensus<ChainSpec> {
    fn validate_block_post_execution(
        &self,
        block: &RecoveredBlock<N::Block>,
        result: &BlockExecutionResult<N::Receipt>,
        receipt_root_bloom: Option<ReceiptRootBloom>,
    ) -> Result<(), ConsensusError> {
        // Apply rewards at checkpoint blocks
        self.apply_rewards(block)?;
        Ok(())
    }
}

impl<B: Block, ChainSpec: XdcChainSpec> Consensus<B> for XDPoSConsensus<ChainSpec> {
    fn validate_block_pre_execution(&self, block: &SealedBlock<B>) -> Result<(), ConsensusError> {
        let number = block.number();
        
        // Route to V1 or V2 based on block number
        if self.is_v2_block(number) {
            self.v2_engine.as_ref()
                .ok_or(ConsensusError::Custom("V2 engine not initialized".into()))?
                .validate_block_pre_execution(block)
        } else {
            self.validate_v1_block_pre_execution(block)
        }
    }
}
```

#### 3.2 V1 Validation

```rust
impl XDPoSConsensus {
    fn validate_v1_block_pre_execution(&self, block: &SealedBlock<B>) -> Result<(), ConsensusError> {
        let header = block.header();
        let number = header.number();
        
        // Check extra data length
        let extra = header.extra_data();
        if extra.len() < EXTRA_VANITY + EXTRA_SEAL {
            return Err(ConsensusError::ExtraDataTooShort);
        }
        
        // Verify checkpoint
        let checkpoint = number % self.config.epoch == 0;
        if checkpoint {
            // Verify masternode list in extra data
            self.verify_checkpoint_masternodes(header)?;
        }
        
        // Verify seal (signature)
        self.verify_seal(header)?;
        
        Ok(())
    }
}
```

#### 3.3 V2 Validation

```rust
impl XDPoSV2Engine {
    fn validate_v2_block(&self, block: &SealedBlock<B>) -> Result<(), ConsensusError> {
        let header = block.header();
        
        // Decode V2 extra fields
        let extra_fields = self.decode_extra_fields(header.extra_data())?;
        
        // Verify QC exists
        let qc = extra_fields.quorum_cert
            .ok_or(XDPoSError::MissingQC)?;
        
        // Verify QC signatures
        self.verify_qc(&qc, header)?;
        
        // Verify creator is in masternode list
        let creator = self.recover_creator(header)?;
        self.verify_creator_in_masternodes(&creator, header)?;
        
        Ok(())
    }
    
    fn verify_qc(&self, qc: &QuorumCert, parent: &Header) -> Result<(), ConsensusError> {
        // Get masternodes for this epoch
        let masternodes = self.get_masternodes(parent);
        
        // Check threshold (2/3 majority)
        let threshold = (masternodes.len() * 2) / 3;
        if qc.signatures.len() < threshold {
            return Err(XDPoSError::InsufficientSignatures);
        }
        
        // Verify each signature
        let sig_hash = vote_sig_hash(&VoteForSign {
            proposed_block_info: qc.proposed_block_info.clone(),
            gap_number: qc.gap_number,
        });
        
        for sig in &qc.signatures {
            self.verify_signature(sig_hash, sig, &masternodes)?;
        }
        
        Ok(())
    }
}
```

### Phase 4: P2P Protocol (Week 5)

#### 4.1 XDC P2P Messages

**Location**: `crates/net/xdc/`

```rust
// XDC BFT message types
pub const XDC_VOTE_MSG: u8 = 0xe0;
pub const XDC_TIMEOUT_MSG: u8 = 0xe1;
pub const XDC_SYNC_INFO_MSG: u8 = 0xe2;

pub struct VoteMessage {
    pub proposed_block_info: BlockInfo,
    pub signature: Signature,
    pub gap_number: u64,
}

pub struct TimeoutMessage {
    pub round: Round,
    pub signature: Signature,
    pub gap_number: u64,
}

pub struct SyncInfoMessage {
    pub highest_quorum_cert: QuorumCert,
    pub highest_timeout_cert: Option<TimeoutCert>,
}
```

#### 4.2 Protocol Handler

```rust
pub struct XdcProtocolHandler {
    consensus: Arc<XDPoSConsensus>,
    vote_pool: Arc<RwLock<VotePool>>,
    timeout_pool: Arc<RwLock<TimeoutPool>>,
}

impl ProtocolHandler for XdcProtocolHandler {
    fn on_message(&self, message: ProtocolMessage) -> Result<(), NetworkError> {
        match message.msg_type {
            XDC_VOTE_MSG => self.handle_vote(message)?,
            XDC_TIMEOUT_MSG => self.handle_timeout(message)?,
            XDC_SYNC_INFO_MSG => self.handle_sync_info(message)?,
            _ => return Err(NetworkError::UnknownMessageType),
        }
        Ok(())
    }
}
```

### Phase 5: State Management (Week 5)

#### 5.1 Validator Contract Integration

```rust
pub struct ValidatorManager {
    contract_address: Address, // 0x0000000000000000000000000000000000000088
}

impl ValidatorManager {
    pub fn get_masternodes(&self, state: &State, header: &Header) -> Vec<Address> {
        // Call the validator contract at 0x88
        let call_data = Self::masternode_list_selector();
        self.call_contract(state, header, call_data)
    }
    
    pub fn get_candidates(&self, state: &State, header: &Header) -> Vec<(Address, u64)> {
        // Get validator candidates
        let call_data = Self::candidate_list_selector();
        self.call_contract(state, header, call_data)
    }
}
```

#### 5.2 Snapshot Database

```rust
pub struct SnapshotDatabase {
    db: Arc<Database>,
}

impl SnapshotDatabase {
    pub fn load_snapshot(&self, hash: B256) -> Result<Option<Snapshot>, DatabaseError> {
        let key = snapshot_key(hash);
        self.db.get(&key)
    }
    
    pub fn save_snapshot(&self, snapshot: &Snapshot) -> Result<(), DatabaseError> {
        let key = snapshot_key(snapshot.hash);
        self.db.put(&key, snapshot)
    }
}
```

### Phase 6: RPC Extensions (Week 6)

```rust
pub struct XdcRpcModule {
    consensus: Arc<XDPoSConsensus>,
    chain_spec: Arc<ChainSpec>,
}

#[rpc(server)]
trait XdcApi {
    /// Get current masternode list
    #[method(name = "xdc_getMasternodes")]
    async fn get_masternodes(&self, block_number: Option<BlockNumber>) -> RpcResult<Vec<Address>>;
    
    /// Get current epoch information
    #[method(name = "xdc_getEpochInfo")]
    async fn get_epoch_info(&self) -> RpcResult<EpochInfo>;
    
    /// Get V2 round information
    #[method(name = "xdc_getRoundInfo")]
    async fn get_round_info(&self, block_number: Option<BlockNumber>) -> RpcResult<RoundInfo>;
    
    /// Get validator candidates
    #[method(name = "xdc_getCandidates")]
    async fn get_candidates(&self, block_number: Option<BlockNumber>) -> RpcResult<Vec<CandidateInfo>>;
    
    /// Get snapshot at block
    #[method(name = "xdc_getSnapshot")]
    async fn get_snapshot(&self, block_number: BlockNumber) -> RpcResult<SnapshotInfo>;
}
```

## Integration with Reth Node

### Node Builder Integration

```rust
pub struct XdcNode {
    consensus: Arc<XDPoSConsensus<XdcChainSpec>>,
    payload_builder: Arc<XdcPayloadBuilder>,
}

impl NodeBuilder for XdcNode {
    fn build(self) -> impl FullNode {
        // Initialize consensus
        let consensus = XDPoSConsensus::new(self.chain_spec.clone());
        
        // Add XDC-specific RPC methods
        let rpc_module = XdcRpcModule::new(consensus.clone(), self.chain_spec.clone());
        
        // Configure P2P for XDC messages
        let p2p = XdcP2PConfig::new(consensus.clone());
        
        FullNode::new()
            .with_consensus(consensus)
            .with_rpc(rpc_module)
            .with_p2p(p2p)
    }
}
```

## Testing Strategy

### Unit Tests
```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_v1_header_validation() {
        let consensus = XDPoSConsensus::new(xdc_testnet_config());
        let header = create_v1_test_header();
        assert!(consensus.validate_header(&header).is_ok());
    }
    
    #[test]
    fn test_v2_qc_verification() {
        let v2_engine = XDPoSV2Engine::new(v2_test_config());
        let qc = create_test_qc();
        let parent = create_test_parent();
        assert!(v2_engine.verify_qc(&qc, &parent).is_ok());
    }
    
    #[test]
    fn test_epoch_switch_detection() {
        assert!(is_epoch_switch(900, 900));  // First block of epoch
        assert!(!is_epoch_switch(901, 900)); // Not epoch switch
        assert!(is_epoch_switch(1800, 900)); // Next epoch
    }
}
```

### Integration Tests
```rust
#[tokio::test]
async fn test_sync_from_genesis() {
    let node = XdcNode::test_node(xdc_testnet_config()).await;
    
    // Sync first 1000 blocks
    for i in 0..1000 {
        let block = fetch_testnet_block(i).await;
        node.import_block(block).await.expect("Block import failed");
    }
}
```

## Files Modified in Reth

| File | Changes |
|------|---------|
| `crates/consensus/Cargo.toml` | Add xdpos member |
| `crates/chainspec/src/lib.rs` | Add XDC chain specs |
| `crates/chainspec/src/spec.rs` | Add XDC mainnet/apothem |
| `crates/node/builder/src/lib.rs` | Add XDC node support |
| `crates/net/network/src/protocol.rs` | Add XDC message types |
| `crates/rpc/rpc/src/lib.rs` | Add XDC RPC module |

## Dependencies

```toml
[dependencies]
# Existing Reth dependencies
reth-consensus = { workspace = true }
reth-chainspec = { workspace = true }
reth-primitives = { workspace = true }
reth-evm = { workspace = true }

# Additional for XDC
lru = "0.12"
sha3 = "0.10"
secp256k1 = { version = "0.28", features = ["recovery"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
```

## Build Commands

```bash
# Build XDC consensus crate
cargo build -p reth-consensus-xdpos

# Build with XDC support
cargo build --features xdc

# Run XDC tests
cargo test -p reth-consensus-xdpos

# Run XDC mainnet sync test
cargo test --test xdc_sync --features test-xdc -- --nocapture
```

## Timeline

| Phase | Duration | Deliverable |
|-------|----------|-------------|
| Phase 1 | Weeks 1-2 | XDPoS consensus crate, basic types |
| Phase 2 | Week 2 | Chain specs for mainnet/apothem |
| Phase 3 | Weeks 3-4 | V1/V2 validation implementation |
| Phase 4 | Week 5 | P2P protocol for BFT messages |
| Phase 5 | Week 5 | State management, snapshots |
| Phase 6 | Week 6 | RPC extensions, testing |
| Phase 7 | Week 7 | Integration, devnet testing |
| Phase 8 | Week 8 | Testnet/mainnet validation |

## Risk Assessment

| Risk | Impact | Mitigation |
|------|--------|------------|
| V2 QC verification complexity | High | Extensive testing with mainnet data |
| State root mismatch | High | Compare with geth/erigon at every checkpoint |
| P2P protocol changes | Medium | Follow existing XDC wire protocol |
| Reward calculation errors | Medium | Verify against mainnet reward distribution |
| Performance issues | Medium | Profile and optimize hot paths |

## Success Criteria

1. ✅ Can sync XDC mainnet from genesis
2. ✅ State roots match geth/erigon at checkpoints
3. ✅ Can validate V1 and V2 blocks correctly
4. ✅ Can process rewards at checkpoints
5. ✅ P2P connections to existing XDC nodes
6. ✅ Passes all existing Ethereum consensus tests (where applicable)

## Appendix

### XDC Mainnet Parameters
- **Chain ID**: 50
- **Genesis Hash**: `0x4a9d748bd78a8d0385b67788c2435dcdb914f98a96250b68863a1f8b7642d6b1`
- **V2 Switch Block**: 56,857,600
- **Epoch**: 900 blocks
- **Period**: 2 seconds
- **Gap**: 450 blocks

### Apothem Testnet Parameters
- **Chain ID**: 51
- **V2 Switch Block**: (testnet specific)
- **Genesis**: Different from mainnet

### Useful Links
- [XDC Network](https://xinfin.org)
- [XDPoS Documentation](https://docs.xdc.network)
- [Reth Documentation](https://reth.rs)
- [Erigon XDC Implementation](https://github.com/XDCFoundation/erigon-xdc)
