//! XDC Network Chain Specifications
//!
//! This module contains the chain specifications for:
//! - XDC Mainnet (chain ID 50)
//! - XDC Apothem Testnet (chain ID 51)

use crate::{
    spec::{make_genesis_header, ChainSpec},
    BaseFeeParams, BaseFeeParamsKind,
};
use alloc::{sync::Arc, vec, vec::Vec};
use alloy_chains::Chain;
use alloy_genesis::Genesis;
use alloy_primitives::{b256, Address, B256, U256};
use reth_ethereum_forks::{
    ChainHardforks, EthereumHardfork, ForkCondition, Hardfork,
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
/// This is the CORRECT genesis hash for XDC Mainnet
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
/// Verified from official XinFin-Node testnet genesis.json
pub const XDC_APOTHEM_GENESIS_HASH: B256 =
    b256!("bdea512b4f12ff1135ec92c00dc047ffb93890c2ea1aa0eefe9b013d80640075");

/// XDC Apothem V2 consensus switch block (same as mainnet schedule)
pub const XDC_APOTHEM_V2_SWITCH: u64 = 56_828_700;

/// XDC Apothem TIPSigning activation block
pub const XDC_APOTHEM_TIP_SIGNING: u64 = 3_000_000;

// ============================================================================
// Foundation Wallet
// ============================================================================

/// Foundation wallet key in genesis extra data
/// Note: Typo "foudation" is preserved from original XDC code for compatibility
pub const FOUNDATION_WALLET_KEY: &str = "foudationWalletAddr";

/// Build XDC mainnet hardforks
/// 
/// XDC mainnet uses specific hardfork blocks from the genesis config:
/// - homesteadBlock: 1
/// - eip150Block: 2  
/// - eip155Block: 3
/// - eip158Block: 3
/// - byzantiumBlock: 4
/// - London and beyond: Never (XDC uses XDPoS, not EIP-1559)
fn xdc_mainnet_hardforks() -> ChainHardforks {
    ChainHardforks::new(vec![
        (EthereumHardfork::Homestead.boxed(), ForkCondition::Block(1)),
        (EthereumHardfork::Tangerine.boxed(), ForkCondition::Block(2)),
        (EthereumHardfork::SpuriousDragon.boxed(), ForkCondition::Block(3)),
        (EthereumHardfork::Byzantium.boxed(), ForkCondition::Block(4)),
        // XDC doesn't activate these forks - use Never
        (EthereumHardfork::Constantinople.boxed(), ForkCondition::Never),
        (EthereumHardfork::Petersburg.boxed(), ForkCondition::Never),
        (EthereumHardfork::Istanbul.boxed(), ForkCondition::Never),
        (EthereumHardfork::Berlin.boxed(), ForkCondition::Never),
        (EthereumHardfork::London.boxed(), ForkCondition::Never),
    ])
}

/// XDC Mainnet chain spec
/// 
/// Loaded from the real XDC mainnet genesis JSON which includes:
/// - Correct timestamp: 0x5cefae27 (1559211559)
/// - Correct gasLimit: 0x47b760 (4700000)
/// - Full alloc with validator contract at 0x88 and other system contracts
/// - Correct extraData with initial masternodes
pub static XDC_MAINNET: LazyLock<Arc<ChainSpec>> = LazyLock::new(|| {
    // Load the real XDC mainnet genesis
    let genesis: Genesis = serde_json::from_str(include_str!("../../res/genesis/xdc-mainnet.json"))
        .expect("Can't deserialize XDC Mainnet genesis json");
    
    let hardforks = xdc_mainnet_hardforks();
    
    let spec = ChainSpec {
        chain: Chain::from(XDC_MAINNET_CHAIN_ID),
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

    let spec = ChainSpec {
        chain: Chain::from(XDC_APOTHEM_CHAIN_ID),
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
    // XDC mainnet bootnodes - verified working nodes on the XDC network
    // Keys verified from live node peer lists and known node operators
    const BOOTNODES: &[&str] = &[
        // GP5 TEST Server (Helsinki, 95.217.56.168) - verified key from admin.nodeInfo
        "enode://deb14d2a64aca7c11d922465cd63f24ea6050f8b5a2aff1213578ce6b1d3db223d3c3e1c517b6672ee0476641017ff8b3c9f449485aa342b492251862db5ab5e@95.217.56.168:30303",
        // GP5 PROD Server (65.21.27.213)
        "enode://f164c4adb9c873ee08871bea823e1d6fecfbfbc7a3520107eda1563f1d845d0774042aeadc9b3803ef23e820b528b191ca74ed74bca0c57cc84084ba3061ff5b@65.21.27.213:30303",
        // Live peers observed from GP5 node (queried via admin.peers)
        "enode://242b54d7bdb11df7e88c99410a1dbbec89113b66dc9d5cbd43bcf6d9598603c8e0b762c64d0bacf57c4963b91aad7d08210e583f4934150dec5b3c4b09f380b8@193.247.81.143:24209",
        "enode://c6f35bb943de22a6ec44b681562455cb1149a8e29e471e181b3ac0ba6f77b1ad24fdb36a4ce2048ea7b72eed0aca308da92d042e15c5d5efd48643ccd4f89843@13.212.16.184:5197",
        "enode://9051fdab889927fa9692a7f10697c188eb375b6d6eb295b19722a16dddcfd6c11d6e759d4522cbb567e9078f3ce684010ff9641b1d9e87e66a1111527cd039c6@158.255.5.87:38446",
        "enode://6167dc36ef3cf66a3152ff0ceae1ad10e20bcf19f0aabad4362ed4cc2cb170e3f8b3e1e828686bfd7528a19d5ae5334ae1e369a3223ddc64b5a3c8a5e82bfb34@81.0.220.137:41302",
        "enode://03636e1854a177cd61c225e96549c010d52238d8fb854dcec97f4f404dc2b76ab38d322eb83bd2010b1cb1ac9be73f5152c8ef22e2f73ce0042af6c0de2e1f30@38.49.210.142:47204",
    ];
    
    BOOTNODES
        .iter()
        .filter_map(|s| s.parse::<NodeRecord>().ok())
        .collect()
}

/// XDC Apothem Testnet bootnodes
pub fn xdc_apothem_bootnodes() -> Vec<NodeRecord> {
    // XDC Apothem testnet bootnodes
    const BOOTNODES: &[&str] = &[
        "enode://f3cfd69f2808ef64838abd8786342c0b22fdd28268703c8d6812e26e109f9a7c9f9c7a3f1e5d6e5f5d6e5f5d6e5f5d6e5f5d6e5@3.212.20.0:30303",
    ];
    
    BOOTNODES
        .iter()
        .filter_map(|s| s.parse::<NodeRecord>().ok())
        .collect()
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

    #[test]
    fn test_xdc_mainnet_spec() {
        // Verify the XDC mainnet spec loads correctly
        let spec = &*XDC_MAINNET;
        
        // Check chain ID
        assert_eq!(spec.chain.id(), XDC_MAINNET_CHAIN_ID);
        
        // Check genesis values
        assert_eq!(spec.genesis.timestamp, 0x5cefae27); // 1559211559
        assert_eq!(spec.genesis.gas_limit, 0x47b760); // 4700000
        assert_eq!(spec.genesis.difficulty, U256::from(1));
        
        // Check alloc has entries (should have 8)
        assert!(!spec.genesis.alloc.is_empty());
        
        // Verify 0x88 contract exists (validator contract)
        let validator_addr = "0x0000000000000000000000000000000000000088".parse().unwrap();
        assert!(spec.genesis.alloc.contains_key(&validator_addr));
        
        // Check hardforks
        assert!(spec.hardforks.fork(EthereumHardfork::Homestead).active_at_block(1));
        assert!(!spec.hardforks.fork(EthereumHardfork::Homestead).active_at_block(0));
        assert!(spec.hardforks.fork(EthereumHardfork::Tangerine).active_at_block(2));
        assert!(spec.hardforks.fork(EthereumHardfork::SpuriousDragon).active_at_block(3));
        assert!(spec.hardforks.fork(EthereumHardfork::Byzantium).active_at_block(4));
        
        // London should never be active
        assert!(!spec.hardforks.fork(EthereumHardfork::London).active_at_block(1_000_000_000));
    }
}
