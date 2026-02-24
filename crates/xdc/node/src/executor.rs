//! XDC Block Executor
//!
//! This module provides XDC-specific execution logic that will be integrated
//! with the EVM config's block executor factory.
//!
//! TODO: Implement XDPoS-specific logic:
//! - Pre-block: Consensus version detection
//! - Post-block: Checkpoint reward application
//! - State root: Cache integration for V2 consensus

use alloy_primitives::Address;
use reth_chainspec::ChainSpec;
use std::sync::Arc;

/// XDC execution configuration
///
/// This will be used to customize the block executor from the EVM config
#[derive(Debug, Clone)]
pub struct XdcExecutionConfig {
    /// Chain specification
    chain_spec: Arc<ChainSpec>,
}

impl XdcExecutionConfig {
    /// Create a new XDC execution configuration
    pub fn new(chain_spec: Arc<ChainSpec>) -> Self {
        Self { chain_spec }
    }

    /// Get the consensus version for a block number
    pub fn consensus_version(&self, block_number: u64) -> ConsensusVersion {
        ConsensusVersion::for_block(self.chain_spec.chain().id(), block_number)
    }

    /// Check if rewards should be applied at this block
    pub fn should_apply_rewards(&self, block_number: u64) -> bool {
        const EPOCH: u64 = 900;
        block_number % EPOCH == 0
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
        let spec = Arc::new(ChainSpec::default());
        let config = XdcExecutionConfig::new(spec);

        // Checkpoint blocks
        assert!(config.should_apply_rewards(0));
        assert!(config.should_apply_rewards(900));
        assert!(config.should_apply_rewards(1800));

        // Non-checkpoint blocks
        assert!(!config.should_apply_rewards(1));
        assert!(!config.should_apply_rewards(899));
        assert!(!config.should_apply_rewards(901));
    }
}
