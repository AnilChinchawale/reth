# XDC Execution Pipeline Integration

This document describes how XDC-specific execution logic (rewards and state root cache) is integrated into Reth's execution pipeline.

## Architecture

### 1. Execution Hooks (crates/consensus/xdpos/src/execution.rs)

Provides functions that can be called during block execution:

- **`should_apply_rewards(block_number, epoch)`** - Check if rewards should be applied
- **`apply_checkpoint_rewards(block_number, outcome, state_provider, reward_calculator)`** - Apply rewards to state
- **`finalize_state_root(block_number, header_root, computed_root, cache, epoch)`** - Check cache for known divergences
- **`validate_state_root(...)`** - Validate state root with cache integration

### 2. Consensus Validation (crates/consensus/xdpos/src/xdpos.rs)

The `XDPoSConsensus` implements Reth's `FullConsensus` trait and includes:

- **State root cache** (`XdcStateRootCache`) - Stores known divergences at checkpoint blocks
- **Reward calculator** (`RewardCalculator`) - Computes reward distribution
- **Post-execution validation** - Validates gas usage and logs checkpoint blocks

### 3. Execution Stage Integration Points

The execution pipeline needs XDC hooks at three points:

#### A. Pre-execution (before transactions)
- Detect consensus version (V1 vs V2)
- Initialize checkpoint state if needed

#### B. Transaction execution
- Apply TIPSigning gas exemptions for system contracts (0x88, 0x89)
- Track validator signatures for reward calculation

#### C. Post-execution (after transactions, before state root)
- **Critical**: Apply checkpoint rewards at `block_number % 900 == 0`
- Modify state to add reward balance changes
- This MUST happen before computing the state root

#### D. State root validation (merkle stage)
- Compare computed state root with header state root
- If they differ at a checkpoint block:
  1. Check `XdcStateRootCache` for known mapping
  2. If found, accept the block (this is normal for XDC)
  3. If not found, store the mapping for future use
- For non-checkpoint blocks, reject mismatches as usual

## Current Status

### ✅ Completed
- State root cache with disk persistence
- Reward calculator logic
- Execution hook functions
- Consensus engine with cache integration
- XDC executor types and configuration

### 🚧 In Progress
- Transaction-level gas exemptions (TIPSigning)
- Full reward application to `ExecutionOutcome`
- State root cache integration into merkle stage

### 📝 TODO
1. **Merkle Stage Integration**
   - Add state root cache check before rejecting divergent roots
   - Only accept cached divergences for chain ID 50/51
   - Log when cache is used vs when it's a new divergence

2. **Reward Application**
   - Complete `apply_checkpoint_rewards()` implementation
   - Walk through epoch blocks (previous 900)
   - Count validator signatures
   - Calculate reward distribution (90% masternode, 10% foundation)
   - Apply balance changes to `ExecutionOutcome`

3. **TIPSigning Gas Exemptions**
   - Hook into transaction execution
   - Set gas price = 0 for txs to 0x88/0x89 after block 3M
   - Verify gas used is correct

## How State Root Cache Works

### Problem
At checkpoint blocks (every 900 blocks), reward distribution causes different state roots between XDC clients due to:
- Different execution order
- Different gas calculation  
- EIP-158/161 handling differences

### Solution
1. **During Execution**: After applying rewards, compute state root
2. **Cache Check**: Compare with header state root
3. **If Different**:
   - Check if `cache.get_local_root(header_root)` returns a value
   - If yes: This is a known divergence, use cached local root
   - If no: Store mapping `header_root -> computed_root` in cache
4. **Accept Block**: Use the finalized root (either direct match or cached)

### Cache Persistence
- Saves to disk every 100 blocks
- Loads on startup
- 10M entry capacity (prevents eviction-related crashes)
- Only active for chain ID 50 (mainnet) and 51 (testnet)

## Example Integration Flow

```
Block 1800 (checkpoint):
├─ Execute transactions
├─ Apply rewards (modify state) ✅ CRITICAL
├─ Compute state root → 0xABCD...
├─ Header says state root = 0x1234...
├─ Check cache for 0x1234...
├─ Found: 0x1234... → 0xABCD...
├─ Accept block with cached mapping
└─ Continue to next block
```

## Files Modified

- `crates/consensus/xdpos/src/xdpos.rs` - Added cache and reward calculator
- `crates/consensus/xdpos/src/execution.rs` - Execution hooks
- `crates/consensus/xdpos/src/state_root_cache.rs` - Cache implementation
- `crates/consensus/xdpos/src/reward.rs` - Reward calculation
- `crates/xdc/node/src/executor.rs` - XDC executor types

## Next Steps for Full Integration

1. **Wire rewards into executor**:
   - Modify `EVM` execution to call `apply_checkpoint_rewards()`
   - Ensure rewards are applied BEFORE state root computation
   
2. **Integrate cache into merkle stage**:
   - Access `consensus.state_root_cache()` in merkle validation
   - Call `finalize_state_root()` before rejecting mismatches
   
3. **Test end-to-end**:
   - Sync from genesis
   - Verify checkpoint blocks 900, 1800, 2700, etc.
   - Check cache grows with new entries
   - Verify no crashes on checkpoint blocks

## API for Merkle Stage

```rust
// In merkle stage, after computing state root:
let consensus = /* get XDPoSConsensus */;
let finalized_root = consensus.validate_state_root(
    block_number,
    header.state_root,
    computed_state_root,
)?;

// Use finalized_root for validation instead of computed_state_root
```

## Testing

```bash
# Build with XDC consensus
cargo build --release -p reth-consensus-xdpos

# Run XDC node
cargo run --release --bin reth -- \
    --chain xdc \
    --datadir /path/to/data

# Monitor checkpoint blocks
grep "Checkpoint block" /path/to/data/logs/reth.log

# Check cache stats
# Cache saves to: <datadir>/xdpos/state_root_cache.json
```

## References

- Reth execution stage: `crates/stages/stages/src/stages/execution/mod.rs`
- Reth merkle stage: `crates/stages/stages/src/stages/merkle/mod.rs`
- Ethereum EVM config: `crates/ethereum/evm/src/execute.rs`
- Gnosis consensus (similar pattern): Reth's Gnosis support (if available)
