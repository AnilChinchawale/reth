# Reth XDC Block Sync Fix - Hash Mismatch Issue

## Problem Summary
Reth XDC successfully decoded 105+ headers from P2P but block sync stayed at 0. The root cause was a **hash mismatch** between what FCU expected and what the decoded headers computed.

## Root Cause Analysis

### The Flow
1. **FCU receives target hash** from Erigon (e.g., `0x5b0da10f...`)
   - This hash was computed by Erigon WITHOUT the 3 XDC-specific fields
   - Erigon only hashes the first 15 standard Ethereum fields

2. **Headers downloaded via P2P** (eth/63 protocol)
   - Arrive as 18-field XDC format from GP5 peers
   - Successfully decoded by `decode_xdc_headers_to_eth()` in `crates/xdc/primitives/src/header.rs`
   - Converted to standard 15-field `Header`, then back to `XdcBlockHeader` with empty validators/validator/penalties fields

3. **Header validation in `FetchFullBlockFuture`** (line ~169 in `crates/net/p2p/src/full_block.rs`)
   ```rust
   if header.hash() == this.hash {
       this.header = Some(header);
   } else {
       // Hash mismatch - header rejected!
       debug!("Received wrong header");
       this.client.report_bad_message(peer)
   }
   ```

4. **Hash calculation issue**
   - `XdcBlockHeader::hash_slow()` was encoding ALL 18 fields (including empty XDC fields)
   - The hash included: `[15 standard fields] + [empty validators] + [empty validator] + [empty penalties]`
   - This hash ≠ FCU's expected hash (which only included 15 fields)
   - Result: Header rejected, peer penalized, download retried infinitely → sync stalled at block 0

## The Fix

Modified `XdcBlockHeader::hash_slow()` in `crates/xdc/primitives/src/header.rs`:

### Before (broken):
```rust
pub fn hash_slow(&self) -> B256 {
    let mut buf = Vec::new();
    self.encode(&mut buf);  // Encodes ALL 18 fields
    alloy_primitives::keccak256(&buf)
}
```

### After (fixed):
```rust
pub fn hash_slow(&self) -> B256 {
    let mut buf = Vec::new();
    self.encode_without_xdc_fields(&mut buf);  // Only 15 standard fields
    alloy_primitives::keccak256(&buf)
}
```

Added new method `encode_without_xdc_fields()` that:
- Encodes only the first 15 standard Ethereum header fields
- Includes optional post-London fields (base_fee_per_gas, blob_gas_used, etc.) if present
- **Excludes** the 3 XDC-specific fields (validators, validator, penalties)
- This matches how Erigon/GP5 compute block hashes

## Why This Works

### XDC Block Hash Compatibility
- **GP5/Erigon**: Compute hashes from 15-field headers (XDC fields stored separately)
- **Reth**: Uses 18-field headers but must compute hashes the same way for P2P compatibility
- **Solution**: Hash calculation uses only standard fields, full encoding uses all fields

### Maintaining Two Encodings
1. **Full encoding** (`encode()` method):
   - Used for storage and P2P transmission
   - Includes all 18 fields (standard + XDC)
   - Preserves validator information

2. **Hash encoding** (`encode_without_xdc_fields()` method):
   - Used only for hash calculation
   - Includes only 15 standard fields (+ optional post-London fields)
   - Ensures hash compatibility with other XDC clients

## Expected Outcome

After this fix:
1. Headers downloaded from P2P will have correct hash values
2. `FetchFullBlockFuture` will accept headers (hash match succeeds)
3. Body download will proceed
4. Full blocks will be assembled and inserted into the tree
5. Block number will increase beyond 0
6. Sync will progress normally

## Files Modified

- `crates/xdc/primitives/src/header.rs`:
  - Modified `hash_slow()` to use new encoding method
  - Added `encode_without_xdc_fields()` for hash-compatible encoding
  - Added detailed documentation explaining the compatibility requirement

## Verification Steps

1. Run Reth with the fix
2. Monitor logs for:
   - Headers being accepted (no more "Received wrong header" messages)
   - Body downloads succeeding
   - Block number increasing
   - FCU returning "VALID" instead of "SYNCING"

3. Check that:
   - Headers from P2P have hashes matching FCU expectations
   - Full blocks are being inserted into the tree
   - Sync progresses beyond block 0

## Related Code Locations

- **Header decoding**: `crates/xdc/primitives/src/header.rs` - `decode_xdc_headers_to_eth()`
- **Header validation**: `crates/net/p2p/src/full_block.rs` - `FetchFullBlockFuture::poll()`
- **Tree handling**: `crates/engine/tree/src/tree/mod.rs` - `handle_missing_block()`
- **Download coordination**: `crates/engine/tree/src/download.rs` - `BasicBlockDownloader`
