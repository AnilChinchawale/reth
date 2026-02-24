# XDC Sync Engine Design

**Phase 8: Sync Engine Integration**  
**Author:** anilcinchawale <anil24593@gmail.com>  
**Date:** February 24, 2026

## Overview

This document describes the sync engine adaptations required for XDC Network in Reth. XDC can **only perform full sync** (no snap sync) because v2.6.8 peers don't support the snap protocol. The sync engine must handle XDC-specific consensus requirements during block execution.

## Sync Mode Limitations

### Why Full Sync Only?

- **Legacy Protocol:** XDC v2.6.8 peers only support eth/66 (headers, bodies, receipts)
- **No Snap Protocol:** eth/67+ snap protocol is not implemented in XDC nodes
- **No Beacon Sync:** Pre-merge consensus (XDPoS), no beacon chain

### Sync Flow

```
Download Headers → Download Bodies → Execute Blocks → Verify State
      ↓                   ↓                  ↓              ↓
  HeaderStage         BodyStage      ExecutionStage   MerkleStage
```

## XDC-Specific Execution Requirements

### 1. State Root Cache Integration

**Problem:** At checkpoint blocks (every 900), reward distribution causes state root divergence between clients due to execution order and EIP differences.

**Solution:** Integrate `XdcStateRootCache` (Phase 6) during block execution:

```rust
// During block execution
if is_checkpoint_block(block.number) {
    // Execute block normally
    let computed_state_root = execute_block(...);
    
    // Check if state root matches header
    if computed_state_root != header.state_root {
        // Check cache for known divergence
        if let Some(local_root) = cache.get_local_root(header.state_root) {
            // Known divergence - use cached local root
            state_root = local_root;
        } else {
            // New checkpoint - store mapping for future
            cache.store_mapping(block.number, header.state_root, computed_state_root);
            state_root = computed_state_root;
        }
    }
}
```

**Integration Points:**
- `ExecutionStage` - after block execution, before state root verification
- `XdcBlockExecutor` - custom executor wrapper with cache access

### 2. Reward Application at Checkpoints

**Problem:** Rewards must be applied BEFORE computing the state root at checkpoint blocks.

**Solution:** Hook into execution stage to apply rewards:

```rust
impl XdcBlockExecutor {
    fn execute_block(&mut self, block: Block) -> Result<ExecutionOutcome> {
        // 1. Execute all transactions in block
        let mut outcome = self.base_executor.execute_block(block)?;
        
        // 2. If checkpoint block, apply rewards BEFORE state root
        if block.number % self.config.epoch == 0 && block.number > 0 {
            let rewards = self.reward_calculator.calculate_rewards(
                block.number,
                &self.state_provider,
            )?;
            
            // Apply rewards to state
            for (address, reward) in rewards {
                outcome.state.increment_balance(address, reward)?;
            }
        }
        
        // 3. Compute state root (includes rewards)
        outcome.state_root = outcome.state.root()?;
        
        Ok(outcome)
    }
}
```

**Flow:**
```
Block Execution
    ↓
Transaction Execution (all txs)
    ↓
Checkpoint Check (N % 900 == 0)
    ↓
Walk Previous Epoch (N-899 to N-1)
    ↓
Count Signatures per Validator
    ↓
Calculate Rewards (250 XDC per epoch distributed)
    ↓
Apply Rewards to State (90% masternode, 10% foundation)
    ↓
Compute State Root (includes reward state changes)
```

### 3. Special Transaction Handling

**Problem:** After block 3M (TIPSigning), transactions to 0x89/0x90 are free (no gas cost).

**Solution:** Integrate `special_tx.rs` during transaction execution:

```rust
fn execute_transaction(&mut self, tx: Transaction) -> Result<Receipt> {
    // Check if this is a free gas transaction
    let is_free = is_free_gas_tx(self.block_number, tx.to);
    
    if is_free {
        // Execute with zero gas cost
        self.execute_system_tx(tx)
    } else {
        // Normal gas execution
        self.execute_normal_tx(tx)
    }
}
```

**Affected Blocks:**
- Block >= 3,000,000 (mainnet)
- Transactions to: 0x89 (BlockSigners), 0x90 (Randomize)

### 4. V1/V2 Consensus Switch

**Problem:** Consensus rules change at V2 switch block (~62M mainnet, 23.5M Apothem).

**Solution:** Check consensus version before validation:

```rust
impl XdcBlockExecutor {
    fn validate_block(&self, header: &Header) -> Result<()> {
        if self.config.is_v2(header.number) {
            // Use V2 validation (QC/TC in extra data)
            self.v2_validator.validate(header)
        } else {
            // Use V1 validation (epoch-based PoA)
            self.v1_validator.validate(header)
        }
    }
}
```

**Version Boundaries:**
- **Mainnet:** V1: 0 → 56,857,599, V2: 56,857,600+
- **Apothem:** V1: 0 → 23,556,599, V2: 23,556,600+

## Architecture

### Module Structure

```
crates/consensus/xdpos/src/
├── sync.rs              ← New: Sync coordinator
├── execution.rs         ← New: Block execution hooks
├── xdpos.rs            (existing: consensus engine)
├── reward.rs           (existing: Phase 3)
├── special_tx.rs       (existing: Phase 5)
├── state_root_cache.rs (existing: Phase 6)
└── v1.rs, v2/          (existing: consensus validation)
```

### Component Diagram

```
┌─────────────────────────────────────────────────┐
│            Reth Sync Pipeline                   │
│  HeaderStage → BodyStage → ExecutionStage       │
└─────────────────┬───────────────────────────────┘
                  │
                  ↓
         ┌────────────────────┐
         │  XdcBlockExecutor  │ ← sync.rs
         └────────┬───────────┘
                  │
      ┌───────────┼───────────┬──────────┐
      │           │           │          │
      ↓           ↓           ↓          ↓
┌──────────┐ ┌─────────┐ ┌────────┐ ┌──────────┐
│ V1/V2    │ │ Reward  │ │Special │ │StateRoot │
│Validation│ │ Apply   │ │  Tx    │ │  Cache   │
└──────────┘ └─────────┘ └────────┘ └──────────┘
  v1.rs       reward.rs   special_tx  state_root
  v2/                      .rs         _cache.rs
```

## Implementation Details

### 1. `sync.rs` - Sync Coordinator

**Purpose:** Provide XDC-specific sync configuration and executor wrapper.

**Key Types:**
```rust
/// XDC sync configuration
pub struct XdcSyncConfig {
    /// XDPoS configuration
    pub xdpos_config: XDPoSConfig,
    /// State root cache for checkpoint blocks
    pub state_root_cache: XdcStateRootCache,
    /// Chain ID (50 = mainnet, 51 = apothem)
    pub chain_id: u64,
}

/// XDC block executor wrapper
pub struct XdcBlockExecutor<E, P> {
    /// Base EVM executor
    base_executor: E,
    /// State provider
    state_provider: P,
    /// XDC sync config
    config: XdcSyncConfig,
    /// Reward calculator
    reward_calculator: RewardCalculator,
}
```

**Responsibilities:**
- Wrap standard Reth executor with XDC logic
- Coordinate reward application at checkpoints
- Integrate state root cache for validation
- Handle special transaction gas exemptions

### 2. `execution.rs` - Block Execution Hooks

**Purpose:** Implement execution-time hooks for XDC consensus.

**Key Functions:**
```rust
/// Pre-execution: Determine consensus version
pub fn pre_execute_block(
    block_number: u64,
    config: &XDPoSConfig,
) -> ConsensusVersion {
    if config.is_v2(block_number) {
        ConsensusVersion::V2
    } else {
        ConsensusVersion::V1
    }
}

/// Post-execution: Apply rewards at checkpoints
pub fn post_execute_block(
    block_number: u64,
    state: &mut State,
    state_provider: &impl StateProvider,
    config: &XDPoSConfig,
) -> Result<()> {
    if is_checkpoint_block(block_number, config.epoch) {
        apply_checkpoint_rewards(
            block_number,
            state,
            state_provider,
            config,
        )?;
    }
    Ok(())
}

/// State root override: Check cache for known divergence
pub fn finalize_state_root(
    block_number: u64,
    header_state_root: B256,
    computed_state_root: B256,
    cache: &XdcStateRootCache,
) -> B256 {
    if is_checkpoint_block(block_number, 900) {
        cache.get_or_store(
            block_number,
            header_state_root,
            computed_state_root,
        )
    } else {
        computed_state_root
    }
}
```

## Integration with Reth Pipeline

### Standard Reth Execution Flow

```rust
// crates/stages/stages/src/stages/execution/mod.rs
impl<E> Stage for ExecutionStage<E> {
    fn execute(&mut self, input: ExecInput) -> Result<ExecOutput> {
        for block_number in input.range() {
            let block = self.provider.block(block_number)?;
            
            // Execute block
            let outcome = self.executor.execute(block)?;
            
            // Verify state root
            if outcome.state_root != block.header.state_root {
                return Err(StageError::Validation(...));
            }
        }
    }
}
```

### XDC-Adapted Execution Flow

```rust
// Our custom executor wraps the standard one
impl<E, P> Executor for XdcBlockExecutor<E, P> {
    fn execute(&mut self, block: Block) -> Result<ExecutionOutcome> {
        // 1. Check consensus version
        let version = pre_execute_block(block.number, &self.config.xdpos_config);
        
        // 2. Execute transactions (with special tx handling)
        let mut outcome = self.execute_transactions(block, version)?;
        
        // 3. Apply checkpoint rewards if needed
        post_execute_block(
            block.number,
            &mut outcome.state,
            &self.state_provider,
            &self.config.xdpos_config,
        )?;
        
        // 4. Compute state root
        let computed_root = outcome.state.root()?;
        
        // 5. Finalize state root (check cache for checkpoint blocks)
        outcome.state_root = finalize_state_root(
            block.number,
            block.header.state_root,
            computed_root,
            &self.config.state_root_cache,
        );
        
        Ok(outcome)
    }
}
```

## Checkpoint Block Execution Flow

Detailed execution sequence for checkpoint blocks (N % 900 == 0, N > 0):

```
Block 1800 Execution:
├─ 1. Execute all transactions in block
│  ├─ Check if tx.to == 0x89 || tx.to == 0x90 && block >= 3M
│  ├─ If yes: set gas_price = 0 (free tx)
│  └─ Execute with EVM
├─ 2. Post-transaction state updates
│  └─ State after all txs executed
├─ 3. Checkpoint reward application
│  ├─ Walk blocks 901 to 1799 (previous epoch)
│  ├─ Count signatures per validator (from 0x89 txs)
│  ├─ Calculate rewards: (250 XDC / total_signs) * validator_signs
│  ├─ Apply to state:
│  │  ├─ validator.balance += reward * 0.90
│  │  └─ foundation.balance += reward * 0.10
│  └─ State now includes reward changes
├─ 4. Compute state root
│  └─ computed_root = merkle_root(state)
├─ 5. State root validation
│  ├─ Check: computed_root == header.state_root?
│  ├─ If NO:
│  │  ├─ Check cache: local_root = cache.get(header.state_root)?
│  │  ├─ If found: use local_root (known divergence)
│  │  └─ If not: store mapping(block, header_root, computed_root)
│  └─ If YES: use computed_root
└─ 6. Return execution outcome with final state root
```

## Error Handling

### State Root Mismatch at Non-Checkpoint

```rust
if !is_checkpoint_block(block.number, 900) {
    if computed_root != header_root {
        // This is a real error - reject block
        return Err(ConsensusError::InvalidStateRoot {
            block: block.number,
            expected: header_root,
            got: computed_root,
        });
    }
}
```

### State Root Mismatch at Checkpoint

```rust
if is_checkpoint_block(block.number, 900) {
    if computed_root != header_root {
        // Check cache first
        match cache.get_local_root(header_root) {
            Some(cached) => {
                // Known divergence - use cached value
                state_root = cached;
            }
            None => {
                // New checkpoint - this might be first time syncing
                // Store mapping for future
                cache.store_mapping(block.number, header_root, computed_root);
                state_root = computed_root;
                
                // Log warning
                warn!("New checkpoint state root mapping stored",
                    block = block.number,
                    remote = header_root,
                    local = computed_root,
                );
            }
        }
    }
}
```

## Testing Strategy

### Unit Tests

1. **Checkpoint Reward Application**
   - Test reward calculation at epoch boundaries
   - Verify 90/10 split (masternode/foundation)
   - Verify state changes are included in root

2. **Special Transaction Handling**
   - Test free gas before/after block 3M
   - Test only 0x89/0x90 are free
   - Verify other addresses pay gas normally

3. **V1/V2 Consensus Switch**
   - Test validation switches at correct block
   - Verify V1 rules before switch
   - Verify V2 rules after switch

### Integration Tests

1. **Full Sync Simulation**
   - Sync first 2700 blocks (3 epochs)
   - Verify checkpoint rewards applied correctly
   - Verify state root cache populated

2. **State Root Cache Hit/Miss**
   - Test cache lookup on known checkpoints
   - Test cache storage on new checkpoints
   - Verify persistence across restarts

3. **Cross-Version Sync**
   - Sync through V1→V2 switch block
   - Verify consensus rules change correctly
   - Verify no state corruption at boundary

## Performance Considerations

### State Root Cache

- **Size:** 10M entries (prevents eviction during full sync)
- **Persistence:** Every 100 blocks (reduces I/O overhead)
- **Lookup:** O(1) HashMap lookup (fast path for cache hits)

### Reward Calculation

- **Frequency:** Every 900 blocks (not every block)
- **Epoch Scan:** 900 blocks backward (manageable overhead)
- **Optimization:** Only count blocks at MERGE_SIGN_RANGE intervals

### Special Transaction Check

- **Cost:** Single address comparison (negligible)
- **Frequency:** Every transaction (but very fast)

## Migration Path

### Phase 8a: Basic Execution (Current)
- Implement `sync.rs` and `execution.rs`
- Integrate with `ExecutionStage`
- Add checkpoint reward hooks
- Add state root cache checks

### Phase 8b: Testing & Validation
- Unit tests for all hooks
- Integration tests for full sync
- Testnet sync validation

### Phase 8c: Optimization
- Profile checkpoint performance
- Optimize reward calculation
- Tune cache persistence interval

## References

- **Phase 3:** `reward.rs` - Reward calculation algorithm
- **Phase 5:** `special_tx.rs` - Free gas transaction detection
- **Phase 6:** `state_root_cache.rs` - State root mapping cache
- **Go XDC:** `/root/.openclaw/workspace/go-ethereum/eth/downloader/downloader_xdc.go`
- **Reth Execution:** `crates/stages/stages/src/stages/execution/mod.rs`
- **Gnosis Integration:** `RETH-NETWORK-INTEGRATIONS-ANALYSIS.md`

## Open Questions

1. **Foundation Wallet Address:** Currently set to `Address::ZERO` in config - needs actual mainnet/testnet addresses
2. **V2 Apothem Switch Block:** Currently 0 in config - needs actual testnet switch block number
3. **Cache Size Tuning:** 10M entries may be excessive for Apothem testnet - consider dynamic sizing?

## Conclusion

This sync engine integration ties together all previous XDC consensus phases:
- Uses reward calculator (Phase 3) for checkpoint rewards
- Uses special tx logic (Phase 5) for gas exemptions
- Uses state root cache (Phase 6) for validation bypass
- Uses V1/V2 validation (Phase 4 & 7) for consensus rules

The result is a complete sync implementation that can:
- Perform full sync from genesis
- Handle checkpoint blocks correctly
- Apply rewards before state root computation
- Validate against cached state roots
- Switch consensus versions dynamically
