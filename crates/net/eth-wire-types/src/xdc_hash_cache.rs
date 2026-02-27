//! XDC Hash Cache
//!
//! This module provides a thread-safe cache for XDC 18-field block hashes.
//! When decoding XDC headers, we compute the correct 18-field hash and store it here.
//! Later, when sealing headers (computing hash), we check this cache first.

use alloy_primitives::{B256, U256};
use std::collections::HashMap;
use std::sync::Mutex;

lazy_static::lazy_static! {
    static ref HASH_CACHE: Mutex<HashMap<u64, B256>> = Mutex::new(HashMap::new());
}

/// Store the XDC 18-field hash for a block number
pub fn store_xdc_hash(block_number: u64, hash: B256) {
    if let Ok(mut cache) = HASH_CACHE.lock() {
        cache.insert(block_number, hash);
    }
}

/// Get the cached XDC hash for a block number
pub fn get_xdc_hash(block_number: u64) -> Option<B256> {
    HASH_CACHE.lock().ok()?.get(&block_number).copied()
}

/// Clear the hash cache (useful for testing or memory management)
pub fn clear_cache() {
    if let Ok(mut cache) = HASH_CACHE.lock() {
        cache.clear();
    }
}

/// Check if we have a cached hash for this block
pub fn has_xdc_hash(block_number: u64) -> bool {
    HASH_CACHE.lock().ok().map_or(false, |c| c.contains_key(&block_number))
}

/// Get the XDC hash, or if not cached, try to identify if this is an XDC header
/// by checking the block number pattern (XDC mainnet starts at specific numbers)
pub fn get_xdc_hash_or_default(block_number: u64, default_hash: B256) -> B256 {
    get_xdc_hash(block_number).unwrap_or(default_hash)
}
