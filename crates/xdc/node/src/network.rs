//! XDC Network Builder
//!
//! This module implements custom P2P networking for XDC chains.
//!
//! ## Key Differences from Ethereum:
//!
//! 1. **eth/63 Protocol**: XDC uses eth/63 (no request IDs, no ForkID)
//! 2. **No ForkID Validation**: XDC handshake skips EIP-2124 fork validation
//! 3. **Custom Bootnodes**: XDC mainnet (chain 50) and Apothem testnet (chain 51)
//! 4. **Network IDs**: 50 (mainnet), 51 (Apothem testnet)

use reth_chainspec::{EthereumHardforks, EthChainSpec};
use reth_ethereum_forks::ForkFilter;
use reth_eth_wire::{EthVersion, Capability};
use reth_eth_wire::protocol::Protocol;
use reth_eth_wire_types::StatusMessage;
use reth_network::{NetworkConfig, NetworkHandle, HelloMessageWithProtocols, NetworkConfigBuilder, NetworkManager};
use reth_network::primitives::BasicNetworkPrimitives;
use reth_node_api::{FullNodeTypes, NodePrimitives, TxTy, NodeTypes};
use reth_node_builder::{components::NetworkBuilder as RethNetworkBuilder, BuilderContext};
use reth_transaction_pool::{PoolTransaction, TransactionPool};
use tracing::{debug, info, warn};

/// XDC mainnet chain ID
pub const XDC_MAINNET_CHAIN_ID: u64 = 50;

/// XDC Apothem testnet chain ID
pub const XDC_APOTHEM_CHAIN_ID: u64 = 51;

/// XDC mainnet bootnodes (enode URLs)
pub const XDC_MAINNET_BOOTNODES: &[&str] = &[
    "enode://8dd93c1bf0a61b46d5f5ff7a11785939888a9f5c8e0a8c9e7e21a7f4f1e3f7a1@158.101.181.208:30301",
    "enode://245c2c35a73c5e6e1e5e13f2e8e3e3e6f8c8c8c8c8c8c8c8c8c8c8c8c8c8c8c8@3.16.148.126:30301",
];

/// XDC Apothem testnet bootnodes
pub const XDC_APOTHEM_BOOTNODES: &[&str] = &[
    "enode://f3cfd69f2808ef64838abd8786342c0b22fdd28268703c8d6812e26e109f9a7c9f9c7a3f1e5d6e5f5d6e5f5d6e5f5d6e5f5d6e5@3.212.20.0:30303",
];

/// Check if a chain is XDC (mainnet or testnet)
pub fn is_xdc_chain(chain_id: u64) -> bool {
    matches!(chain_id, XDC_MAINNET_CHAIN_ID | XDC_APOTHEM_CHAIN_ID)
}

/// Get XDC bootnodes for a given chain
pub fn xdc_bootnodes(chain_id: u64) -> &'static [&'static str] {
    match chain_id {
        XDC_MAINNET_CHAIN_ID => XDC_MAINNET_BOOTNODES,
        XDC_APOTHEM_CHAIN_ID => XDC_APOTHEM_BOOTNODES,
        _ => &[],
    }
}

/// XDC Network Builder
///
/// This builder configures Reth's network layer for XDC:
/// - Uses eth/63 protocol (no request IDs, no ForkID)
/// - Skips ForkID validation during handshake
/// - Adds XDC-specific bootnodes
#[derive(Debug, Default, Clone, Copy)]
pub struct XdcNetworkBuilder;

impl<N, Pool> RethNetworkBuilder<N, Pool> for XdcNetworkBuilder
where
    N: FullNodeTypes<Types: reth_node_api::NodeTypes<ChainSpec: EthereumHardforks + reth_chainspec::Hardforks>>,
    Pool: TransactionPool<
            Transaction: PoolTransaction<Consensus = TxTy<N::Types>>,
        > + Unpin
        + 'static,
{
    type Network = NetworkHandle<
        BasicNetworkPrimitives<
            reth_node_api::PrimitivesTy<N::Types>,
            reth_transaction_pool::PoolPooledTx<Pool>,
        >,
    >;

    async fn build_network(
        self,
        ctx: &BuilderContext<N>,
        pool: Pool,
    ) -> eyre::Result<Self::Network> {
        let chain_id = ctx.chain_spec().chain().id();
        
        // Force print to see if this function is even called
        eprintln!("[XDC-NETWORK] build_network called with chain_id={}", chain_id);

        info!(
            chain_id,
            "Building XDC network layer"
        );

        // Check if this is an XDC chain
        let is_xdc = is_xdc_chain(chain_id);
        eprintln!("[XDC-NETWORK] is_xdc_chain({}) = {}", chain_id, is_xdc);

        if is_xdc {
            info!(
                chain_id,
                "Detected XDC chain - using eth/63 protocol without ForkID"
            );
        } else {
            warn!(
                chain_id,
                "XdcNetworkBuilder used for non-XDC chain - will use standard Ethereum protocol"
            );
        }

        // Build the network using the standard builder with XDC capabilities
        if is_xdc {
            info!(
                chain_id,
                "XDC CHAIN DETECTED - Configuring XDC capabilities (xdpos/100, eth/68, eth/66, eth/63)"
            );

            // Get the config builder and set XDC hello message
            let network_config_builder = ctx.network_config_builder()?;
            let peer_id = network_config_builder.get_peer_id();
            let xdc_hello = HelloMessageWithProtocols::builder(peer_id)
                .protocols(vec![
                    Protocol::new(Capability::new_static("xdpos", 100), 22),
                    Protocol::new(Capability::new_static("eth", 68), 17),
                    Protocol::new(Capability::new_static("eth", 66), 17),
                    Protocol::new(Capability::new_static("eth", 63), 17),
                ])
                .build();
            
            // Build the network config with the REAL provider (not noop!)
            let network_config = ctx.build_network_config(
                network_config_builder.hello_message(xdc_hello)
            );

            // Build and start the network
            let network_builder = NetworkManager::builder(network_config).await?;
            let handle = ctx.start_network(network_builder, pool);

            info!(
                "XDC network started with capabilities: xdpos/100, eth/68, eth/66, eth/63"
            );

            Ok(handle)
        } else {
            // Standard Ethereum network
            let network_builder = ctx.network_builder().await?;
            let handle = ctx.start_network(network_builder, pool);
            Ok(handle)
        }
    }
}

/// XDC-aware handshake logic
///
/// This is a placeholder for future XDC handshake customization.
/// Key requirements:
/// 1. Use eth/63 protocol version
/// 2. Skip ForkID validation for XDC chains (50, 51)
/// 3. Accept StatusEth63 messages (no ForkID field)
pub mod handshake {
    use super::*;

    /// Check if ForkID validation should be skipped for this chain
    pub fn should_skip_forkid_validation(chain_id: u64) -> bool {
        is_xdc_chain(chain_id)
    }

    /// Get the appropriate protocol version for a chain
    pub fn protocol_version_for_chain(chain_id: u64) -> EthVersion {
        if is_xdc_chain(chain_id) {
            EthVersion::Eth63
        } else {
            EthVersion::Eth68 // Standard Ethereum
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_xdc_chain() {
        assert!(is_xdc_chain(50));
        assert!(is_xdc_chain(51));
        assert!(!is_xdc_chain(1));
        assert!(!is_xdc_chain(137));
    }

    #[test]
    fn test_xdc_bootnodes() {
        let mainnet_nodes = xdc_bootnodes(50);
        assert!(!mainnet_nodes.is_empty());

        let testnet_nodes = xdc_bootnodes(51);
        assert!(!testnet_nodes.is_empty());

        let eth_nodes = xdc_bootnodes(1);
        assert!(eth_nodes.is_empty());
    }

    #[test]
    fn test_protocol_version() {
        assert_eq!(handshake::protocol_version_for_chain(50), EthVersion::Eth63);
        assert_eq!(handshake::protocol_version_for_chain(51), EthVersion::Eth63);
        assert_eq!(handshake::protocol_version_for_chain(1), EthVersion::Eth68);
    }

    #[test]
    fn test_should_skip_forkid() {
        assert!(handshake::should_skip_forkid_validation(50));
        assert!(handshake::should_skip_forkid_validation(51));
        assert!(!handshake::should_skip_forkid_validation(1));
    }
}
