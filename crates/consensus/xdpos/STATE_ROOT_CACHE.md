# XDC State Root Cache Implementation

## Overview

The XDC State Root Cache is a critical component for XDC blockchain synchronization that addresses state root divergence between different XDC client implementations at checkpoint blocks.

## Problem Statement

At checkpoint blocks (every 900 blocks starting at block 1800), XDC's reward distribution mechanism causes different state roots to be computed by different client implementations due to:

- **Different execution order**: Transaction processing order variations
- **Different gas calculations**: Subtle differences in gas metering
- **EIP-158/161 handling**: Account cleanup implementation differences

Since blockchain state is cumulative, this divergence affects **all subsequent blocks**, not just checkpoint blocks. This means from block 1800 onwards, every block has a different state root than what geth v2.6.8 computes.

## Solution Architecture

The `XdcStateRootCache` maintains a persistent mapping between:
- Remote state roots (from geth v2.6.8, stored in downloaded block headers)
- Local state roots (computed by this client during execution)

### Key Features

1. **Persistent Storage**: Saves mappings to disk in CSV format
2. **Thread-Safe**: Uses `parking_lot::RwLock` for concurrent access
3. **Large Capacity**: Supports 10 million entries to prevent eviction-related crashes
4. **Auto-Persistence**: Saves every 100 blocks and on shutdown
5. **Backward Scan**: On startup, scans last 10,000 blocks to find valid state
6. **Chain-Specific**: Only active for chainId 50 (mainnet) and 51 (testnet)

## Implementation Details

### Data Structures

```rust
pub struct XdcStateRootCache {
    /// remote_root → local_root mapping
    remote_to_local: HashMap<B256, B256>,
    /// block_number → local_root (for restart recovery)
    block_roots: HashMap<u64, B256>,
    /// block_number → remote_root (for cleanup during eviction)
    block_to_remote: HashMap<u64, B256>,
    /// Disk persistence path
    persist_path: Option<PathBuf>,
    /// Maximum entries before eviction (default: 10M)
    max_entries: usize,
    /// Last persisted block number
    last_persisted_block: u64,
}
```

### Disk Format

CSV format for human readability and easy debugging:

```csv
block_number,remote_root_hex,local_root_hex
1800,0xabc...,0xdef...
2700,0x123...,0x456...
```

### API Usage

#### Creating the Cache

```rust
use reth_consensus_xdpos::XdcStateRootCache;

// With persistence
let cache = XdcStateRootCache::with_default_size(
    Some(PathBuf::from("/data/xdc-state-root-cache.csv"))
);

// Load from disk
cache.load().expect("Failed to load cache");
```

#### During Block Validation

```rust
// Get local root for a remote (header) root
if let Some(local_root) = cache.get_local_root(&block_header.state_root) {
    // Use local_root for validation instead of block_header.state_root
    validate_state(&local_root)?;
}
```

#### After State Execution

```rust
// Store the mapping
cache.insert(
    remote_root,  // From block header
    local_root,   // Computed by execution
    block_number
);

// Cache automatically persists every 100 blocks
```

#### Restart Recovery

```rust
// Find last valid state root
if let Some((block_num, root)) = cache.find_valid_root(head_block, 10_000) {
    println!("Resuming from block {} with root {}", block_num, root);
}
```

## Performance Considerations

### Memory Usage

With 10 million entries:
- Each entry: ~96 bytes (3 × 32-byte hashes + overhead)
- Total: ~960 MB maximum

### Disk I/O

- **Writes**: Buffered writes every 100 blocks (~1 KB/write)
- **Reads**: Single read on startup (typically < 100ms for 10M entries)
- **Atomic Writes**: Uses temp file + rename for crash safety

### Thread Safety

- Read operations: Lock-free for multiple concurrent readers
- Write operations: Single writer lock
- No lock contention during normal operation

## Testing

The implementation includes comprehensive tests:

1. **Basic Operations**: Insert and retrieve
2. **Persistence**: Save and load from disk
3. **Backward Scan**: Recovery from incomplete cache
4. **Eviction**: Automatic cleanup when full
5. **Thread Safety**: Concurrent access validation
6. **Edge Cases**: Identical roots, missing files, corrupt data

Run tests:

```bash
cargo test -p reth-consensus-xdpos state_root_cache
```

## Integration Guide

### Step 1: Initialize Cache on Node Startup

```rust
let cache = XdcStateRootCache::with_default_size(
    Some(data_dir.join("xdc-state-root-cache.csv"))
);

// Load existing cache
if let Err(e) = cache.load() {
    tracing::warn!("Failed to load state root cache: {}", e);
    // Continue with empty cache
}
```

### Step 2: Use in Block Validation

```rust
fn validate_block_header(&self, header: &Header, cache: &XdcStateRootCache) -> Result<()> {
    let state_root = if let Some(local) = cache.get_local_root(&header.state_root) {
        local  // Use cached local root
    } else {
        header.state_root  // Use header root (before first checkpoint)
    };
    
    // Validate using the resolved root
    self.validate_state_root(state_root)?;
    Ok(())
}
```

### Step 3: Store After Execution

```rust
fn execute_block(&self, block: &Block, cache: &XdcStateRootCache) -> Result<B256> {
    let local_root = self.compute_state_root(block)?;
    
    // Store mapping (only if diverged)
    cache.insert(
        block.header.state_root,  // Remote root
        local_root,                // Local root
        block.number
    );
    
    Ok(local_root)
}
```

### Step 4: Graceful Shutdown

```rust
fn shutdown(&self, cache: &XdcStateRootCache) {
    if let Err(e) = cache.save() {
        tracing::error!("Failed to save state root cache: {}", e);
    }
}
```

## Troubleshooting

### Cache Miss on Restart

**Symptom**: Client rewinds to genesis on restart

**Solution**: The backward scan should prevent this. If it happens:
1. Check if cache file exists and is readable
2. Verify cache contains entries near the head block
3. Increase `BACKWARD_SCAN_RANGE` if needed

### Memory Usage Too High

**Symptom**: Process using > 1 GB for cache

**Solution**:
1. Reduce `MAX_CACHE_ENTRIES` (minimum: 1M for safety)
2. Eviction should automatically trigger at configured limit
3. Monitor `CacheStats` to verify eviction is working

### Slow Sync After Cache Clear

**Symptom**: Slow sync after deleting cache file

**Solution**: This is expected! The cache needs to be rebuilt. It will catch up as blocks are processed. Consider:
1. Keeping the old cache file as backup
2. Syncing from a checkpoint block to minimize diverged blocks
3. Copying cache from another synchronized node

## Production Deployment Checklist

- [ ] Configure persistent path in node config
- [ ] Ensure write permissions for cache directory
- [ ] Set up log monitoring for cache-related errors
- [ ] Back up cache file during upgrades
- [ ] Monitor memory usage (expect ~1 GB max)
- [ ] Verify cache stats periodically

## Comparison with Reference Implementations

### Nethermind
- Uses `ConcurrentDictionary` (C#) vs `RwLock<HashMap>` (Rust)
- Persists full mapping vs incremental updates
- Same 10M entry limit
- Similar backward scan logic

### Go-Ethereum (GP5)
- If implemented, likely uses similar approach
- XDC's geth v2.6.8 is the reference (doesn't need cache)

## License

Same as reth-consensus-xdpos crate (MIT/Apache-2.0)

## Author

Implementation based on Nethermind reference by Anil Chinchawale.
