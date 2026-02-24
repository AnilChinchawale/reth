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

/// XDC Mainnet genesis hash
pub const XDC_MAINNET_GENESIS_HASH: B256 =
    b256!("0x4a9d748bd78a8d0385b67788c2435dcdb914f98a96250b68863a1f8b7642d6b1");

/// XDC Apothem Testnet genesis hash
pub const XDC_APOTHEM_GENESIS_HASH: B256 =
    b256!("0x7d7a264c1b3f1a40e5260c7b924c6f3b3b8e9d9e8c8f8f7e6d5c4b3a2918070605"); // TODO: Set actual

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

/// XDC Mainnet bootnodes
pub fn xdc_mainnet_bootnodes() -> Vec<NodeRecord> {
    // TODO: Add actual XDC mainnet bootnodes
    vec![]
}

/// XDC Apothem Testnet bootnodes
pub fn xdc_apothem_bootnodes() -> Vec<NodeRecord> {
    // TODO: Add actual XDC apothem bootnodes
    vec![]
}
