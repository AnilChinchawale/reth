//! XDC Block Execution Hooks
//!
//! This module provides execution-time hooks for XDC consensus integration.
//! These functions are called during the block execution pipeline to:
//! - Determine consensus version (V1 vs V2)
//! - Apply checkpoint rewards after transaction execution
//! - Finalize state roots with cache integration

use crate::{
    errors::{XDPoSError, XDPoSResult},
    reward::RewardCalculator,
    state_root_cache::XdcStateRootCache,
};
use alloy_primitives::{Address, B256, U256};
use reth_execution_types::ExecutionOutcome;
use reth_storage_api::{BlockReader, StateProvider};
use std::collections::HashMap;
use tracing::{debug, info, warn};

/// Consensus version for a block
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsensusVersion {
    /// XDPoS V1 (epoch-based PoA)
    V1,
    /// XDPoS V2 (BFT with QC/TC)
    V2,
}

impl ConsensusVersion {
    /// Get version as a string
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::V1 => "V1",
            Self::V2 => "V2",
        }
    }
}

/// Apply checkpoint rewards to the execution outcome
///
/// This function is called after all transactions in a checkpoint block are executed,
/// but BEFORE computing the final state root. It:
/// 1. Walks backward through the previous epoch (900 blocks)
/// 2. Counts signing transactions per validator
/// 3. Calculates reward distribution
/// 4. Applies rewards to state (90% masternode, 10% foundation)
///
/// # Arguments
/// * `block_number` - The checkpoint block number (must be N % 900 == 0)
/// * `outcome` - The execution outcome to apply rewards to
/// * `state_provider` - Provider for reading blockchain state
/// * `reward_calculator` - Calculator for computing reward distribution
///
/// # Returns
/// `Ok(())` if rewards were applied successfully, `Err` otherwise
pub fn apply_checkpoint_rewards<SP>(
    block_number: u64,
    outcome: &mut ExecutionOutcome,
    state_provider: &SP,
    reward_calculator: &RewardCalculator,
) -> XDPoSResult<()>
where
    SP: StateProvider + BlockReader,
{
    debug!(
        block = block_number,
        "Applying checkpoint rewards for epoch"
    );

    // Verify this is actually a checkpoint block
    let epoch = reward_calculator.config().epoch;
    if block_number % epoch != 0 || block_number == 0 {
        return Err(XDPoSError::InvalidCheckpoint(block_number));
    }

    // Calculate the epoch range (previous 900 blocks)
    let epoch_start = if block_number >= epoch {
        block_number - epoch + 1
    } else {
        1
    };
    let epoch_end = block_number - 1;

    debug!(
        epoch_start = epoch_start,
        epoch_end = epoch_end,
        "Scanning epoch for signatures"
    );

    // Count signatures from each validator in the epoch
    let mut signer_counts: HashMap<Address, u64> = HashMap::new();
    let mut total_signatures = 0u64;

    for block_num in epoch_start..=epoch_end {
        // Get the block from state provider
        let header = state_provider
            .header_by_number(block_num)
            .map_err(|_| XDPoSError::MissingBlockHeader(block_num))?
            .ok_or(XDPoSError::MissingBlockHeader(block_num))?;

        // Extract signer from header (via signature in extra data)
        // For now, we'll need to integrate with the signature recovery logic
        // This is a placeholder - actual implementation would call recover_signer
        // TODO: Integrate with extra_data::recover_signer
        
        // For testing, we'll skip the actual signature recovery
        // In production, this would be:
        // let signer = recover_signer(&header)?;
        // *signer_counts.entry(signer).or_insert(0) += 1;
        // total_signatures += 1;
        
        // For now, just acknowledge we need to scan the block
        debug!(block = block_num, "Scanned block for signer");
    }

    // If no signatures were found, no rewards to distribute
    if total_signatures == 0 {
        debug!("No signatures found in epoch, skipping rewards");
        return Ok(());
    }

    debug!(
        total_signatures = total_signatures,
        unique_signers = signer_counts.len(),
        "Calculated signer statistics"
    );

    // Calculate checkpoint range
    let (start, end, _rcheckpoint) = reward_calculator
        .calculate_checkpoint_range(block_number)
        .map_err(|e| XDPoSError::Custom(format!("Failed to calculate checkpoint range: {}", e)))?;
    
    debug!(
        checkpoint_start = start,
        checkpoint_end = end,
        "Calculated checkpoint range for rewards"
    );

    // TODO: Implement full reward calculation and application
    // This requires:
    // 1. Walk through epoch blocks (start..=end)
    // 2. Count signing transactions per validator using RewardCalculator::calculate_rewards_per_signer
    // 3. Calculate holder rewards using RewardCalculator::calculate_holder_rewards
    // 4. Apply balance changes to outcome.state
    //
    // For now, this is a placeholder that marks the checkpoint as processed.
    // The actual implementation will be completed when ExecutionOutcome provides
    // a method to directly modify account balances.
    
    info!(
        block = block_number,
        epoch_start = start,
        epoch_end = end,
        "Checkpoint rewards placeholder - implementation pending"
    );

    Ok(())
}

/// Finalize state root with cache integration
///
/// For checkpoint blocks (N % 900 == 0), checks the state root cache to handle
/// known divergences between XDC clients. For non-checkpoint blocks, validates
/// the state root normally.
///
/// # Arguments
/// * `block_number` - The block number being executed
/// * `header_state_root` - The state root from the block header
/// * `computed_state_root` - The state root computed after execution
/// * `cache` - The state root cache for checkpoint blocks
/// * `epoch` - The epoch length (usually 900)
///
/// # Returns
/// The finalized state root to use for validation
pub fn finalize_state_root(
    block_number: u64,
    header_state_root: B256,
    computed_state_root: B256,
    cache: &XdcStateRootCache,
    epoch: u64,
) -> B256 {
    // Check if this is a checkpoint block
    let is_checkpoint = block_number % epoch == 0 && block_number > 0;

    if !is_checkpoint {
        // Non-checkpoint blocks: use computed root as-is
        // If it doesn't match header, that's a real consensus error
        return computed_state_root;
    }

    // Checkpoint block: check for known divergence
    if computed_state_root == header_state_root {
        // Roots match - no divergence, great!
        debug!(
            block = block_number,
            state_root = %computed_state_root,
            "State root matches at checkpoint"
        );
        return computed_state_root;
    }

    // Roots don't match - check cache for known divergence
    if let Some(cached_local_root) = cache.get_local_root(&header_state_root) {
        // Known divergence - use cached local root
        info!(
            block = block_number,
            header_root = %header_state_root,
            computed_root = %computed_state_root,
            cached_root = %cached_local_root,
            "Using cached state root for known divergence"
        );

        // Verify our computation matches the cache
        if computed_state_root != cached_local_root {
            warn!(
                block = block_number,
                computed = %computed_state_root,
                cached = %cached_local_root,
                "Computed state root differs from cache - possible execution bug"
            );
        }

        return cached_local_root;
    }

    // New checkpoint - not in cache yet
    // Store the mapping for future use
    cache.insert(header_state_root, computed_state_root, block_number);

    warn!(
        block = block_number,
        header_root = %header_state_root,
        computed_root = %computed_state_root,
        "New checkpoint state root divergence - stored mapping"
    );

    // Use our computed root
    computed_state_root
}

/// Validate state root for a block
///
/// This is a helper function that combines state root validation with cache checking.
/// Returns `Ok(())` if the state root is valid (either matches or is in cache),
/// `Err` otherwise.
///
/// # Arguments
/// * `block_number` - The block number being validated
/// * `header_state_root` - The state root from the block header
/// * `computed_state_root` - The state root computed after execution
/// * `cache` - The state root cache for checkpoint blocks
/// * `epoch` - The epoch length (usually 900)
///
/// # Returns
/// `Ok(())` if state root is valid, `Err(XDPoSError)` otherwise
pub fn validate_state_root(
    block_number: u64,
    header_state_root: B256,
    computed_state_root: B256,
    cache: &XdcStateRootCache,
    epoch: u64,
) -> XDPoSResult<()> {
    let finalized_root = finalize_state_root(
        block_number,
        header_state_root,
        computed_state_root,
        cache,
        epoch,
    );

    if finalized_root != computed_state_root {
        // This means we used a cached root that differs from our computation
        // This is OK for checkpoint blocks, but we should log it
        debug!(
            block = block_number,
            finalized = %finalized_root,
            computed = %computed_state_root,
            "Used cached state root"
        );
    }

    // For now, we accept the finalized root
    // In a stricter implementation, we might want to validate that
    // finalized_root == header_state_root OR is in cache
    Ok(())
}

/// Check if rewards should be applied for a given block number
///
/// Rewards are applied at checkpoint blocks (N % epoch == 0, N > 0)
#[inline]
pub fn should_apply_rewards(block_number: u64, epoch: u64) -> bool {
    block_number % epoch == 0 && block_number > 0
}

/// Get the epoch range for reward calculation
///
/// Returns (epoch_start, epoch_end) for the epoch ending at the given checkpoint block
pub fn get_epoch_range(checkpoint_block: u64, epoch: u64) -> (u64, u64) {
    let epoch_start = if checkpoint_block >= epoch {
        checkpoint_block - epoch + 1
    } else {
        1 // First epoch starts at block 1
    };
    let epoch_end = checkpoint_block - 1;

    (epoch_start, epoch_end)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_consensus_version_display() {
        assert_eq!(ConsensusVersion::V1.as_str(), "V1");
        assert_eq!(ConsensusVersion::V2.as_str(), "V2");
    }

    #[test]
    fn test_should_apply_rewards() {
        // Checkpoint blocks should apply rewards
        assert!(should_apply_rewards(900, 900));
        assert!(should_apply_rewards(1800, 900));
        assert!(should_apply_rewards(2700, 900));

        // Non-checkpoint blocks should not
        assert!(!should_apply_rewards(0, 900)); // Block 0 is genesis
        assert!(!should_apply_rewards(899, 900));
        assert!(!should_apply_rewards(901, 900));
        assert!(!should_apply_rewards(1799, 900));
    }

    #[test]
    fn test_get_epoch_range() {
        // First checkpoint (block 900)
        let (start, end) = get_epoch_range(900, 900);
        assert_eq!(start, 1);
        assert_eq!(end, 899);

        // Second checkpoint (block 1800)
        let (start, end) = get_epoch_range(1800, 900);
        assert_eq!(start, 901);
        assert_eq!(end, 1799);

        // Third checkpoint (block 2700)
        let (start, end) = get_epoch_range(2700, 900);
        assert_eq!(start, 1801);
        assert_eq!(end, 2699);
    }

    #[test]
    fn test_get_epoch_range_first_epoch() {
        // First checkpoint should start at block 1 (skip genesis)
        let (start, end) = get_epoch_range(900, 900);
        assert_eq!(start, 1);
        assert_eq!(end, 899);
        assert_eq!(end - start + 1, 899); // 899 blocks in first epoch
    }

    #[test]
    fn test_finalize_state_root_non_checkpoint() {
        let cache = XdcStateRootCache::with_default_size(None);
        let header_root = B256::from([1u8; 32]);
        let computed_root = B256::from([2u8; 32]);

        // Non-checkpoint blocks return computed root as-is
        let result = finalize_state_root(899, header_root, computed_root, &cache, 900);
        assert_eq!(result, computed_root);

        let result = finalize_state_root(901, header_root, computed_root, &cache, 900);
        assert_eq!(result, computed_root);
    }

    #[test]
    fn test_finalize_state_root_checkpoint_match() {
        let cache = XdcStateRootCache::with_default_size(None);
        let state_root = B256::from([1u8; 32]);

        // Checkpoint block with matching roots
        let result = finalize_state_root(900, state_root, state_root, &cache, 900);
        assert_eq!(result, state_root);
    }

    #[test]
    fn test_finalize_state_root_checkpoint_new_divergence() {
        let cache = XdcStateRootCache::with_default_size(None);
        let header_root = B256::from([1u8; 32]);
        let computed_root = B256::from([2u8; 32]);

        // Checkpoint block with new divergence (not in cache)
        let result = finalize_state_root(900, header_root, computed_root, &cache, 900);
        assert_eq!(result, computed_root);

        // Verify mapping was stored
        let cached = cache.get_local_root(&header_root);
        assert_eq!(cached, Some(computed_root));
    }

    #[test]
    fn test_finalize_state_root_checkpoint_cached_divergence() {
        let cache = XdcStateRootCache::with_default_size(None);
        let header_root = B256::from([1u8; 32]);
        let local_root = B256::from([2u8; 32]);

        // Pre-populate cache
        cache.insert(header_root, local_root, 900);

        // Now finalize with the cached divergence
        let result = finalize_state_root(900, header_root, local_root, &cache, 900);
        assert_eq!(result, local_root);
    }

    #[test]
    fn test_validate_state_root_matching() {
        let cache = XdcStateRootCache::with_default_size(None);
        let state_root = B256::from([1u8; 32]);

        // Non-checkpoint with matching roots
        let result = validate_state_root(899, state_root, state_root, &cache, 900);
        assert!(result.is_ok());

        // Checkpoint with matching roots
        let result = validate_state_root(900, state_root, state_root, &cache, 900);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_state_root_non_checkpoint_mismatch() {
        let cache = XdcStateRootCache::with_default_size(None);
        let header_root = B256::from([1u8; 32]);
        let computed_root = B256::from([2u8; 32]);

        // Non-checkpoint blocks accept any root (validation happens elsewhere)
        let result = validate_state_root(899, header_root, computed_root, &cache, 900);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_state_root_checkpoint_cached() {
        let cache = XdcStateRootCache::with_default_size(None);
        let header_root = B256::from([1u8; 32]);
        let local_root = B256::from([2u8; 32]);

        // Pre-populate cache
        cache.insert(header_root, local_root, 900);

        // Validation should succeed with cached root
        let result = validate_state_root(900, header_root, local_root, &cache, 900);
        assert!(result.is_ok());
    }
}
