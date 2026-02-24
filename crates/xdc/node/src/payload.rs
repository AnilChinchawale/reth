//! XDC Payload Builder
//!
//! This module provides payload building capabilities for XDC validators.
//! It wraps Reth's basic payload builder with XDPoS-specific logic.

use reth_basic_payload_builder::{BasicPayloadJobGenerator, BasicPayloadJobGeneratorConfig};
use reth_chainspec::ChainSpec;
use reth_errors::RethResult;
use reth_evm::ConfigureEvm;
use reth_payload_builder::{PayloadBuilderHandle, PayloadBuilderService};
use reth_payload_primitives::{PayloadBuilder, PayloadBuilderAttributes};
use reth_primitives::{Block, BlockBody, Header};
use reth_provider::CanonStateSubscriptions;
use reth_transaction_pool::TransactionPool;
use std::sync::Arc;
use tracing::debug;

/// XDC payload builder
///
/// Builds blocks for XDC validators with XDPoS-specific constraints
#[derive(Debug, Clone)]
pub struct XdcPayloadBuilder {
    /// Chain specification
    chain_spec: Arc<ChainSpec>,
}

impl XdcPayloadBuilder {
    /// Create a new XDC payload builder
    pub fn new(chain_spec: Arc<ChainSpec>) -> Self {
        Self { chain_spec }
    }

    /// Check if the current node should build a block
    ///
    /// In XDPoS:
    /// - V1: Validators build blocks in round-robin order
    /// - V2: Leader builds blocks based on QC voting
    ///
    /// This is a simplified version. Full implementation would:
    /// 1. Check if we're a registered masternode
    /// 2. Check if it's our turn (V1) or we're the leader (V2)
    /// 3. Validate we have the necessary credentials
    pub fn should_build(&self, block_number: u64) -> bool {
        // TODO: Implement actual validator turn checking
        // For now, always allow building (node operator will configure)
        debug!(
            block_number,
            "XDC payload builder: should_build check (default: true)"
        );
        true
    }

    /// Validate extra data for XDPoS consensus
    ///
    /// XDPoS extra data format:
    /// - V1: [32 bytes vanity][n*20 bytes validators][65 bytes seal]
    /// - V2: [32 bytes vanity][V2 extra fields][65 bytes seal]
    fn validate_extra_data(&self, extra_data: &[u8], block_number: u64) -> RethResult<()> {
        use reth_consensus_xdpos::ConsensusVersion;

        let version = ConsensusVersion::for_block(self.chain_spec.chain.id(), block_number);

        match version {
            ConsensusVersion::V1 => {
                // V1: Minimum 32 (vanity) + 65 (seal) = 97 bytes
                if extra_data.len() < 97 {
                    return Err(reth_errors::RethError::Custom(
                        "XDPoS V1 extra data too short".into(),
                    ));
                }
            }
            ConsensusVersion::V2 => {
                // V2: Minimum 32 (vanity) + 1 (version byte) + 65 (seal) = 98 bytes
                // Plus QC/TC data which varies
                if extra_data.len() < 98 {
                    return Err(reth_errors::RethError::Custom(
                        "XDPoS V2 extra data too short".into(),
                    ));
                }
            }
        }

        Ok(())
    }
}

impl PayloadBuilder for XdcPayloadBuilder {
    type Attributes = PayloadBuilderAttributes;
    type BuiltPayload = Block;

    fn build_empty_payload(
        &self,
        client: &impl reth_provider::StateProviderFactory,
        config: PayloadBuilderAttributes,
    ) -> RethResult<Self::BuiltPayload> {
        debug!(
            parent = %config.parent_beacon_block_root.unwrap_or_default(),
            timestamp = config.timestamp,
            "Building empty XDC payload"
        );

        // Build basic empty block
        let header = Header {
            parent_hash: config.parent_beacon_block_root.unwrap_or_default(),
            number: config.block_number(),
            timestamp: config.timestamp,
            gas_limit: config.gas_limit.unwrap_or(30_000_000),
            beneficiary: config.suggested_fee_recipient,
            difficulty: alloy_primitives::U256::from(1), // XDPoS uses difficulty=1
            ..Default::default()
        };

        let block = Block {
            header,
            body: BlockBody::default(),
        };

        Ok(block)
    }

    fn try_build(
        &self,
        args: reth_payload_primitives::PayloadBuilderArgs,
    ) -> RethResult<Self::BuiltPayload> {
        debug!(
            block_number = args.config.block_number(),
            parent = %args.config.parent_beacon_block_root.unwrap_or_default(),
            "Building XDC payload"
        );

        // Check if we should build
        if !self.should_build(args.config.block_number()) {
            debug!("Not our turn to build block");
            return self.build_empty_payload(args.client, args.config);
        }

        // Build block using basic payload builder logic
        // TODO: Integrate with XDPoS-specific block building:
        // - Select transactions based on gas limits
        // - Apply proper extra data format
        // - Sign block with validator key
        // - Include QC/TC for V2

        self.build_empty_payload(args.client, args.config)
    }

    fn on_missing_payload(
        &self,
        args: reth_payload_primitives::PayloadBuilderArgs,
    ) -> RethResult<Self::BuiltPayload> {
        debug!(
            block_number = args.config.block_number(),
            "Handling missing payload"
        );

        // Return empty block as fallback
        self.build_empty_payload(args.client, args.config)
    }
}

/// Create XDC payload builder service
///
/// This sets up the payload building infrastructure for validator nodes
pub fn create_payload_builder<Pool, Client, Evm>(
    chain_spec: Arc<ChainSpec>,
    evm_config: Evm,
    pool: Pool,
    client: Client,
) -> PayloadBuilderService<BasicPayloadJobGenerator<Client, Pool, Evm>>
where
    Pool: TransactionPool + 'static,
    Client: CanonStateSubscriptions + Clone + 'static,
    Evm: ConfigureEvm<Header = Header> + 'static,
{
    let payload_builder = XdcPayloadBuilder::new(chain_spec.clone());

    let config = BasicPayloadJobGeneratorConfig::default();

    let generator = BasicPayloadJobGenerator::with_builder(
        client,
        pool,
        Default::default(), // task executor
        config,
        chain_spec,
        evm_config,
    );

    PayloadBuilderService::new(generator)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::b256;

    fn create_xdc_spec() -> Arc<ChainSpec> {
        let mut spec = ChainSpec::default();
        spec.chain = reth_chainspec::Chain::from_id(50);
        Arc::new(spec)
    }

    #[test]
    fn test_payload_builder_creation() {
        let spec = create_xdc_spec();
        let builder = XdcPayloadBuilder::new(spec);

        // Should allow building by default
        assert!(builder.should_build(1000));
    }

    #[test]
    fn test_v1_extra_data_validation() {
        let spec = create_xdc_spec();
        let builder = XdcPayloadBuilder::new(spec);

        // Too short
        let short_data = vec![0u8; 96];
        assert!(builder.validate_extra_data(&short_data, 1000).is_err());

        // Valid minimum
        let valid_data = vec![0u8; 97];
        assert!(builder.validate_extra_data(&valid_data, 1000).is_ok());

        // Valid with validators
        let with_validators = vec![0u8; 97 + 20 * 5]; // 5 validators
        assert!(builder.validate_extra_data(&with_validators, 1000).is_ok());
    }

    #[test]
    fn test_v2_extra_data_validation() {
        let spec = create_xdc_spec();
        let builder = XdcPayloadBuilder::new(spec);

        // V2 blocks start at 56,857,600
        let v2_block = 56_857_600;

        // Too short
        let short_data = vec![0u8; 97];
        assert!(builder.validate_extra_data(&short_data, v2_block).is_err());

        // Valid minimum
        let valid_data = vec![0u8; 98];
        assert!(builder.validate_extra_data(&valid_data, v2_block).is_ok());

        // Valid with QC data
        let with_qc = vec![0u8; 200]; // QC adds extra bytes
        assert!(builder.validate_extra_data(&with_qc, v2_block).is_ok());
    }
}
