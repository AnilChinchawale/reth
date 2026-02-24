//! XDC EVM Configuration
//!
//! This module provides the EVM configuration for XDC Network, including:
//! - EIP-158 state clear disabled (XDC requirement)
//! - TIPSigning gas exemptions for system contracts
//! - Custom precompile addresses (future)

pub mod config;

use alloy_consensus::Header;
use alloy_evm::{
    eth::{EthBlockExecutionCtx, EthBlockExecutorFactory},
    EthEvmFactory,
};
use alloy_primitives::{Address, Bytes, U256};
use core::convert::Infallible;
use reth_chainspec::{ChainSpec, EthChainSpec, EthereumHardforks};
use reth_ethereum_primitives::{EthPrimitives, TransactionSigned};
use reth_evm::{
    eth::{spec::EthExecutorSpec, NextEvmEnvAttributes},
    ConfigureEvm, EvmEnv, NextBlockEnvAttributes,
};
use reth_primitives_traits::{SealedBlock, SealedHeader};
use revm::primitives::hardfork::SpecId;
use std::{borrow::Cow, sync::Arc};

use crate::build::XdcBlockAssembler;
use crate::receipt::XdcReceiptBuilder;

/// XDC EVM configuration
#[derive(Debug, Clone)]
pub struct XdcEvmConfig<C = ChainSpec> {
    /// Inner Ethereum block executor factory
    pub executor_factory: EthBlockExecutorFactory<XdcReceiptBuilder, Arc<C>, EthEvmFactory>,
    /// XDC block assembler
    pub block_assembler: XdcBlockAssembler<C>,
    /// Chain specification
    chain_spec: Arc<C>,
}

impl<C> XdcEvmConfig<C> {
    /// Create a new XDC EVM configuration
    pub fn new(chain_spec: Arc<C>) -> Self {
        Self {
            block_assembler: XdcBlockAssembler::new(chain_spec.clone()),
            executor_factory: EthBlockExecutorFactory::new(
                XdcReceiptBuilder::default(),
                chain_spec.clone(),
                EthEvmFactory::default(),
            ),
            chain_spec,
        }
    }

    /// Check if EIP-158 state clear should be disabled
    ///
    /// XDC Networks (chain ID 50, 51) disable EIP-158 state clearing
    /// to maintain compatibility with existing contracts
    fn disable_eip158_state_clear(&self) -> bool
    where
        C: EthChainSpec,
    {
        let chain_id = self.chain_spec.chain().id();
        chain_id == 50 || chain_id == 51 // XDC Mainnet or Apothem
    }

    /// Check if a transaction is eligible for TIPSigning gas exemption
    ///
    /// TIPSigning allows specific system contract interactions to be gas-free
    /// Active after block 3,000,000 on both mainnet and testnet
    fn is_tipsigning_tx(&self, block_number: u64, to: Option<Address>) -> bool {
        // TIPSigning activation block
        const TIPSIGNING_BLOCK: u64 = 3_000_000;

        if block_number < TIPSIGNING_BLOCK {
            return false;
        }

        // System contract addresses eligible for gas exemption
        const VALIDATOR_CONTRACT: Address =
            Address::new([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x88]);
        const BLOCK_SIGNERS_CONTRACT: Address =
            Address::new([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x89]);

        // Check if transaction is to a system contract
        to == Some(VALIDATOR_CONTRACT) || to == Some(BLOCK_SIGNERS_CONTRACT)
    }
}

impl<C> ConfigureEvm for XdcEvmConfig<C>
where
    C: EthExecutorSpec + EthChainSpec<Header = Header> + reth_ethereum_forks::Hardforks + 'static,
{
    type Primitives = EthPrimitives;
    type Error = Infallible;
    type NextBlockEnvCtx = NextBlockEnvAttributes;
    type BlockExecutorFactory = EthBlockExecutorFactory<XdcReceiptBuilder, Arc<C>, EthEvmFactory>;
    type BlockAssembler = XdcBlockAssembler<C>;

    fn block_executor_factory(&self) -> &Self::BlockExecutorFactory {
        &self.executor_factory
    }

    fn block_assembler(&self) -> &Self::BlockAssembler {
        &self.block_assembler
    }

    fn evm_env(&self, header: &Header) -> Result<EvmEnv<SpecId>, Self::Error> {
        Ok(EvmEnv::for_eth_block(
            header,
            self.chain_spec.as_ref(),
            self.chain_spec.chain().id(),
            self.chain_spec.blob_params_at_timestamp(header.timestamp),
        ))
    }

    fn next_evm_env(
        &self,
        parent: &Header,
        attributes: &NextBlockEnvAttributes,
    ) -> Result<EvmEnv, Self::Error> {
        Ok(EvmEnv::for_eth_next_block(
            parent,
            NextEvmEnvAttributes {
                timestamp: attributes.timestamp,
                suggested_fee_recipient: attributes.suggested_fee_recipient,
                prev_randao: attributes.prev_randao,
                gas_limit: attributes.gas_limit,
            },
            self.chain_spec.next_block_base_fee(parent, attributes.timestamp).unwrap_or_default(),
            self.chain_spec.as_ref(),
            self.chain_spec.chain().id(),
            self.chain_spec.blob_params_at_timestamp(attributes.timestamp),
        ))
    }

    fn context_for_block<'a>(
        &self,
        block: &'a SealedBlock<reth_ethereum_primitives::Block>,
    ) -> Result<EthBlockExecutionCtx<'a>, Self::Error> {
        Ok(EthBlockExecutionCtx {
            tx_count_hint: Some(block.transaction_count()),
            parent_hash: block.header().parent_hash,
            parent_beacon_block_root: block.header().parent_beacon_block_root,
            ommers: &block.body().ommers,
            withdrawals: block.body().withdrawals.as_ref().map(Cow::Borrowed),
            extra_data: block.header().extra_data.clone(),
        })
    }

    fn context_for_next_block(
        &self,
        parent: &SealedHeader,
        attributes: Self::NextBlockEnvCtx,
    ) -> Result<EthBlockExecutionCtx<'_>, Self::Error> {
        Ok(EthBlockExecutionCtx {
            tx_count_hint: None,
            parent_hash: parent.hash(),
            parent_beacon_block_root: attributes.parent_beacon_block_root,
            ommers: &[],
            withdrawals: attributes.withdrawals.map(Cow::Owned),
            extra_data: attributes.extra_data,
        })
    }
}

impl<C> reth_evm::ConfigureEngineEvm<alloy_rpc_types_engine::ExecutionData> for XdcEvmConfig<C>
where
    C: EthExecutorSpec + EthChainSpec<Header = Header> + reth_ethereum_forks::Hardforks + 'static,
{
    fn evm_env_for_payload(
        &self,
        payload: &alloy_rpc_types_engine::ExecutionData,
    ) -> Result<reth_evm::EvmEnvFor<Self>, Self::Error> {
        let timestamp = payload.payload.timestamp();
        let block_number = payload.payload.block_number();

        let blob_params = self.chain_spec.blob_params_at_timestamp(timestamp);
        let spec = crate::evm::config::revm_spec_by_timestamp_and_block_number(
            self.chain_spec.as_ref(),
            timestamp,
            block_number,
        );

        // configure evm env based on parent block
        let mut cfg_env = revm::context::CfgEnv::new()
            .with_chain_id(self.chain_spec.chain().id())
            .with_spec_and_mainnet_gas_params(spec);

        if let Some(blob_params) = &blob_params {
            cfg_env.set_max_blobs_per_tx(blob_params.max_blobs_per_tx);
        }

        // XDC: No Osaka fork yet, but keeping this for future compatibility
        // if self.chain_spec.is_osaka_active_at_timestamp(timestamp) {
        //     cfg_env.tx_gas_limit_cap = Some(MAX_TX_GAS_LIMIT_OSAKA);
        // }

        // derive the EIP-4844 blob fees
        let blob_excess_gas_and_price =
            payload.payload.excess_blob_gas().zip(blob_params).map(|(excess_blob_gas, params)| {
                let blob_gasprice = params.calc_blob_fee(excess_blob_gas);
                revm::context_interface::block::BlobExcessGasAndPrice {
                    excess_blob_gas,
                    blob_gasprice,
                }
            });

        let block_env = revm::context::BlockEnv {
            number: U256::from(block_number),
            beneficiary: payload.payload.fee_recipient(),
            timestamp: U256::from(timestamp),
            difficulty: if spec >= SpecId::MERGE {
                U256::ZERO
            } else {
                payload.payload.as_v1().prev_randao.into()
            },
            prevrandao: (spec >= SpecId::MERGE).then(|| payload.payload.as_v1().prev_randao),
            gas_limit: payload.payload.gas_limit(),
            basefee: payload.payload.saturated_base_fee_per_gas(),
            blob_excess_gas_and_price,
        };

        Ok(EvmEnv { cfg_env, block_env })
    }

    fn context_for_payload<'a>(
        &self,
        payload: &'a alloy_rpc_types_engine::ExecutionData,
    ) -> Result<reth_evm::ExecutionCtxFor<'a, Self>, Self::Error> {
        Ok(EthBlockExecutionCtx {
            tx_count_hint: Some(payload.payload.transactions().len()),
            parent_hash: payload.parent_hash(),
            parent_beacon_block_root: payload.sidecar.parent_beacon_block_root(),
            ommers: &[],
            withdrawals: payload.payload.withdrawals().map(|w| Cow::Owned(w.clone().into())),
            extra_data: payload.payload.as_v1().extra_data.clone(),
        })
    }

    fn tx_iterator_for_payload(
        &self,
        payload: &alloy_rpc_types_engine::ExecutionData,
    ) -> Result<impl reth_evm::ExecutableTxIterator<Self>, Self::Error> {
        use reth_primitives_traits::SignedTransaction;
        use reth_storage_errors::any::AnyError;

        let txs = payload.payload.transactions().clone();
        let convert = |tx: Bytes| {
            use alloy_eips::Decodable2718;

            let tx = reth_ethereum_primitives::TransactionSigned::decode_2718_exact(tx.as_ref())
                .map_err(AnyError::new)?;
            let signer = tx.try_recover().map_err(AnyError::new)?;
            Ok::<_, AnyError>(tx.with_signer(signer))
        };

        Ok((txs, convert))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use reth_chainspec::Chain;

    fn create_xdc_mainnet_spec() -> Arc<ChainSpec> {
        let mut spec = ChainSpec::default();
        spec.chain = Chain::from_id(50);
        Arc::new(spec)
    }

    fn create_xdc_apothem_spec() -> Arc<ChainSpec> {
        let mut spec = ChainSpec::default();
        spec.chain = Chain::from_id(51);
        Arc::new(spec)
    }

    #[test]
    fn test_eip158_disabled_on_xdc() {
        let config = XdcEvmConfig::new(create_xdc_mainnet_spec());
        assert!(config.disable_eip158_state_clear());

        let config = XdcEvmConfig::new(create_xdc_apothem_spec());
        assert!(config.disable_eip158_state_clear());
    }

    #[test]
    fn test_eip158_enabled_on_ethereum() {
        let mut spec = ChainSpec::default();
        spec.chain = Chain::from_id(1);
        let config = XdcEvmConfig::new(Arc::new(spec));
        assert!(!config.disable_eip158_state_clear());
    }

    #[test]
    fn test_tipsigning_activation() {
        let config = XdcEvmConfig::new(create_xdc_mainnet_spec());
        let validator_addr = Address::new([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x88]);

        // Before activation
        assert!(!config.is_tipsigning_tx(2_999_999, Some(validator_addr)));

        // After activation
        assert!(config.is_tipsigning_tx(3_000_000, Some(validator_addr)));
        assert!(config.is_tipsigning_tx(4_000_000, Some(validator_addr)));
    }

    #[test]
    fn test_tipsigning_only_system_contracts() {
        let config = XdcEvmConfig::new(create_xdc_mainnet_spec());
        let random_addr = Address::new([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20]);

        // Random address should not be exempt
        assert!(!config.is_tipsigning_tx(4_000_000, Some(random_addr)));

        // System contracts should be exempt
        let validator_addr = Address::new([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x88]);
        let signers_addr = Address::new([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x89]);

        assert!(config.is_tipsigning_tx(4_000_000, Some(validator_addr)));
        assert!(config.is_tipsigning_tx(4_000_000, Some(signers_addr)));
    }
}
