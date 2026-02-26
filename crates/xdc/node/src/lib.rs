//! XDC Network Node Implementation
//!
//! This crate provides the XDC Network node implementation for Reth,
//! integrating XDPoS consensus (V1+V2), custom EVM configuration,
//! and XDC-specific execution logic.
//!
//! ## Architecture
//!
//! - [`XdcNode`] - Main node type implementing Reth's `NodeTypes` and `Node` traits
//! - [`XdcEvmConfig`] - XDC EVM configuration (EIP-158 disabled, TIPSigning)

#![doc(
    html_logo_url = "https://raw.githubusercontent.com/paradigmxyz/reth/main/assets/reth-docs.png",
    html_favicon_url = "https://avatars0.githubusercontent.com/u/97369466?s=256",
    issue_tracker_base_url = "https://github.com/paradigmxyz/reth/issues/"
)]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![cfg_attr(docsrs, feature(doc_cfg))]

use alloy_primitives::{address, Address};
use reth_chainspec::{ChainSpec, EthChainSpec, EthereumHardforks};
use reth_ethereum_primitives::EthPrimitives;
use reth_evm::eth::spec::EthExecutorSpec;
use reth_node_api::{FullNodeTypes, NodePrimitives};
use reth_network::PeersInfo;
use reth_node_builder::{
    components::{ComponentsBuilder, ConsensusBuilder, ExecutorBuilder, NetworkBuilder as RethNetworkBuilder, PoolBuilder},
    node::{FullNodeTypes as FullNodeTypesT, Node, NodeTypes},
    BuilderContext,
};
use reth_payload_primitives::PayloadTypes;
use reth_provider::EthStorage;
use std::sync::Arc;
use tracing::{debug, info};

pub mod evm;
pub mod payload;
pub mod build;
pub mod receipt;

use evm::XdcEvmConfig;

/// XDC Network node
///
/// This is the main node type for XDC Network. It implements Reth's
/// `NodeTypes` and `Node` traits to provide a complete XDC node.
#[derive(Debug, Clone, Default)]
pub struct XdcNode;

impl XdcNode {
    /// Create a new XDC node
    pub fn new() -> Self {
        Self
    }
}

impl NodeTypes for XdcNode {
    type Primitives = EthPrimitives;
    type ChainSpec = ChainSpec;
    type Storage = EthStorage;
    type Payload = reth_ethereum_engine_primitives::EthEngineTypes;
}

impl<N> Node<N> for XdcNode
where
    N: FullNodeTypes<Types = Self>,
{
    type ComponentsBuilder = ComponentsBuilder<
        N,
        reth_node_ethereum::EthereumPoolBuilder,
        reth_node_builder::components::BasicPayloadServiceBuilder<reth_node_ethereum::EthereumPayloadBuilder>,
        reth_node_ethereum::EthereumNetworkBuilder,
        XdcExecutorBuilder,
        XdcConsensusBuilder,
    >;

    type AddOns = reth_node_ethereum::EthereumAddOns<
        reth_node_builder::NodeAdapter<N>,
        reth_node_ethereum::EthereumEthApiBuilder,
        reth_node_ethereum::EthereumEngineValidatorBuilder,
    >;

    fn components_builder(&self) -> Self::ComponentsBuilder {
        ComponentsBuilder::default()
            .node_types::<N>()
            .pool(reth_node_ethereum::EthereumPoolBuilder::default())
            .executor(XdcExecutorBuilder::default())
            .payload(reth_node_builder::components::BasicPayloadServiceBuilder::default())
            .network(reth_node_ethereum::EthereumNetworkBuilder::default())
            .consensus(XdcConsensusBuilder::default())
    }

    fn add_ons(&self) -> Self::AddOns {
        reth_node_ethereum::EthereumAddOns::default()
    }
}

/// XDC executor builder
#[derive(Debug, Default, Clone, Copy)]
#[non_exhaustive]
pub struct XdcExecutorBuilder;

impl<N> ExecutorBuilder<N> for XdcExecutorBuilder
where
    N: FullNodeTypes<
        Types: NodeTypes<
            ChainSpec: reth_ethereum_forks::Hardforks + EthExecutorSpec + EthChainSpec,
            Primitives = EthPrimitives,
        >,
    >,
{
    type EVM = XdcEvmConfig<<N::Types as NodeTypes>::ChainSpec>;

    async fn build_evm(self, ctx: &BuilderContext<N>) -> eyre::Result<Self::EVM> {
        info!(
            chain_id = ctx.chain_spec().chain().id(),
            "Building XDC EVM configuration"
        );
        Ok(XdcEvmConfig::new(ctx.chain_spec()))
    }
}

/// XDC transaction pool builder
#[derive(Debug, Default, Clone, Copy)]
#[non_exhaustive]
pub struct XdcPoolBuilder;

impl<Types, N, Evm> PoolBuilder<N, Evm> for XdcPoolBuilder
where
    Types: NodeTypes<
        ChainSpec: EthereumHardforks,
        Primitives: NodePrimitives<SignedTx = reth_ethereum_primitives::TransactionSigned>,
    >,
    N: FullNodeTypes<Types = Types>,
    Evm: reth_evm::ConfigureEvm<Primitives = reth_node_api::PrimitivesTy<Types>> + Clone + 'static,
{
    type Pool = reth_transaction_pool::EthTransactionPool<
        N::Provider,
        reth_transaction_pool::blobstore::DiskFileBlobStore,
        Evm,
    >;

    async fn build_pool(
        self,
        ctx: &BuilderContext<N>,
        evm_config: Evm,
    ) -> eyre::Result<Self::Pool> {
        info!(
            chain_id = ctx.chain_spec().chain().id(),
            "Building XDC transaction pool"
        );
        
        // Use Ethereum pool builder for now
        reth_node_ethereum::EthereumPoolBuilder::default()
            .build_pool(ctx, evm_config)
            .await
    }
}


/// XDC consensus builder
#[derive(Debug, Default, Clone, Copy)]
pub struct XdcConsensusBuilder;

impl<N> ConsensusBuilder<N> for XdcConsensusBuilder
where
    N: FullNodeTypes<
        Types: NodeTypes<ChainSpec: EthChainSpec + EthereumHardforks, Primitives = EthPrimitives>,
    >,
{
    type Consensus = Arc<dyn reth_consensus::FullConsensus<<N::Types as NodeTypes>::Primitives>>;

    async fn build_consensus(self, ctx: &BuilderContext<N>) -> eyre::Result<Self::Consensus> {
        use reth_chainspec::xdc::is_xdc_chain;
        use reth_consensus_xdpos::{XDPoSConsensus, XDPoSConfig};
        
        let chain_id = ctx.chain_spec().chain().id();
        info!(
            chain_id = chain_id,
            "Building XDC consensus"
        );

        // Check if this is an XDC chain
        if is_xdc_chain(chain_id) {
            // Use XDPoS consensus for XDC chains
            let xdpos_config = XDPoSConfig::default();
            let consensus = XDPoSConsensus::new(xdpos_config);
            info!("Using XDPoS consensus for XDC chain");
            Ok(consensus)
        } else {
            // Fall back to Ethereum consensus for non-XDC chains
            info!("Using EthBeaconConsensus for non-XDC chain");
            Ok(Arc::new(reth_ethereum_consensus::EthBeaconConsensus::new(
                ctx.chain_spec(),
            )))
        }
    }
}

/// XDC payload builder (placeholder)
#[derive(Debug, Default, Clone)]
pub struct XdcPayloadBuilder;

/// XDC system contract addresses
pub mod system_contracts {
    use super::*;

    /// Validator contract address (0x88)
    pub const VALIDATOR_CONTRACT: Address = address!("0000000000000000000000000000000000000088");

    /// Block signers contract address (0x89)
    pub const BLOCK_SIGNERS_CONTRACT: Address = address!("0000000000000000000000000000000000000089");

    /// Random contract address (0x90)
    pub const RANDOM_CONTRACT: Address = address!("0000000000000000000000000000000000000090");

    /// Block reward address (0x200)
    pub const BLOCK_REWARD_CONTRACT: Address = address!("0000000000000000000000000000000000000200");
}

/// Helper function to check if running on XDC network
pub fn is_xdc_network(chain_id: u64) -> bool {
    chain_id == 50 || chain_id == 51
}

/// Helper function to get XDC chain name
pub fn xdc_chain_name(chain_id: u64) -> Option<&'static str> {
    match chain_id {
        50 => Some("XDC Mainnet"),
        51 => Some("XDC Apothem"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_xdc_node_creation() {
        let _node = XdcNode::new();
    }

    #[test]
    fn test_is_xdc_network() {
        assert!(is_xdc_network(50));
        assert!(is_xdc_network(51));
        assert!(!is_xdc_network(1));
        assert!(!is_xdc_network(137));
    }

    #[test]
    fn test_xdc_chain_name() {
        assert_eq!(xdc_chain_name(50), Some("XDC Mainnet"));
        assert_eq!(xdc_chain_name(51), Some("XDC Apothem"));
        assert_eq!(xdc_chain_name(1), None);
    }

    #[test]
    fn test_system_contract_addresses() {
        use system_contracts::*;

        assert_eq!(
            VALIDATOR_CONTRACT,
            Address::new([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x88])
        );

        assert_eq!(
            BLOCK_SIGNERS_CONTRACT,
            Address::new([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x89])
        );

        assert_eq!(
            RANDOM_CONTRACT,
            Address::new([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x90])
        );
    }
}
