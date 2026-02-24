//! XDC Network Node Implementation
//!
//! This crate provides the XDC Network node implementation for Reth,
//! integrating XDPoS consensus (V1+V2), custom EVM configuration,
//! and XDC-specific execution logic.
//!
//! ## Architecture
//!
//! - [`XdcNode`] - Main node type implementing Reth's `NodeTypes` and `Node` traits
//! - [`XdcNodeComponents`] - Component builder for consensus, executor, network
//! - [`XdcEvmConfig`] - XDC EVM configuration (EIP-158 disabled, TIPSigning)
//! - [`XdcExecutorProvider`] - Block executor with reward application
//! - [`XdcPayloadBuilder`] - Validator block building

#![doc(
    html_logo_url = "https://raw.githubusercontent.com/paradigmxyz/reth/main/assets/reth-docs.png",
    html_favicon_url = "https://avatars0.githubusercontent.com/u/97369466?s=256",
    issue_tracker_base_url = "https://github.com/paradigmxyz/reth/issues/"
)]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![cfg_attr(docsrs, feature(doc_cfg))]

use alloy_primitives::{address, Address, U256};
use reth_chainspec::ChainSpec;
use reth_consensus::Consensus;
use reth_consensus_xdpos::{XdcStateRootCache, XDPoSConfig, XDPoSConsensus};
use reth_evm::execute::BlockExecutorProvider;
use reth_evm_ethereum::execute::EthExecutorProvider;
use reth_network::{NetworkBuilder, NetworkConfig, NetworkManager};
use reth_node_builder::{
    components::{ConsensusBuilder, ExecutorBuilder, NetworkBuilder as RethNetworkBuilder},
    BuilderContext, FullNodeTypes, Node, NodeTypes,
};
use reth_payload_builder::PayloadBuilderService;
use reth_primitives::Header;
use reth_provider::CanonStateSubscriptions;
use reth_transaction_pool::TransactionPool;
use std::sync::Arc;
use tracing::{debug, info};

pub mod evm;
pub mod executor;
pub mod payload;

use evm::XdcEvmConfig;
use executor::XdcExecutorProvider;

/// XDC Network node
///
/// This is the main node type for XDC Network. It implements Reth's
/// `NodeTypes` and `Node` traits to provide a complete XDC node.
///
/// # Example
///
/// ```rust,ignore
/// use reth_xdc_node::XdcNode;
///
/// let node = XdcNode::new();
/// ```
#[derive(Debug, Clone, Default)]
pub struct XdcNode;

impl XdcNode {
    /// Create a new XDC node
    pub fn new() -> Self {
        Self
    }
}

impl NodeTypes for XdcNode {
    type Primitives = reth_primitives::EthPrimitives;
    type ChainSpec = ChainSpec;
    type Payload = reth_primitives::Block;

    fn chain_spec(&self) -> Arc<Self::ChainSpec> {
        // Return XDC mainnet by default
        // This is overridden by CLI arguments
        reth_chainspec::xdc::XDC_MAINNET.clone()
    }
}

impl<N> Node<N> for XdcNode
where
    N: FullNodeTypes<
        Types: NodeTypes<
            ChainSpec = ChainSpec,
            Primitives = reth_primitives::EthPrimitives,
        >,
    >,
{
    type ComponentsBuilder = XdcNodeComponents;

    fn components_builder(&self) -> Self::ComponentsBuilder {
        XdcNodeComponents::default()
    }

    fn add_ons<EVM>(&self) -> reth_node_builder::rpc::RpcAddOns<N, EVM, ()>
    where
        EVM: reth_evm::ConfigureEvm<Header = Header> + 'static,
    {
        reth_node_builder::rpc::RpcAddOns::default()
    }
}

/// XDC node component builder
///
/// Builds the custom components for XDC node:
/// - XDPoS consensus engine
/// - XDC executor with reward application
/// - XDC EVM configuration
/// - Custom network configuration
#[derive(Debug, Clone, Default)]
pub struct XdcNodeComponents;

impl<N> ConsensusBuilder<N> for XdcNodeComponents
where
    N: FullNodeTypes<Types: NodeTypes<ChainSpec = ChainSpec>>,
{
    type Consensus = Arc<XDPoSConsensus>;

    async fn build_consensus(
        self,
        ctx: &BuilderContext<N>,
    ) -> eyre::Result<Self::Consensus> {
        info!(
            chain_id = ctx.chain_spec().chain.id(),
            "Building XDPoS consensus engine"
        );

        // Get chain ID for V2 switch configuration
        let chain_id = ctx.chain_spec().chain.id();

        // Get V2 switch block from chain spec
        let v2_switch = reth_chainspec::xdc::v2_switch_block(chain_id).unwrap_or(u64::MAX);

        // Create XDPoS configuration
        let config = XDPoSConfig {
            epoch: 900,
            period: 2,
            gap: 450,
            v2_config: Some(reth_consensus_xdpos::V2Config {
                v2_switch_block: v2_switch,
                ..Default::default()
            }),
        };

        // Create consensus engine
        let consensus = XDPoSConsensus::new(config, ctx.chain_spec().clone());

        debug!("XDPoS consensus engine initialized");

        Ok(Arc::new(consensus))
    }
}

impl<N> ExecutorBuilder<N> for XdcNodeComponents
where
    N: FullNodeTypes<
        Types: NodeTypes<
            ChainSpec = ChainSpec,
            Primitives = reth_primitives::EthPrimitives,
        >,
    >,
{
    type EVM = XdcEvmConfig;
    type Executor = XdcExecutorProvider;

    async fn build_evm(
        self,
        ctx: &BuilderContext<N>,
    ) -> eyre::Result<Self::EVM> {
        info!(
            chain_id = ctx.chain_spec().chain.id(),
            "Building XDC EVM configuration"
        );

        Ok(XdcEvmConfig::new(ctx.chain_spec()))
    }

    async fn build_executor(
        self,
        ctx: &BuilderContext<N>,
    ) -> eyre::Result<Self::Executor> {
        info!(
            chain_id = ctx.chain_spec().chain.id(),
            "Building XDC block executor"
        );

        // Build consensus for executor
        let consensus = XDPoSConsensus::new(
            XDPoSConfig::default(),
            ctx.chain_spec(),
        );

        // Initialize state root cache
        let cache = XdcStateRootCache::new(
            1000, // max blocks cached
            50,   // max validators cached
            std::time::Duration::from_secs(300), // cache ttl
        );

        Ok(XdcExecutorProvider::new(
            ctx.chain_spec(),
            Arc::new(consensus),
            Arc::new(cache),
        ))
    }
}

impl<N> RethNetworkBuilder<N> for XdcNodeComponents
where
    N: FullNodeTypes<
        Types: NodeTypes<
            ChainSpec = ChainSpec,
            Primitives = reth_primitives::EthPrimitives,
        >,
    >,
{
    type Network = NetworkManager;

    async fn build_network(
        self,
        ctx: &BuilderContext<N>,
    ) -> eyre::Result<Self::Network> {
        info!(
            chain_id = ctx.chain_spec().chain.id(),
            "Building XDC network"
        );

        // Get chain ID for bootnodes
        let chain_id = ctx.chain_spec().chain.id();

        // Get XDC bootnodes based on chain
        let bootnodes = if chain_id == 50 {
            reth_chainspec::xdc::xdc_mainnet_bootnodes()
        } else if chain_id == 51 {
            reth_chainspec::xdc::xdc_apothem_bootnodes()
        } else {
            vec![]
        };

        // Build network configuration
        // eth/63 for block sync
        // eth/100 for XDPoS voting (future)
        let config = NetworkConfig::builder(ctx.secret_key())
            .boot_nodes(bootnodes)
            .chain_spec(ctx.chain_spec())
            .build(ctx.config().network);

        // Create and start network manager
        let network = NetworkManager::new(config).await?;

        debug!("XDC network initialized");

        Ok(network)
    }
}

/// XDC chain specification parser
///
/// Parses chain specification from CLI arguments
#[derive(Debug, Clone)]
pub struct XdcChainSpecParser;

impl reth_cli::chainspec::ChainSpecParser for XdcChainSpecParser {
    type ChainSpec = ChainSpec;

    fn parse(&self, s: &str) -> eyre::Result<Arc<Self::ChainSpec>> {
        match s {
            "xdc" | "mainnet" | "50" => {
                info!("Using XDC Mainnet chain spec");
                Ok(reth_chainspec::xdc::XDC_MAINNET.clone())
            }
            "apothem" | "testnet" | "51" => {
                info!("Using XDC Apothem Testnet chain spec");
                Ok(reth_chainspec::xdc::XDC_APOTHEM.clone())
            }
            path if std::path::Path::new(path).exists() => {
                info!("Loading XDC chain spec from file: {}", path);
                // Load from JSON file
                let contents = std::fs::read_to_string(path)?;
                let spec: ChainSpec = serde_json::from_str(&contents)?;
                Ok(Arc::new(spec))
            }
            _ => Err(eyre::eyre!(
                "Unknown chain spec: {}. Use 'xdc', 'apothem', or a path to a JSON file",
                s
            )),
        }
    }

    fn supported_chains(&self) -> Vec<reth_cli::chainspec::SupportedChain> {
        vec![
            reth_cli::chainspec::SupportedChain {
                name: "xdc",
                aliases: vec!["mainnet", "50"],
                description: "XDC Network Mainnet",
            },
            reth_cli::chainspec::SupportedChain {
                name: "apothem",
                aliases: vec!["testnet", "51"],
                description: "XDC Apothem Testnet",
            },
        ]
    }
}

/// XDC system contract addresses
pub mod system_contracts {
    use super::*;

    /// Validator contract address (0x88)
    pub const VALIDATOR_CONTRACT: Address = address!(0x0000000000000000000000000000000000000088);

    /// Block signers contract address (0x89)
    pub const BLOCK_SIGNERS_CONTRACT: Address = address!(0x0000000000000000000000000000000000000089);

    /// Random contract address (0x90)
    pub const RANDOM_CONTRACT: Address = address!(0x0000000000000000000000000000000000000090);

    /// Block reward address (0x200)
    pub const BLOCK_REWARD_CONTRACT: Address = address!(0x0000000000000000000000000000000000000200);
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
        let node = XdcNode::new();
        assert_eq!(node.chain_spec().chain.id(), 50); // Default to mainnet
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
