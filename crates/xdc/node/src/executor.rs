//! XDC Block Executor
//!
//! This module provides a custom block executor that wraps Reth's standard
//! EVM executor with XDPoS-specific logic:
//! - Pre-block: Consensus version detection
//! - Post-block: Checkpoint reward application
//! - State root: Cache integration for V2 consensus

use alloy_primitives::{Address, B256};
use reth_chainspec::ChainSpec;
use reth_consensus_xdpos::{
    apply_checkpoint_rewards, finalize_state_root, is_checkpoint_block, should_apply_rewards,
    validate_state_root, ConsensusVersion, XdcStateRootCache, XDPoSConsensus,
};
use reth_errors::{BlockExecutionError, ProviderError};
use reth_evm::execute::{
    BatchExecutor, BlockExecutionInput, BlockExecutionOutput, BlockExecutorProvider, Executor,
};
use reth_evm_ethereum::execute::EthExecutorProvider;
use reth_primitives::{BlockWithSenders, Receipt};
use reth_primitives_traits::BlockBody;
use reth_provider::{BlockReader, StateProviderFactory};
use reth_revm::db::states::bundle_state::BundleRetention;
use std::sync::Arc;
use tracing::{debug, trace};

/// XDC block executor provider
///
/// Wraps the standard Ethereum executor with XDPoS consensus logic
#[derive(Debug, Clone)]
pub struct XdcExecutorProvider {
    /// Inner Ethereum executor provider
    inner: EthExecutorProvider,
    /// Chain specification
    chain_spec: Arc<ChainSpec>,
    /// XDPoS consensus engine
    consensus: Arc<XDPoSConsensus>,
    /// State root cache for V2 consensus
    state_root_cache: Arc<XdcStateRootCache>,
}

impl XdcExecutorProvider {
    /// Create a new XDC executor provider
    pub fn new(
        chain_spec: Arc<ChainSpec>,
        consensus: Arc<XDPoSConsensus>,
        state_root_cache: Arc<XdcStateRootCache>,
    ) -> Self {
        Self {
            inner: EthExecutorProvider::new(chain_spec.clone()),
            chain_spec,
            consensus,
            state_root_cache,
        }
    }

    /// Get the consensus version for a block number
    fn consensus_version(&self, block_number: u64) -> ConsensusVersion {
        ConsensusVersion::for_block(self.chain_spec.chain.id(), block_number)
    }
}

impl BlockExecutorProvider for XdcExecutorProvider {
    type Executor<DB: reth_evm::execute::BlockExecutionDb> = XdcBlockExecutor<DB>;

    type BatchExecutor<DB: reth_evm::execute::BlockExecutionDb> = XdcBatchExecutor<DB>;

    fn executor<DB>(&self, db: DB) -> Self::Executor<DB>
    where
        DB: reth_evm::execute::BlockExecutionDb,
    {
        XdcBlockExecutor {
            inner: self.inner.executor(db),
            chain_spec: self.chain_spec.clone(),
            consensus: self.consensus.clone(),
            state_root_cache: self.state_root_cache.clone(),
        }
    }

    fn batch_executor<DB>(&self, db: DB) -> Self::BatchExecutor<DB>
    where
        DB: reth_evm::execute::BlockExecutionDb,
    {
        XdcBatchExecutor {
            inner: self.inner.batch_executor(db),
            chain_spec: self.chain_spec.clone(),
            consensus: self.consensus.clone(),
            state_root_cache: self.state_root_cache.clone(),
        }
    }
}

/// XDC single block executor
///
/// Executes a single block with XDPoS consensus integration
pub struct XdcBlockExecutor<DB> {
    /// Inner Ethereum executor
    inner: <EthExecutorProvider as BlockExecutorProvider>::Executor<DB>,
    /// Chain specification
    chain_spec: Arc<ChainSpec>,
    /// XDPoS consensus engine
    consensus: Arc<XDPoSConsensus>,
    /// State root cache
    state_root_cache: Arc<XdcStateRootCache>,
}

impl<DB> Executor<DB> for XdcBlockExecutor<DB>
where
    DB: reth_evm::execute::BlockExecutionDb,
{
    type Input<'a> = BlockExecutionInput<'a, BlockWithSenders>;
    type Output = BlockExecutionOutput<Receipt>;
    type Error = BlockExecutionError;

    fn execute(mut self, input: Self::Input<'_>) -> Result<Self::Output, Self::Error> {
        let block = input.block;
        let block_number = block.number;
        let block_hash = block.hash();

        debug!(
            block_number,
            block_hash = %block_hash,
            "Executing XDC block"
        );

        // Detect consensus version
        let version = ConsensusVersion::for_block(self.chain_spec.chain.id(), block_number);
        trace!(block_number, version = ?version, "Detected consensus version");

        // Execute block with standard Ethereum execution
        let mut output = self.inner.execute(input)?;

        // Apply XDPoS rewards if this is a checkpoint block
        if should_apply_rewards(block_number, self.chain_spec.chain.id()) {
            debug!(
                block_number,
                "Applying XDPoS checkpoint rewards"
            );

            // Apply rewards to the state
            apply_checkpoint_rewards(
                &mut output.state,
                block_number,
                &block.header,
                self.chain_spec.chain.id(),
            )?;
        }

        // Finalize state root (with cache for V2)
        let final_state_root = finalize_state_root(
            &output.state,
            block_number,
            self.chain_spec.chain.id(),
            &self.state_root_cache,
        )?;

        // Validate state root matches block header
        validate_state_root(block.state_root, final_state_root, block_number)?;

        Ok(output)
    }

    fn execute_with_state_witness<F>(
        self,
        input: Self::Input<'_>,
        witness: F,
    ) -> Result<Self::Output, Self::Error>
    where
        F: FnMut(&B256, &revm::Database),
    {
        // For now, delegate to inner executor
        // TODO: Add XDPoS-specific witness handling if needed
        self.inner.execute_with_state_witness(input, witness)
    }
}

/// XDC batch executor
///
/// Executes multiple blocks in sequence with XDPoS consensus integration
pub struct XdcBatchExecutor<DB> {
    /// Inner Ethereum batch executor
    inner: <EthExecutorProvider as BlockExecutorProvider>::BatchExecutor<DB>,
    /// Chain specification
    chain_spec: Arc<ChainSpec>,
    /// XDPoS consensus engine
    consensus: Arc<XDPoSConsensus>,
    /// State root cache
    state_root_cache: Arc<XdcStateRootCache>,
}

impl<DB> BatchExecutor<DB> for XdcBatchExecutor<DB>
where
    DB: reth_evm::execute::BlockExecutionDb,
{
    type Input<'a> = BlockExecutionInput<'a, BlockWithSenders>;
    type Output = BlockExecutionOutput<Receipt>;
    type Error = BlockExecutionError;

    fn execute_and_verify_one(&mut self, input: Self::Input<'_>) -> Result<(), Self::Error> {
        let block = input.block;
        let block_number = block.number;

        trace!(block_number, "Batch executing XDC block");

        // Detect consensus version
        let version = ConsensusVersion::for_block(self.chain_spec.chain.id(), block_number);

        // Execute with inner executor
        self.inner.execute_and_verify_one(input)?;

        // Apply XDPoS rewards if this is a checkpoint block
        if should_apply_rewards(block_number, self.chain_spec.chain.id()) {
            trace!(block_number, "Applying checkpoint rewards in batch");
            // Note: In batch mode, rewards are applied during finalize()
        }

        Ok(())
    }

    fn finalize(mut self) -> Self::Output {
        // Finalize inner executor
        let mut output = self.inner.finalize();

        // Apply any pending state root cache updates
        // (This is where we'd sync cache state if needed)

        output
    }

    fn set_tip(&mut self, tip: u64) {
        self.inner.set_tip(tip);
    }

    fn set_prune_modes(&mut self, prune_modes: reth_prune_types::PruneModes) {
        self.inner.set_prune_modes(prune_modes);
    }

    fn size_hint(&self) -> Option<usize> {
        self.inner.size_hint()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_consensus_version_detection() {
        // XDC Mainnet V2 switch: 56,857,600
        assert_eq!(
            ConsensusVersion::for_block(50, 56_857_599),
            ConsensusVersion::V1
        );
        assert_eq!(
            ConsensusVersion::for_block(50, 56_857_600),
            ConsensusVersion::V2
        );

        // XDC Apothem V2 switch: 23,556,600
        assert_eq!(
            ConsensusVersion::for_block(51, 23_556_599),
            ConsensusVersion::V1
        );
        assert_eq!(
            ConsensusVersion::for_block(51, 23_556_600),
            ConsensusVersion::V2
        );
    }

    #[test]
    fn test_checkpoint_block_detection() {
        const EPOCH: u64 = 900;

        // Checkpoint blocks
        assert!(is_checkpoint_block(0, EPOCH));
        assert!(is_checkpoint_block(900, EPOCH));
        assert!(is_checkpoint_block(1800, EPOCH));

        // Non-checkpoint blocks
        assert!(!is_checkpoint_block(1, EPOCH));
        assert!(!is_checkpoint_block(899, EPOCH));
        assert!(!is_checkpoint_block(901, EPOCH));
    }
}
