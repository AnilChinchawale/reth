# XDC BlockHeaders Decode Fix

## Problem
When Reth XDC received BlockHeaders responses from peers (buf len=677), decoding would start but never complete - no success or error messages appeared, and block sync stayed at 0.

## Root Cause
**Type Mismatch in Conversion Logic**

In `crates/net/eth-wire-types/src/message.rs` (lines 148-167), the eth/63 BlockHeaders decode path had a critical flaw:

1. ❌ Called `decode_xdc_block_headers(buf)` which decoded 18-field XDC headers into 15-field standard `alloy_consensus::Header` structs (stripping the 3 XDC-specific fields: validators, validator, penalties)

2. ❌ Re-encoded each 15-field `Header` back to RLP bytes

3. ❌ Tried to decode the re-encoded bytes as `N::BlockHeader` (which is `XdcBlockHeader` expecting **18 fields**)

4. ❌ `XdcBlockHeader::decode()` failed/hung because it expected 18 fields but only received 15 from the re-encoded buffer

**Result:** Decoding never completed, no error was visible, and headers were never passed to the sync logic.

## The Fix

**Decode headers directly as `N::BlockHeader` (XdcBlockHeader) without intermediate conversion:**

```rust
// OLD (BROKEN):
let std_headers = crate::xdc_header::decode_xdc_block_headers(buf)?;
for h in std_headers {
    let mut re_encoded = Vec::new();
    Encodable::encode(&h, &mut re_encoded);
    let nh = N::BlockHeader::decode(&mut &re_encoded[..])?; // FAILS - expects 18, gets 15
    eth_headers.push(nh);
}

// NEW (FIXED):
let list_header = alloy_rlp::Header::decode(buf)?;
let started_len = buf.len();
let mut headers = Vec::new();

while started_len - buf.len() < list_header.payload_length {
    let header = N::BlockHeader::decode(buf)?; // Directly decode as XdcBlockHeader (18 fields)
    headers.push(header);
}
```

## Changes Made

### 1. `crates/net/eth-wire-types/src/message.rs` (lines 147-176)
- Removed the broken re-encode/decode logic
- Decode headers directly as `Vec<N::BlockHeader>` using the proper `XdcBlockHeader::decode()` implementation
- Added debug prints to confirm successful decoding

### 2. `crates/net/eth-wire-types/src/xdc_header.rs` (header comment)
- Added clarification that this module is NOT used in the main decode path
- Explained it can still be used for cases where standard `Header` structs are needed

## Why This Works

- `XdcBlockHeader` (from `crates/xdc/primitives/src/header.rs`) implements `Decodable` for the full 18-field XDC header format
- By decoding directly as `N::BlockHeader` (bound to `XdcBlockHeader`), we preserve all 18 fields
- No data loss, no type mismatch, no conversion errors

## Expected Behavior After Fix

When Reth receives BlockHeaders from XDC peers:
1. ✅ Decode starts: `[XDC-DECODE] About to decode XDC headers in message.rs`
2. ✅ Each header decodes: `[XDC-DECODE] Decoded header #0, block=123`
3. ✅ Completion message: `[XDC-DECODE] Successfully decoded N XDC headers`
4. ✅ Headers passed to sync logic via `on_response!` macro
5. ✅ Block number increases as headers are processed

## Files Modified
- `crates/net/eth-wire-types/src/message.rs` - Fixed decode logic
- `crates/net/eth-wire-types/src/xdc_header.rs` - Added clarifying comment

## Testing
Run Reth XDC and connect to the prod Geth node:
```bash
cargo run -- node --chain xdc --port 30304
```

Watch logs for:
- `[XDC-DECODE]` messages showing successful header decoding
- Block number increasing from 0
- Successful peer synchronization
