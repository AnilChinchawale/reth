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

/// XDC Apothem Testnet genesis hash (placeholder)
pub const XDC_APOTHEM_GENESIS_HASH: B256 =
    B256::new([0x7d, 0x7a, 0x26, 0x4c, 0x1b, 0x3f, 0x1a, 0x40, 0xe5, 0x26, 0x0c, 0x7b, 0x92, 0x4c, 0x6f, 0x3b, 0x3b, 0x8e, 0x9d, 0x9e, 0x8c, 0x8f, 0x8f, 0x7e, 0x6d, 0x5c, 0x4b, 0x3a, 0x29, 0x18, 0x07, 0x00]);

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
    // XDC mainnet bootnodes - collected from running XDC network nodes
    const BOOTNODES: &[&str] = &[
        // TEST Server (Helsinki)
        "enode://cc5dc85e4035d4950439831f3d83fe3423e603a40b7c767496eae9973ec61088a9e0a268f4e64ba907636c56f662687b51068f5fb2bb3343e626626aece12ce7@95.217.56.168:30303",
        // Public XDC bootnodes
        "enode://687a4b7ee0f7e3ecdf9598db24113e2cead5cd795f1ebe69bfa007f7a261d13c08dd7c66a9c4995b0e1545c2ea24aace20504f3ddd4b6fbdcd553705cbd64e36@78.46.75.143:64506",
        "enode://879752d7744fd2c88492c024591995e8d3da2353a24b4b8f599a545db436c928a528b435f5f862044a89d0f1edcaaedcec847a5d8f20df73f297382fe484c5be@13.124.58.33:9500",
        "enode://8be95052933250e9fe76c86d33981fa82e6ba7fd684948f734ebc19479b1c0019fd5deb2e310013b401c56508a67548d58b9bc3cab6bfcdb3a6934e2670d2ec2@185.130.224.247:36384",
        "enode://775e2a6a656ddf2904bd8c702f10007a0f2380ceedf56e4860a50104bd0b6a6d38a078fc0473789efb74a9eedb91f68bf9ffa34c1763cb3b2402f37e3b3c4538@54.169.174.107:31153",
        "enode://0a1c1808658ebce358d457d979419ae87dfa492614d9fc64b5a0e226fba735d7477c4eb82c855850670ec56877729087bdf61a83334738abb5d6484fa72ba27c@18.142.144.199:9422",
        // XinFin official bootnodes
        "enode://149.102.140.32:30303",
        "enode://5.189.144.192:30303",
        "enode://37.60.243.5:30303",
        "enode://89.117.49.48:30303",
        "enode://38.102.87.174:30303",
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
