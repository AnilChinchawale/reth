//! XDC EVM Configuration
//!
//! This module provides the EVM configuration for XDC Network, including:
//! - EIP-158 state clear disabled (XDC requirement)
//! - TIPSigning gas exemptions for system contracts
//! - Custom precompile addresses (future)

use alloy_primitives::{Address, U256};
use reth_chainspec::{ChainSpec, EthereumHardforks};
use reth_evm::{ConfigureEvm, ConfigureEvmEnv};
use reth_evm_ethereum::EthEvmConfig;
use reth_primitives::Header;
use reth_revm::{inspector_handle_register, Database, Evm, EvmBuilder, GetInspector};
use revm_primitives::{
    AnalysisKind, BlobExcessGasAndPrice, BlockEnv, Bytes, CfgEnv, CfgEnvWithHandlerCfg, Env,
    HandlerCfg, SpecId, TxEnv, TxKind,
};
use std::sync::Arc;

/// XDC EVM configuration
#[derive(Debug, Clone)]
pub struct XdcEvmConfig {
    /// Inner Ethereum EVM config (for standard behavior)
    inner: EthEvmConfig,
    /// Chain specification
    chain_spec: Arc<ChainSpec>,
}

impl XdcEvmConfig {
    /// Create a new XDC EVM configuration
    pub fn new(chain_spec: Arc<ChainSpec>) -> Self {
        Self {
            inner: EthEvmConfig::new(chain_spec.clone()),
            chain_spec,
        }
    }

    /// Check if EIP-158 state clear should be disabled
    ///
    /// XDC Networks (chain ID 50, 51) disable EIP-158 state clearing
    /// to maintain compatibility with existing contracts
    fn disable_eip158_state_clear(&self) -> bool {
        let chain_id = self.chain_spec.chain.id();
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

impl ConfigureEvmEnv for XdcEvmConfig {
    type Header = Header;
    type Error = <EthEvmConfig as ConfigureEvmEnv>::Error;

    fn fill_tx_env(&self, tx_env: &mut TxEnv, transaction: &alloy_consensus::Transaction, sender: Address) {
        // Use Ethereum's standard tx env filling
        self.inner.fill_tx_env(tx_env, transaction, sender);

        // Apply TIPSigning gas exemption if applicable
        // Note: This is a simplified version. Full implementation would need block context
        // For now, we just set the stage for gas exemption logic
    }

    fn fill_tx_env_system_contract_call(
        &self,
        env: &mut Env,
        caller: Address,
        contract: Address,
        data: Bytes,
    ) {
        self.inner.fill_tx_env_system_contract_call(env, caller, contract, data);
    }

    fn fill_cfg_env(
        &self,
        cfg_env: &mut CfgEnvWithHandlerCfg,
        header: &Self::Header,
    ) -> Result<(), Self::Error> {
        // Start with Ethereum's standard config
        self.inner.fill_cfg_env(cfg_env, header)?;

        // XDC-specific modifications
        if self.disable_eip158_state_clear() {
            // Disable EIP-158 state clearing
            // This is done by modifying the spec ID or handler config
            // In XDC, we keep empty accounts even after touching them
            cfg_env.handler_cfg.is_eip158_enabled = false;
        }

        Ok(())
    }

    fn fill_block_env(&self, block_env: &mut BlockEnv, header: &Self::Header, after_merge: bool) {
        self.inner.fill_block_env(block_env, header, after_merge);
    }
}

impl ConfigureEvm for XdcEvmConfig {
    type DefaultExternalContext<'a> = ();

    fn evm<DB: Database>(&self, db: DB) -> Evm<'_, (), DB> {
        // Build EVM with XDC configuration
        EvmBuilder::default().with_db(db).build()
    }

    fn evm_with_inspector<DB, I>(&self, db: DB, inspector: I) -> Evm<'_, I, DB>
    where
        DB: Database,
        I: GetInspector<DB>,
    {
        EvmBuilder::default()
            .with_db(db)
            .with_external_context(inspector)
            .append_handler_register(inspector_handle_register)
            .build()
    }

    fn default_external_context<'a>(&self) -> Self::DefaultExternalContext<'a> {
        ()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::b256;
    use reth_chainspec::{Chain, ChainSpec};

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
