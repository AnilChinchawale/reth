//! XDC State Root Cache
//!
//! Persistent cache mapping remote (geth v2.6.8) state roots to locally computed state roots.
//!
//! ## Problem
//! At checkpoint blocks (every 900 blocks starting at 1800), reward distribution causes different
//! state roots between XDC clients due to:
//! - Different execution order
//! - Different gas calculation
//! - EIP-158/161 handling differences
//!
//! Since all subsequent blocks inherit the diverged state, every block from 1800 onwards has
//! a different state root than what geth v2.6.8 computes.
//!
//! ## Solution
//! This cache maintains a mapping of `remote_root → local_root` and `block_number → local_root`
//! to:
//! 1. Validate block headers by replacing remote state roots with cached local roots
//! 2. Store mappings when computing state for future use
//! 3. Persist to disk so restarts don't cause chain rewind (critical!)
//!
//! ## Architecture
//! - **Thread-safe**: Uses `parking_lot::RwLock` for concurrent access
//! - **Persistent**: Saves full mapping to disk every 100 blocks
//! - **Large capacity**: 10M entries to prevent eviction-related crashes
//! - **Backward scan**: On startup, scans last 10K blocks to find valid state
//! - **Chain-specific**: Only active for chainId 50 (mainnet) and 51 (testnet)

use alloy_primitives::B256;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs::{self, File},
    io::{BufRead, BufReader, BufWriter, Write},
    path::PathBuf,
    sync::Arc,
};

/// Maximum number of entries before eviction (10 million blocks)
/// This is critical - smaller caches cause eviction-related crashes in production
pub const MAX_CACHE_ENTRIES: usize = 10_000_000;

/// Persist to disk every N blocks
pub const PERSIST_INTERVAL: u64 = 100;

/// Backward scan range on startup (last 10K blocks)
pub const BACKWARD_SCAN_RANGE: u64 = 10_000;

/// Persistent cache mapping remote state roots to local state roots
#[derive(Clone)]
pub struct XdcStateRootCache {
    inner: Arc<RwLock<CacheInner>>,
}

struct CacheInner {
    /// remote_root → local_root mapping
    remote_to_local: HashMap<B256, B256>,
    /// block_number → local_root (for restart recovery)
    block_roots: HashMap<u64, B256>,
    /// block_number → remote_root (for reverse lookup during eviction)
    block_to_remote: HashMap<u64, B256>,
    /// Path for disk persistence
    persist_path: Option<PathBuf>,
    /// Max entries before eviction
    max_entries: usize,
    /// Last persisted block number
    last_persisted_block: u64,
}

/// Entry for disk persistence (CSV format)
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CacheEntry {
    block_number: u64,
    remote_root: String,
    local_root: String,
}

impl XdcStateRootCache {
    /// Create a new state root cache
    ///
    /// # Arguments
    /// * `persist_path` - Optional path for disk persistence file
    /// * `max_entries` - Maximum entries before eviction (default: 10M)
    pub fn new(persist_path: Option<PathBuf>, max_entries: usize) -> Self {
        let inner = CacheInner {
            remote_to_local: HashMap::new(),
            block_roots: HashMap::new(),
            block_to_remote: HashMap::new(),
            persist_path,
            max_entries,
            last_persisted_block: 0,
        };

        Self {
            inner: Arc::new(RwLock::new(inner)),
        }
    }

    /// Create with default settings (10M entries)
    pub fn with_default_size(persist_path: Option<PathBuf>) -> Self {
        Self::new(persist_path, MAX_CACHE_ENTRIES)
    }

    /// Load cache from disk
    ///
    /// This reads the persisted cache file in CSV format and populates the in-memory cache.
    /// On failure, logs a warning and continues with an empty cache.
    pub fn load(&self) -> Result<usize, Box<dyn std::error::Error>> {
        let mut inner = self.inner.write();
        
        let persist_path = match &inner.persist_path {
            Some(path) => path,
            None => return Ok(0),
        };

        if !persist_path.exists() {
            tracing::info!("No state root cache file found at {:?}", persist_path);
            return Ok(0);
        }

        let file = File::open(persist_path)?;
        let reader = BufReader::new(file);
        let mut count = 0;

        for (line_num, line) in reader.lines().enumerate() {
            let line = line?;
            
            // Skip header line
            if line_num == 0 && line.starts_with("block_number") {
                continue;
            }

            // Parse CSV: block_number,remote_root_hex,local_root_hex
            let parts: Vec<&str> = line.split(',').collect();
            if parts.len() != 3 {
                tracing::warn!("Invalid cache line (line {}): {}", line_num, line);
                continue;
            }

            let block_number: u64 = match parts[0].trim().parse() {
                Ok(n) => n,
                Err(e) => {
                    tracing::warn!("Invalid block number at line {}: {}", line_num, e);
                    continue;
                }
            };

            let remote_root = match parse_hash(parts[1].trim()) {
                Ok(h) => h,
                Err(e) => {
                    tracing::warn!("Invalid remote root at line {}: {}", line_num, e);
                    continue;
                }
            };

            let local_root = match parse_hash(parts[2].trim()) {
                Ok(h) => h,
                Err(e) => {
                    tracing::warn!("Invalid local root at line {}: {}", line_num, e);
                    continue;
                }
            };

            inner.remote_to_local.insert(remote_root, local_root);
            inner.block_roots.insert(block_number, local_root);
            inner.block_to_remote.insert(block_number, remote_root);
            
            if block_number > inner.last_persisted_block {
                inner.last_persisted_block = block_number;
            }

            count += 1;
        }

        tracing::info!(
            "Loaded {} state root mappings from disk (up to block {})",
            count,
            inner.last_persisted_block
        );

        Ok(count)
    }

    /// Save cache to disk
    ///
    /// Writes the full cache to disk in CSV format for crash recovery.
    /// This is called automatically every PERSIST_INTERVAL blocks.
    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let inner = self.inner.read();
        
        let persist_path = match &inner.persist_path {
            Some(path) => path,
            None => return Ok(()),
        };

        // Create parent directory if needed
        if let Some(parent) = persist_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Write to temporary file first, then rename (atomic operation)
        let temp_path = persist_path.with_extension("tmp");
        let file = File::create(&temp_path)?;
        let mut writer = BufWriter::new(file);

        // Write CSV header
        writeln!(writer, "block_number,remote_root_hex,local_root_hex")?;

        // Write all entries sorted by block number
        let mut entries: Vec<_> = inner.block_roots.iter().collect();
        entries.sort_by_key(|(block_num, _)| *block_num);

        for (block_number, local_root) in entries {
            if let Some(remote_root) = inner.block_to_remote.get(block_number) {
                writeln!(
                    writer,
                    "{},{},{}",
                    block_number,
                    format_hash(remote_root),
                    format_hash(local_root)
                )?;
            }
        }

        writer.flush()?;
        drop(writer);

        // Atomic rename
        fs::rename(&temp_path, persist_path)?;

        tracing::debug!(
            "Persisted {} state root mappings to disk",
            inner.block_roots.len()
        );

        Ok(())
    }

    /// Get local root for a remote root
    ///
    /// This is used during block validation to translate the remote (geth) state root
    /// from the block header to our locally computed state root.
    pub fn get_local_root(&self, remote_root: &B256) -> Option<B256> {
        let inner = self.inner.read();
        inner.remote_to_local.get(remote_root).copied()
    }

    /// Store a mapping of remote root → local root
    ///
    /// Called after computing state to cache the mapping for future validation.
    /// Automatically persists to disk every PERSIST_INTERVAL blocks.
    pub fn insert(&self, remote_root: B256, local_root: B256, block_number: u64) {
        let mut inner = self.inner.write();

        // Don't cache if roots are identical (no divergence)
        if remote_root == local_root {
            return;
        }

        inner.remote_to_local.insert(remote_root, local_root);
        inner.block_roots.insert(block_number, local_root);
        inner.block_to_remote.insert(block_number, remote_root);

        // Evict old entries if cache is full
        let should_evict = inner.block_roots.len() > inner.max_entries;
        if should_evict {
            let evict_count = inner.max_entries / 10;
            Self::evict_oldest(&mut inner, evict_count); // Evict 10%
        }

        // Persist to disk periodically
        if block_number - inner.last_persisted_block >= PERSIST_INTERVAL {
            inner.last_persisted_block = block_number;
            drop(inner); // Release lock before I/O
            
            if let Err(e) = self.save() {
                tracing::warn!("Failed to persist state root cache: {}", e);
            }
        }
    }

    /// Get local root by block number (for restart recovery)
    pub fn get_root_by_block(&self, block_number: u64) -> Option<B256> {
        let inner = self.inner.read();
        inner.block_roots.get(&block_number).copied()
    }

    /// Backward scan for valid root on startup (prevents genesis rewind)
    ///
    /// Scans backwards from `from_block` up to `scan_range` blocks to find the first
    /// valid cached state root. This prevents the client from rewinding to genesis
    /// on restart when the cache is incomplete.
    ///
    /// # Returns
    /// `Some((block_number, local_root))` if a valid root is found, `None` otherwise
    pub fn find_valid_root(&self, from_block: u64, scan_range: u64) -> Option<(u64, B256)> {
        let inner = self.inner.read();
        
        let start_block = from_block.saturating_sub(scan_range);
        
        for block_num in (start_block..=from_block).rev() {
            if let Some(root) = inner.block_roots.get(&block_num) {
                tracing::info!(
                    "Found valid state root at block {} (scanned back {} blocks)",
                    block_num,
                    from_block - block_num
                );
                return Some((block_num, *root));
            }
        }

        tracing::warn!(
            "No valid state root found in backward scan (blocks {} to {})",
            start_block,
            from_block
        );

        None
    }

    /// Get cache statistics
    pub fn stats(&self) -> CacheStats {
        let inner = self.inner.read();
        CacheStats {
            remote_to_local_count: inner.remote_to_local.len(),
            block_roots_count: inner.block_roots.len(),
            last_persisted_block: inner.last_persisted_block,
            max_entries: inner.max_entries,
        }
    }

    /// Clear the cache (for testing)
    #[cfg(test)]
    pub fn clear(&self) {
        let mut inner = self.inner.write();
        inner.remote_to_local.clear();
        inner.block_roots.clear();
        inner.block_to_remote.clear();
        inner.last_persisted_block = 0;
    }

    /// Evict oldest entries to free up space
    fn evict_oldest(inner: &mut CacheInner, count: usize) {
        let mut blocks: Vec<u64> = inner.block_roots.keys().copied().collect();
        blocks.sort_unstable();

        let to_evict = blocks.iter().take(count);
        
        for block_num in to_evict {
            if let Some(remote_root) = inner.block_to_remote.remove(block_num) {
                inner.remote_to_local.remove(&remote_root);
            }
            inner.block_roots.remove(block_num);
        }

        tracing::debug!("Evicted {} old state root cache entries", count);
    }
}

/// Cache statistics
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub remote_to_local_count: usize,
    pub block_roots_count: usize,
    pub last_persisted_block: u64,
    pub max_entries: usize,
}

/// Parse a hex string to B256
fn parse_hash(s: &str) -> Result<B256, String> {
    let s = s.trim().trim_start_matches("0x");
    
    if s.len() != 64 {
        return Err(format!("Invalid hash length: {}", s.len()));
    }

    let mut bytes = [0u8; 32];
    for (i, chunk) in s.as_bytes().chunks(2).enumerate() {
        let hex_str = std::str::from_utf8(chunk).map_err(|e| e.to_string())?;
        bytes[i] = u8::from_str_radix(hex_str, 16).map_err(|e| e.to_string())?;
    }

    Ok(B256::from(bytes))
}

/// Format B256 as hex string
fn format_hash(hash: &B256) -> String {
    format!("0x{}", hex::encode(hash.as_slice()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn random_hash() -> B256 {
        B256::random()
    }

    #[test]
    fn test_insert_and_retrieve() {
        let cache = XdcStateRootCache::with_default_size(None);
        
        let remote = random_hash();
        let local = random_hash();
        let block = 1800;

        cache.insert(remote, local, block);

        assert_eq!(cache.get_local_root(&remote), Some(local));
        assert_eq!(cache.get_root_by_block(block), Some(local));
    }

    #[test]
    fn test_skip_identical_roots() {
        let cache = XdcStateRootCache::with_default_size(None);
        
        let same_root = random_hash();
        cache.insert(same_root, same_root, 100);

        // Should not cache identical roots
        assert_eq!(cache.get_local_root(&same_root), None);
    }

    #[test]
    fn test_disk_persistence() {
        let temp_dir = TempDir::new().unwrap();
        let cache_path = temp_dir.path().join("state-root-cache.csv");

        // Create cache and insert entries
        let cache = XdcStateRootCache::with_default_size(Some(cache_path.clone()));
        
        let entries: Vec<_> = (1800..1900)
            .step_by(10)
            .map(|block| {
                let remote = random_hash();
                let local = random_hash();
                cache.insert(remote, local, block);
                (block, remote, local)
            })
            .collect();

        // Save to disk
        cache.save().unwrap();
        assert!(cache_path.exists());

        // Create new cache and load
        let cache2 = XdcStateRootCache::with_default_size(Some(cache_path.clone()));
        let loaded_count = cache2.load().unwrap();
        
        assert_eq!(loaded_count, entries.len());

        // Verify all entries
        for (block, remote, local) in entries {
            assert_eq!(cache2.get_local_root(&remote), Some(local));
            assert_eq!(cache2.get_root_by_block(block), Some(local));
        }
    }

    #[test]
    fn test_backward_scan() {
        let cache = XdcStateRootCache::with_default_size(None);
        
        // Insert some entries
        for block in [1800, 2700, 3600, 4500] {
            let remote = random_hash();
            let local = random_hash();
            cache.insert(remote, local, block);
        }

        // Scan backward from block 5000
        let result = cache.find_valid_root(5000, 2000);
        assert!(result.is_some());
        let (found_block, _) = result.unwrap();
        assert_eq!(found_block, 4500); // Should find the closest cached block

        // Scan backward from block 3000
        let result = cache.find_valid_root(3000, 1500);
        assert!(result.is_some());
        let (found_block, _) = result.unwrap();
        assert_eq!(found_block, 2700);
    }

    #[test]
    fn test_backward_scan_not_found() {
        let cache = XdcStateRootCache::with_default_size(None);
        
        cache.insert(random_hash(), random_hash(), 5000);

        // Scan a range that doesn't include the cached block
        let result = cache.find_valid_root(2000, 500);
        assert!(result.is_none());
    }

    #[test]
    fn test_cache_eviction() {
        let cache = XdcStateRootCache::new(None, 100); // Small cache for testing
        
        // Fill cache beyond capacity
        for block in 1..=150 {
            let remote = random_hash();
            let local = random_hash();
            cache.insert(remote, local, block);
        }

        let stats = cache.stats();
        assert!(stats.block_roots_count <= 100);
    }

    #[test]
    fn test_thread_safety() {
        use std::thread;
        
        let cache = XdcStateRootCache::with_default_size(None);
        let cache_clone = cache.clone();

        // Writer thread
        let writer = thread::spawn(move || {
            for block in 1800..1900 {
                let remote = random_hash();
                let local = random_hash();
                cache_clone.insert(remote, local, block);
            }
        });

        // Reader thread
        let cache_clone2 = cache.clone();
        let reader = thread::spawn(move || {
            for _ in 0..100 {
                let _ = cache_clone2.stats();
            }
        });

        writer.join().unwrap();
        reader.join().unwrap();
    }

    #[test]
    fn test_parse_hash() {
        let hash_str = "0xabcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890";
        let result = parse_hash(hash_str);
        assert!(result.is_ok());

        let hash_no_prefix = "abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890";
        let result = parse_hash(hash_no_prefix);
        assert!(result.is_ok());

        // Invalid length
        let result = parse_hash("0xabcd");
        assert!(result.is_err());
    }

    #[test]
    fn test_format_hash() {
        let hash = B256::from([0xab; 32]);
        let formatted = format_hash(&hash);
        assert!(formatted.starts_with("0x"));
        assert_eq!(formatted.len(), 66); // "0x" + 64 hex chars
    }

    #[test]
    fn test_stats() {
        let cache = XdcStateRootCache::with_default_size(None);
        
        for block in 1800..1810 {
            cache.insert(random_hash(), random_hash(), block);
        }

        let stats = cache.stats();
        assert_eq!(stats.block_roots_count, 10);
        assert_eq!(stats.remote_to_local_count, 10);
        assert_eq!(stats.max_entries, MAX_CACHE_ENTRIES);
    }

    #[test]
    fn test_auto_persist() {
        let temp_dir = TempDir::new().unwrap();
        let cache_path = temp_dir.path().join("state-root-cache.csv");
        let cache = XdcStateRootCache::with_default_size(Some(cache_path.clone()));

        // Insert entries up to trigger auto-persist
        for block in 1800..(1800 + PERSIST_INTERVAL + 1) {
            cache.insert(random_hash(), random_hash(), block);
        }

        // Give it a moment for async persist
        std::thread::sleep(std::time::Duration::from_millis(100));

        // File should exist
        assert!(cache_path.exists());
    }
}
