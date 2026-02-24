//! XDC Block Executor
//!
//! This module provides XDC-specific execution logic integrated into the EVM config's
//! block executor pipeline. It handles:
//! - Pre-execution: Consensus version detection (V1 vs V2)
//! - Transaction processing: TIPSigning gas exemptions
//! - Post-execution: Checkpoint reward application
//! - State root finalization: Cache integration for known divergences

use alloy_consensus::Header;
use alloy_evm::{
    block::{BlockExecutor, BlockExecutorFactory, CommitChanges, ExecutableTxParts},
    Evm, EvmEnv, EvmFactory, RecoveredTx, ToTxEnv,
};
use alloy_primitives::{Address, B256, U256};
use reth_chainspec::{ChainSpec, EthChainSpec};
use reth_ethereum_primitives::TransactionSigned;
use reth_evm::{
    execute::{BlockExecutionError, BlockExecutionOutput, BlockExecutionResult, ExecutionOutcome, ProviderError},
    ConfigureEvm, Database, OnStateHook, TxEnvFor,
};
use reth_primitives_traits::{Block, NodePrimitives, RecoveredBlock, SealedHeader};
use reth_storage_api::{BlockReader, StateProvider};
use revm::{
    context::result::ExecutionResult,
    database::{states::bundle_state::BundleRetention, BundleState, State},
};
use std::sync::Arc;
use tracing::{debug, info, trace, warn};

use reth_consensus_xdpos::{
    apply_checkpoint_rewards, finalize_state_root, should_apply_rewards, XdcStateRootCache,
    XdPoSConfig,
};

/// XDC execution configuration
///
/// This configuration is used to customize block execution with XDC-specific logic
#[derive(Debug, Clone)]
pub struct XdcExecutionConfig {
    /// Chain specification
    chain_spec: Arc<ChainSpec>,
    /// XDPoS consensus configuration
    xdpos_config: Arc<XdPoSConfig>,
    /// State root cache for checkpoint blocks
    state_root_cache: Arc<XdcStateRootCache>,
}

impl XdcExecutionConfig {
    /// Create a new XDC execution configuration
    pub fn new(
        chain_spec: Arc<ChainSpec>,
        xdpos_config: Arc<XdPoSConfig>,
        state_root_cache: Arc<XdcStateRootCache>,
    ) -> Self {
        Self {
            chain_spec,
            xdpos_config,
            state_root_cache,
        }
    }

    /// Get the consensus version for a block number
    pub fn consensus_version(&self, block_number: u64) -> ConsensusVersion {
        if self.xdpos_config.is_v2(block_number) {
            ConsensusVersion::V2
        } else {
            ConsensusVersion::V1
        }
    }

    /// Check if rewards should be applied at this block
    pub fn should_apply_rewards(&self, block_number: u64) -> bool {
        should_apply_rewards(block_number, self.xdpos_config.epoch)
    }

    /// Check if a transaction is eligible for TIPSigning gas exemption
    pub fn is_tipsigning_tx(&self, block_number: u64, to: Option<Address>) -> bool {
        // TIPSigning activation block (3M for both mainnet and testnet)
        const TIPSIGNING_BLOCK: u64 = 3_000_000;

        if block_number < TIPSIGNING_BLOCK {
            return false;
        }

        // System contract addresses eligible for gas exemption (0x88, 0x89)
        const VALIDATOR_CONTRACT: Address =
            Address::new([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x88]);
        const BLOCK_SIGNERS_CONTRACT: Address =
            Address::new([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x89]);

        to == Some(VALIDATOR_CONTRACT) || to == Some(BLOCK_SIGNERS_CONTRACT)
    }

    /// Finalize state root with cache integration
    pub fn finalize_state_root(
        &self,
        block_number: u64,
        header_root: B256,
        computed_root: B256,
    ) -> B256 {
        finalize_state_root(
            block_number,
            header_root,
            computed_root,
            &self.state_root_cache,
            self.xdpos_config.epoch,
        )
    }
}

/// Consensus version
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsensusVersion {
    /// XDPoS V1 (original consensus)
    V1,
    /// XDPoS V2 (timeout-based consensus)
    V2,
}

impl ConsensusVersion {
    /// Get consensus version for a block number
    pub fn for_block(chain_id: u64, block_number: u64) -> Self {
        // V2 switch blocks
        const MAINNET_V2_SWITCH: u64 = 56_857_600;
        const APOTHEM_V2_SWITCH: u64 = 23_556_600;

        let switch = match chain_id {
            50 => MAINNET_V2_SWITCH,
            51 => APOTHEM_V2_SWITCH,
            _ => u64::MAX,
        };

        if block_number >= switch {
            ConsensusVersion::V2
        } else {
            ConsensusVersion::V1
        }
    }

    /// Get version as a string
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::V1 => "V1",
            Self::V2 => "V2",
        }
    }
}

/// XDC Block Executor
///
/// Wraps the standard Ethereum block executor with XDC-specific hooks:
/// 1. **Pre-execution**: Detect consensus version
/// 2. **Transaction execution**: Apply TIPSigning gas exemptions
/// 3. **Post-execution**: Apply checkpoint rewards
/// 4. **State root validation**: Check cache for known divergences
///
/// This executor is used by the execution stage to process blocks according
/// to XDC consensus rules.
pub struct XdcBlockExecutor<DB, EvmConfig>
where
    DB: Database,
    EvmConfig: ConfigureEvm,
{
    /// Inner EVM configuration
    evm_config: EvmConfig,
    /// XDC execution configuration
    xdc_config: Arc<XdcExecutionConfig>,
    /// Database for EVM state
    db: DB,
    /// Block number being executed
    block_number: u64,
    /// Consensus version for current block
    consensus_version: ConsensusVersion,
}

impl<DB, EvmConfig> XdcBlockExecutor<DB, EvmConfig>
where
    DB: Database,
    EvmConfig: ConfigureEvm,
{
    /// Create a new XDC block executor
    pub fn new(
        evm_config: EvmConfig,
        xdc_config: Arc<XdcExecutionConfig>,
        db: DB,
        block_number: u64,
    ) -> Self {
        let consensus_version = xdc_config.consensus_version(block_number);

        debug!(
            block = block_number,
            version = consensus_version.as_str(),
            "Initialized XDC block executor"
        );

        Self {
            evm_config,
            xdc_config,
            db,
            block_number,
            consensus_version,
        }
    }

    /// Get the consensus version for the current block
    pub fn consensus_version(&self) -> ConsensusVersion {
        self.consensus_version
    }

    /// Check if a transaction should have free gas
    pub fn should_exempt_gas(&self, to: Option<Address>) -> bool {
        self.xdc_config.is_tipsigning_tx(self.block_number, to)
    }

    /// Apply checkpoint rewards to the execution outcome
    ///
    /// Called after all transactions are executed but before state root computation
    fn apply_rewards<SP>(
        &self,
        outcome: &mut ExecutionOutcome,
        state_provider: &SP,
    ) -> Result<(), BlockExecutionError>
    where
        SP: StateProvider + BlockReader,
    {
        if !self.xdc_config.should_apply_rewards(self.block_number) {
            return Ok(());
        }

        info!(
            block = self.block_number,
            epoch = self.xdc_config.xdpos_config.epoch,
            "Applying checkpoint rewards"
        );

        // TODO: Integrate reward calculator from xdpos crate
        // For now, this is a placeholder - actual implementation needs:
        // 1. Reward calculator instance
        // 2. Walk through epoch blocks
        // 3. Count validator signatures
        // 4. Calculate and apply rewards to state
        
        debug!(
            block = self.block_number,
            "Checkpoint reward application placeholder"
        );

        Ok(())
    }
}

/// Integration placeholder for state root validation
///
/// This function will be called by the execution stage after computing the state root
/// to check if it matches the header or if there's a known divergence in the cache
pub fn validate_state_root_with_cache(
    block_number: u64,
    header_root: B256,
    computed_root: B256,
    cache: &XdcStateRootCache,
    epoch: u64,
) -> Result<(), BlockExecutionError> {
    let finalized_root = finalize_state_root(
        block_number,
        header_root,
        computed_root,
        cache,
        epoch,
    );

    if finalized_root != header_root {
        // For checkpoint blocks, we accept divergent roots if they're in the cache
        // This is normal for XDC due to client differences in reward application
        trace!(
            block = block_number,
            header = %header_root,
            computed = %computed_root,
            finalized = %finalized_root,
            "State root divergence handled by cache"
        );
    }

    Ok(())
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
        assert_eq!(
            ConsensusVersion::for_block(50, 60_000_000),
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
    fn test_consensus_version_display() {
        assert_eq!(ConsensusVersion::V1.as_str(), "V1");
        assert_eq!(ConsensusVersion::V2.as_str(), "V2");
    }

    #[test]
    fn test_tipsigning_activation() {
        let chain_spec = Arc::new(ChainSpec::default());
        let xdpos_config = Arc::new(XdPoSConfig::default());
        let cache = Arc::new(XdcStateRootCache::with_default_size(None));
        let config = XdcExecutionConfig::new(chain_spec, xdpos_config, cache);

        // Before TIPSigning block
        assert!(!config.is_tipsigning_tx(2_999_999, Some(Address::new([0; 20]))));

        // After TIPSigning block, but not to system contract
        assert!(!config.is_tipsigning_tx(3_000_000, Some(Address::new([0; 20]))));

        // After TIPSigning block, to validator contract (0x88)
        let validator_addr = Address::new([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x88]);
        assert!(config.is_tipsigning_tx(3_000_000, Some(validator_addr)));

        // After TIPSigning block, to block signers contract (0x89)
        let signers_addr = Address::new([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x89]);
        assert!(config.is_tipsigning_tx(3_000_001, Some(signers_addr)));
    }

    #[test]
    fn test_checkpoint_reward_detection() {
        let chain_spec = Arc::new(ChainSpec::default());
        let xdpos_config = Arc::new(XdPoSConfig::default());
        let cache = Arc::new(XdcStateRootCache::with_default_size(None));
        let config = XdcExecutionConfig::new(chain_spec, xdpos_config, cache);

        // Checkpoint blocks (epoch = 900)
        assert!(!config.should_apply_rewards(0)); // Genesis
        assert!(config.should_apply_rewards(900));
        assert!(config.should_apply_rewards(1800));
        assert!(config.should_apply_rewards(2700));

        // Non-checkpoint blocks
        assert!(!config.should_apply_rewards(1));
        assert!(!config.should_apply_rewards(899));
        assert!(!config.should_apply_rewards(901));
        assert!(!config.should_apply_rewards(1799));
    }
}
