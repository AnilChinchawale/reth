//! XDC Network Chain Specifications
//!
//! This module contains the chain specifications for:
//! - XDC Mainnet (chain ID 50)
//! - XDC Apothem Testnet (chain ID 51)

use crate::{
    spec::{make_genesis_header, ChainSpec},
    BaseFeeParams, BaseFeeParamsKind, EthChainSpec,
};
use alloc::sync::Arc;
use alloy_chains::{Chain, NamedChain};
use alloy_consensus::constants::EMPTY_OMMER_ROOT_HASH;
use alloy_eips::eip1559::INITIAL_BASE_FEE;
use alloy_genesis::Genesis;
use alloy_primitives::{b256, Address, B256, U256};
use reth_ethereum_forks::{
    ChainHardforks, EthereumHardfork, ForkCondition, Hardforks,
};
use reth_network_peers::NodeRecord;
use reth_primitives_traits::SealedHeader;
use reth_primitives_traits::sync::LazyLock;

// ============================================================================
// XDC Mainnet Constants
// ============================================================================

/// XDC Mainnet chain ID
pub const XDC_MAINNET_CHAIN_ID: u64 = 50;

/// XDC Mainnet genesis hash
pub const XDC_MAINNET_GENESIS_HASH: B256 =
    b256!("4a9d748bd78a8d0385b67788c2435dcdb914f98a96250b68863a1f8b7642d6b1");

/// XDC Mainnet V2 consensus switch block (transition from V1 to V2)
pub const XDC_MAINNET_V2_SWITCH: u64 = 56_857_600;

/// XDC Mainnet TIPSigning activation block (free gas for system contracts)
pub const XDC_MAINNET_TIP_SIGNING: u64 = 3_000_000;

// ============================================================================
// XDC Apothem Testnet Constants
// ============================================================================

/// XDC Apothem Testnet chain ID
pub const XDC_APOTHEM_CHAIN_ID: u64 = 51;

/// XDC Apothem Testnet genesis hash
pub const XDC_APOTHEM_GENESIS_HASH: B256 =
    b256!("7d7a264c1b3f1a40e5260c7b924c6f3b3b8e9d9e8c8f8f7e6d5c4b3a2918070605"); // TODO: Set actual

/// XDC Apothem V2 consensus switch block
pub const XDC_APOTHEM_V2_SWITCH: u64 = 23_556_600;

/// XDC Apothem TIPSigning activation block
pub const XDC_APOTHEM_TIP_SIGNING: u64 = 3_000_000;

// ============================================================================
// Foundation Wallet
// ============================================================================

/// Foundation wallet key in genesis extra data
/// Note: Typo "foudation" is preserved from original XDC code for compatibility
pub const FOUNDATION_WALLET_KEY: &str = "foudationWalletAddr";

/// XDC Mainnet chain spec
pub static XDC_MAINNET: LazyLock<Arc<ChainSpec>> = LazyLock::new(|| {
    let genesis = Genesis {
        nonce: 0,
        timestamp: 1546272000, // 2019-01-01 00:00:00 UTC
        extra_data: hex::decode(
            "0000000000000000000000000000000000000000000000000000000000000000\
             0000000000000000000000000000000000000000000000000000000000000000"
        ).unwrap_or_default().into(),
        gas_limit: 50_000_000,
        difficulty: U256::from(1),
        mix_hash: B256::ZERO,
        coinbase: Address::ZERO,
        alloc: Default::default(),
        ..Default::default()
    };

    // XDC uses a simplified hardfork structure
    let hardforks = ChainHardforks::new(vec![
        (EthereumHardfork::Homestead.boxed(), ForkCondition::Block(0)),
        (EthereumHardfork::Tangerine.boxed(), ForkCondition::Block(0)),
        (EthereumHardfork::SpuriousDragon.boxed(), ForkCondition::Block(0)),
        (EthereumHardfork::Byzantium.boxed(), ForkCondition::Block(0)),
        (EthereumHardfork::Constantinople.boxed(), ForkCondition::Block(0)),
        (EthereumHardfork::Petersburg.boxed(), ForkCondition::Block(0)),
        (EthereumHardfork::Istanbul.boxed(), ForkCondition::Block(0)),
        (EthereumHardfork::Berlin.boxed(), ForkCondition::Block(0)),
        (EthereumHardfork::London.boxed(), ForkCondition::Never),
    ]);

    let mut spec = ChainSpec {
        chain: Chain::from(50), // XDC Mainnet
        genesis_header: SealedHeader::new(
            make_genesis_header(&genesis, &hardforks),
            XDC_MAINNET_GENESIS_HASH,
        ),
        genesis,
        paris_block_and_final_difficulty: None, // XDPoS, not PoS
        hardforks,
        deposit_contract: None,
        base_fee_params: BaseFeeParamsKind::Constant(BaseFeeParams::ethereum()),
        prune_delete_limit: 10000,
        blob_params: Default::default(),
    };

    spec.into()
});

/// XDC Apothem Testnet chain spec
pub static XDC_APOTHEM: LazyLock<Arc<ChainSpec>> = LazyLock::new(|| {
    let genesis = Genesis {
        nonce: 0,
        timestamp: 1546272000,
        extra_data: Default::default(),
        gas_limit: 50_000_000,
        difficulty: U256::from(1),
        mix_hash: B256::ZERO,
        coinbase: Address::ZERO,
        alloc: Default::default(),
        ..Default::default()
    };

    let hardforks = ChainHardforks::new(vec![
        (EthereumHardfork::Homestead.boxed(), ForkCondition::Block(0)),
        (EthereumHardfork::Tangerine.boxed(), ForkCondition::Block(0)),
        (EthereumHardfork::SpuriousDragon.boxed(), ForkCondition::Block(0)),
        (EthereumHardfork::Byzantium.boxed(), ForkCondition::Block(0)),
        (EthereumHardfork::Constantinople.boxed(), ForkCondition::Block(0)),
        (EthereumHardfork::Petersburg.boxed(), ForkCondition::Block(0)),
        (EthereumHardfork::Istanbul.boxed(), ForkCondition::Block(0)),
        (EthereumHardfork::Berlin.boxed(), ForkCondition::Block(0)),
        (EthereumHardfork::London.boxed(), ForkCondition::Never),
    ]);

    let mut spec = ChainSpec {
        chain: Chain::from(51), // XDC Apothem Testnet
        genesis_header: SealedHeader::new(
            make_genesis_header(&genesis, &hardforks),
            XDC_APOTHEM_GENESIS_HASH,
        ),
        genesis,
        paris_block_and_final_difficulty: None,
        hardforks,
        deposit_contract: None,
        base_fee_params: BaseFeeParamsKind::Constant(BaseFeeParams::ethereum()),
        prune_delete_limit: 10000,
        blob_params: Default::default(),
    };

    spec.into()
});

// ============================================================================
// Helper Functions
// ============================================================================

/// Auto-detect XDC chain and return V2 switch block
///
/// Returns the block number where the chain switches from XDPoS V1 to V2,
/// or None if the chain ID is not an XDC chain.
///
/// # Arguments
/// * `chain_id` - The chain ID to check
///
/// # Returns
/// * `Some(block_number)` for XDC chains (mainnet or Apothem)
/// * `None` for non-XDC chains
pub fn v2_switch_block(chain_id: u64) -> Option<u64> {
    match chain_id {
        XDC_MAINNET_CHAIN_ID => Some(XDC_MAINNET_V2_SWITCH),
        XDC_APOTHEM_CHAIN_ID => Some(XDC_APOTHEM_V2_SWITCH),
        _ => None,
    }
}

/// Check if this is an XDC chain
///
/// # Arguments
/// * `chain_id` - The chain ID to check
///
/// # Returns
/// `true` if the chain is XDC Mainnet (50) or Apothem (51)
pub fn is_xdc_chain(chain_id: u64) -> bool {
    chain_id == XDC_MAINNET_CHAIN_ID || chain_id == XDC_APOTHEM_CHAIN_ID
}

/// Get TIPSigning activation block for a chain
///
/// Returns the block number where TIPSigning (free gas for system contracts)
/// is activated, or None if the chain ID is not an XDC chain.
///
/// # Arguments
/// * `chain_id` - The chain ID to check
///
/// # Returns
/// * `Some(block_number)` for XDC chains
/// * `None` for non-XDC chains
pub fn tipsigning_block(chain_id: u64) -> Option<u64> {
    match chain_id {
        XDC_MAINNET_CHAIN_ID => Some(XDC_MAINNET_TIP_SIGNING),
        XDC_APOTHEM_CHAIN_ID => Some(XDC_APOTHEM_TIP_SIGNING),
        _ => None,
    }
}

// ============================================================================
// Bootnodes
// ============================================================================

/// XDC Mainnet bootnodes
pub fn xdc_mainnet_bootnodes() -> Vec<NodeRecord> {
    // TODO: Add actual XDC mainnet bootnodes
    // Format: enode://<node_id>@<ip>:<port>
    vec![]
}

/// XDC Apothem Testnet bootnodes
pub fn xdc_apothem_bootnodes() -> Vec<NodeRecord> {
    // TODO: Add actual XDC apothem bootnodes
    // Format: enode://<node_id>@<ip>:<port>
    vec![]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_v2_switch_detection() {
        // XDC Mainnet
        assert_eq!(v2_switch_block(50), Some(XDC_MAINNET_V2_SWITCH));
        assert_eq!(v2_switch_block(50), Some(56_857_600));
        
        // XDC Apothem
        assert_eq!(v2_switch_block(51), Some(XDC_APOTHEM_V2_SWITCH));
        assert_eq!(v2_switch_block(51), Some(23_556_600));
        
        // Non-XDC chains
        assert_eq!(v2_switch_block(1), None); // Ethereum
        assert_eq!(v2_switch_block(137), None); // Polygon
    }

    #[test]
    fn test_is_xdc_chain() {
        // XDC chains
        assert!(is_xdc_chain(50)); // Mainnet
        assert!(is_xdc_chain(51)); // Apothem
        
        // Non-XDC chains
        assert!(!is_xdc_chain(1)); // Ethereum
        assert!(!is_xdc_chain(137)); // Polygon
        assert!(!is_xdc_chain(0)); // Invalid
    }

    #[test]
    fn test_tipsigning_block() {
        // XDC Mainnet
        assert_eq!(tipsigning_block(50), Some(3_000_000));
        
        // XDC Apothem
        assert_eq!(tipsigning_block(51), Some(3_000_000));
        
        // Non-XDC chains
        assert_eq!(tipsigning_block(1), None);
        assert_eq!(tipsigning_block(137), None);
    }

    #[test]
    fn test_genesis_hash_constants() {
        // Verify genesis hashes are set
        assert_ne!(XDC_MAINNET_GENESIS_HASH, B256::ZERO);
        assert_ne!(XDC_APOTHEM_GENESIS_HASH, B256::ZERO);
        
        // Verify mainnet genesis hash matches expected value
        assert_eq!(
            XDC_MAINNET_GENESIS_HASH,
            b256!("4a9d748bd78a8d0385b67788c2435dcdb914f98a96250b68863a1f8b7642d6b1")
        );
    }

    #[test]
    fn test_foundation_wallet_key() {
        // Verify the typo is preserved for compatibility
        assert_eq!(FOUNDATION_WALLET_KEY, "foudationWalletAddr");
    }
}
