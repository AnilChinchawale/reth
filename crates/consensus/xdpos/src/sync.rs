//! XDC Sync Coordinator
//!
//! This module provides XDC-specific sync coordination for Reth's execution pipeline.
//! It wraps the standard block executor with XDC consensus logic including:
//! - Checkpoint reward application
//! - State root cache integration
//! - Special transaction handling
//! - V1/V2 consensus version switching

use crate::{
    config::XDPoSConfig,
    errors::XDPoSResult,
    execution::{apply_checkpoint_rewards, finalize_state_root, ConsensusVersion},
    reward::RewardCalculator,
    special_tx::is_free_gas_tx,
    state_root_cache::XdcStateRootCache,
};
use alloy_primitives::{Address, B256, U256};
use reth_execution_types::ExecutionOutcome;
use reth_primitives_traits::{Block, SignedTransaction};
use reth_storage_api::{BlockReader, StateProvider};
use std::sync::Arc;

/// XDC sync configuration
#[derive(Debug, Clone)]
pub struct XdcSyncConfig {
    /// XDPoS consensus configuration
    pub xdpos_config: XDPoSConfig,
    /// State root cache for checkpoint blocks
    pub state_root_cache: XdcStateRootCache,
    /// Chain ID (50 = mainnet, 51 = apothem)
    pub chain_id: u64,
}

impl XdcSyncConfig {
    /// Create a new XDC sync configuration
    pub fn new(
        xdpos_config: XDPoSConfig,
        state_root_cache: XdcStateRootCache,
        chain_id: u64,
    ) -> Self {
        Self {
            xdpos_config,
            state_root_cache,
            chain_id,
        }
    }

    /// Create sync config for XDC mainnet
    pub fn mainnet(cache_path: Option<std::path::PathBuf>) -> Self {
        let cache = XdcStateRootCache::with_default_size(cache_path);

        Self {
            xdpos_config: crate::config::xdc_mainnet_config(),
            state_root_cache: cache,
            chain_id: 50,
        }
    }

    /// Create sync config for XDC apothem testnet
    pub fn apothem(cache_path: Option<std::path::PathBuf>) -> Self {
        let cache = XdcStateRootCache::with_default_size(cache_path);

        Self {
            xdpos_config: crate::config::xdc_apothem_config(),
            state_root_cache: cache,
            chain_id: 51,
        }
    }

    /// Check if this is an XDC chain (mainnet or apothem)
    pub fn is_xdc_chain(&self) -> bool {
        self.chain_id == 50 || self.chain_id == 51
    }

    /// Get the consensus version for a given block number
    pub fn consensus_version(&self, block_number: u64) -> ConsensusVersion {
        if self.xdpos_config.is_v2(block_number) {
            ConsensusVersion::V2
        } else {
            ConsensusVersion::V1
        }
    }

    /// Check if state root cache should be used for this block
    pub fn should_use_cache(&self, block_number: u64) -> bool {
        self.is_xdc_chain() && is_checkpoint_block(block_number, self.xdpos_config.epoch)
    }
}

/// Check if a block is a checkpoint block (epoch boundary)
#[inline]
pub fn is_checkpoint_block(block_number: u64, epoch: u64) -> bool {
    block_number % epoch == 0 && block_number > 0
}

/// XDC block executor wrapper
///
/// Wraps a standard Reth executor with XDC-specific logic for:
/// - Checkpoint reward application
/// - State root validation with cache
/// - Special transaction gas handling
/// - Consensus version switching
pub struct XdcBlockExecutor<SP> {
    /// State provider for reading blockchain state
    state_provider: Arc<SP>,
    /// XDC sync configuration
    config: Arc<XdcSyncConfig>,
    /// Reward calculator for checkpoint blocks
    reward_calculator: RewardCalculator,
}

impl<SP> XdcBlockExecutor<SP>
where
    SP: StateProvider + BlockReader,
{
    /// Create a new XDC block executor
    pub fn new(state_provider: Arc<SP>, config: Arc<XdcSyncConfig>) -> Self {
        let reward_calculator = RewardCalculator::new(config.xdpos_config.clone());

        Self {
            state_provider,
            config,
            reward_calculator,
        }
    }

    /// Get the sync configuration
    pub fn config(&self) -> &XdcSyncConfig {
        &self.config
    }

    /// Get the reward calculator
    pub fn reward_calculator(&self) -> &RewardCalculator {
        &self.reward_calculator
    }

    /// Pre-execution hook: Determine consensus version
    pub fn pre_execute(&self, block_number: u64) -> ConsensusVersion {
        self.config.consensus_version(block_number)
    }

    /// Check if a transaction should have free gas
    pub fn is_free_gas_transaction<T: SignedTransaction>(
        &self,
        block_number: u64,
        transaction: &T,
    ) -> bool {
        is_free_gas_tx(block_number, transaction.to())
    }

    /// Calculate effective gas price for a transaction
    ///
    /// Returns 0 for free gas transactions (TIPSigning), otherwise returns the transaction's gas price
    pub fn effective_gas_price<T: SignedTransaction>(
        &self,
        block_number: u64,
        transaction: &T,
    ) -> u128 {
        if self.is_free_gas_transaction(block_number, transaction) {
            0
        } else {
            transaction.max_fee_per_gas()
        }
    }

    /// Post-execution hook: Apply checkpoint rewards if needed
    ///
    /// This should be called after all transactions in a block are executed,
    /// but BEFORE computing the final state root.
    pub fn post_execute(
        &self,
        block_number: u64,
        outcome: &mut ExecutionOutcome,
    ) -> XDPoSResult<()> {
        if is_checkpoint_block(block_number, self.config.xdpos_config.epoch) {
            // Apply checkpoint rewards to state
            apply_checkpoint_rewards(
                block_number,
                outcome,
                &*self.state_provider,
                &self.reward_calculator,
            )?;
        }
        Ok(())
    }

    /// Finalize state root with cache integration
    ///
    /// For checkpoint blocks, checks the state root cache to handle known divergences
    /// between XDC clients. For non-checkpoint blocks, validates state root normally.
    pub fn finalize_state_root(
        &self,
        block_number: u64,
        header_state_root: B256,
        computed_state_root: B256,
    ) -> XDPoSResult<B256> {
        Ok(finalize_state_root(
            block_number,
            header_state_root,
            computed_state_root,
            &self.config.state_root_cache,
            self.config.xdpos_config.epoch,
        ))
    }

    /// Check if rewards should be applied for this block
    pub fn should_apply_rewards(&self, block_number: u64) -> bool {
        is_checkpoint_block(block_number, self.config.xdpos_config.epoch)
    }

    /// Get the foundation wallet address for reward distribution
    pub fn foundation_wallet(&self) -> Address {
        self.config.xdpos_config.foundation_wallet
    }
}

/// Sync mode for XDC
///
/// XDC only supports full sync because v2.6.8 peers don't support snap protocol
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XdcSyncMode {
    /// Full sync: download all headers, bodies, and execute all blocks
    Full,
}

impl Default for XdcSyncMode {
    fn default() -> Self {
        Self::Full
    }
}

impl XdcSyncMode {
    /// Check if this sync mode is full sync
    pub fn is_full(&self) -> bool {
        matches!(self, Self::Full)
    }

    /// Get the sync mode as a string
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Full => "full",
        }
    }
}

/// Sync statistics for monitoring
#[derive(Debug, Default, Clone)]
pub struct XdcSyncStats {
    /// Total blocks synced
    pub blocks_synced: u64,
    /// Checkpoint blocks processed
    pub checkpoints_processed: u64,
    /// State root cache hits
    pub cache_hits: u64,
    /// State root cache misses
    pub cache_misses: u64,
    /// Total rewards applied (in wei)
    pub total_rewards_applied: U256,
    /// V1 blocks processed
    pub v1_blocks: u64,
    /// V2 blocks processed
    pub v2_blocks: u64,
}

impl XdcSyncStats {
    /// Create new sync statistics
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a synced block
    pub fn record_block(&mut self, block_number: u64, version: ConsensusVersion, epoch: u64) {
        self.blocks_synced += 1;

        if is_checkpoint_block(block_number, epoch) {
            self.checkpoints_processed += 1;
        }

        match version {
            ConsensusVersion::V1 => self.v1_blocks += 1,
            ConsensusVersion::V2 => self.v2_blocks += 1,
        }
    }

    /// Record a state root cache hit
    pub fn record_cache_hit(&mut self) {
        self.cache_hits += 1;
    }

    /// Record a state root cache miss
    pub fn record_cache_miss(&mut self) {
        self.cache_misses += 1;
    }

    /// Record applied rewards
    pub fn record_rewards(&mut self, amount: U256) {
        self.total_rewards_applied += amount;
    }

    /// Get cache hit rate (0.0 to 1.0)
    pub fn cache_hit_rate(&self) -> f64 {
        let total = self.cache_hits + self.cache_misses;
        if total == 0 {
            0.0
        } else {
            self.cache_hits as f64 / total as f64
        }
    }

    /// Get average rewards per checkpoint
    pub fn avg_rewards_per_checkpoint(&self) -> U256 {
        if self.checkpoints_processed == 0 {
            U256::ZERO
        } else {
            self.total_rewards_applied / U256::from(self.checkpoints_processed)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::address;

    #[test]
    fn test_is_checkpoint_block() {
        assert!(!is_checkpoint_block(0, 900)); // Block 0 is not a checkpoint
        assert!(!is_checkpoint_block(899, 900));
        assert!(is_checkpoint_block(900, 900));
        assert!(is_checkpoint_block(1800, 900));
        assert!(is_checkpoint_block(2700, 900));
        assert!(!is_checkpoint_block(901, 900));
    }

    #[test]
    fn test_xdc_sync_config_mainnet() {
        let config = XdcSyncConfig::mainnet(None);
        assert_eq!(config.chain_id, 50);
        assert!(config.is_xdc_chain());
        assert_eq!(config.xdpos_config.epoch, 900);
    }

    #[test]
    fn test_xdc_sync_config_apothem() {
        let config = XdcSyncConfig::apothem(None);
        assert_eq!(config.chain_id, 51);
        assert!(config.is_xdc_chain());
        assert_eq!(config.xdpos_config.epoch, 900);
    }

    #[test]
    fn test_consensus_version() {
        let config = XdcSyncConfig::mainnet(None);

        // Before V2 switch (56,857,600)
        assert_eq!(
            config.consensus_version(56_857_599),
            ConsensusVersion::V1
        );

        // At V2 switch
        assert_eq!(
            config.consensus_version(56_857_600),
            ConsensusVersion::V2
        );

        // After V2 switch
        assert_eq!(
            config.consensus_version(56_857_601),
            ConsensusVersion::V2
        );
    }

    #[test]
    fn test_should_use_cache() {
        let config = XdcSyncConfig::mainnet(None);

        // Checkpoint blocks should use cache
        assert!(config.should_use_cache(900));
        assert!(config.should_use_cache(1800));
        assert!(config.should_use_cache(2700));

        // Non-checkpoint blocks should not
        assert!(!config.should_use_cache(899));
        assert!(!config.should_use_cache(901));

        // Block 0 is not a checkpoint
        assert!(!config.should_use_cache(0));
    }

    #[test]
    fn test_xdc_sync_mode() {
        let mode = XdcSyncMode::default();
        assert_eq!(mode, XdcSyncMode::Full);
        assert!(mode.is_full());
        assert_eq!(mode.as_str(), "full");
    }

    #[test]
    fn test_sync_stats() {
        let mut stats = XdcSyncStats::new();

        // Record some blocks
        stats.record_block(899, ConsensusVersion::V1, 900);
        stats.record_block(900, ConsensusVersion::V1, 900); // Checkpoint
        stats.record_block(901, ConsensusVersion::V1, 900);

        assert_eq!(stats.blocks_synced, 3);
        assert_eq!(stats.checkpoints_processed, 1);
        assert_eq!(stats.v1_blocks, 3);
        assert_eq!(stats.v2_blocks, 0);

        // Record cache stats
        stats.record_cache_hit();
        stats.record_cache_hit();
        stats.record_cache_miss();

        assert_eq!(stats.cache_hits, 2);
        assert_eq!(stats.cache_misses, 1);
        assert_eq!(stats.cache_hit_rate(), 2.0 / 3.0);

        // Record rewards
        let reward = U256::from(250_000_000_000_000_000_000u128); // 250 XDC
        stats.record_rewards(reward);
        assert_eq!(stats.total_rewards_applied, reward);
        assert_eq!(stats.avg_rewards_per_checkpoint(), reward);
    }

    #[test]
    fn test_cache_hit_rate_with_no_data() {
        let stats = XdcSyncStats::new();
        assert_eq!(stats.cache_hit_rate(), 0.0);
    }

    #[test]
    fn test_avg_rewards_with_no_checkpoints() {
        let stats = XdcSyncStats::new();
        assert_eq!(stats.avg_rewards_per_checkpoint(), U256::ZERO);
    }
}
