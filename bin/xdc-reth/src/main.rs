//! XDC Network Reth Node
//!
//! This is the main entry point for the XDC Network Reth node.
//! It parses CLI arguments, loads the appropriate chain specification,
//! configures the XDPoS consensus engine, and starts the node.
//!
//! ## Usage
//!
//! ```bash
//! # Run XDC Mainnet
//! xdc-reth node --chain xdc
//!
//! # Run XDC Apothem Testnet
//! xdc-reth node --chain apothem
//!
//! # Run with custom config
//! xdc-reth node --chain /path/to/chainspec.json
//! ```

use alloy_primitives::U256;
use clap::Parser;
use reth_chainspec::xdc::{
    is_xdc_chain, xdc_apothem_bootnodes, xdc_mainnet_bootnodes, XDC_APOTHEM, XDC_MAINNET,
};
use reth_cli::chainspec::ChainSpecParser;
use reth_cli_runner::CliRunner;
use reth_consensus_xdpos::{XdcStateRootCache, XDPoSConfig, XDPoSConsensus};
use reth_node_builder::{
    NodeBuilder, NodeConfig, NodeHandle,
};
use reth_xdc_node::{XdcChainSpecParser, XdcNode};
use std::sync::Arc;
use tracing::{info, warn};

/// XDC Network Reth CLI
#[derive(Debug, Parser)]
#[command(name = "xdc-reth")]
#[command(about = "XDC Network Reth - XDPoS consensus node")]
struct Cli {
    #[command(flatten)]
    node: NodeArgs,
}

/// Node-specific arguments
#[derive(Debug, Parser)]
struct NodeArgs {
    /// Chain specification to use (xdc, apothem, or path to JSON)
    #[arg(long, value_name = "CHAIN", default_value = "xdc")]
    chain: String,

    /// Enable validator mode (participate in consensus)
    #[arg(long)]
    validator: bool,

    /// Masternode private key (for validator mode)
    #[arg(long, env = "XDC_MN_SECRET_KEY")]
    masternode_key: Option<String>,

    /// Disable state root caching (for debugging)
    #[arg(long)]
    no_state_cache: bool,

    /// Maximum state root cache size
    #[arg(long, default_value = "1000")]
    cache_size: usize,
}

/// XDC Node Launcher
struct XdcNodeLauncher {
    chain_id: u64,
    is_validator: bool,
    state_cache: Option<Arc<XdcStateRootCache>>,
}

impl XdcNodeLauncher {
    fn new(chain_id: u64, is_validator: bool, cache_size: usize) -> Self {
        let state_cache = if is_xdc_chain(chain_id) {
            info!(
                chain_id,
                cache_size,
                "Initializing XDC state root cache"
            );
            Some(Arc::new(XdcStateRootCache::new(
                cache_size,
                50,
                std::time::Duration::from_secs(300),
            )))
        } else {
            warn!(chain_id, "Not an XDC chain, state root cache disabled");
            None
        };

        Self {
            chain_id,
            is_validator,
            state_cache,
        }
    }

    /// Get chain name
    fn chain_name(&self) -> &'static str {
        match self.chain_id {
            50 => "XDC Mainnet",
            51 => "XDC Apothem Testnet",
            _ => "Unknown",
        }
    }

    /// Get V2 switch block for this chain
    fn v2_switch_block(&self) -> Option<u64> {
        reth_chainspec::xdc::v2_switch_block(self.chain_id)
    }

    /// Print startup banner
    fn print_banner(&self) {
        println!(r#"
╔════════════════════════════════════════════════════════════╗
║                                                            ║
║                 XDC Network Reth Node                      ║
║                                                            ║
║  Chain:    {:48}║
║  Chain ID: {:<48}║
║  Mode:     {:48}║
║                                                            ║
╚════════════════════════════════════════════════════════════╝
        "#, self.chain_name(), self.chain_id,
            if self.is_validator { "Validator" } else { "Archive" });
    }
}

#[tokio::main]
async fn main() -> eyre::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    info!("Starting XDC Network Reth node");

    // Parse CLI arguments
    let cli = Cli::parse();

    // Parse chain spec
    let chain_spec = XdcChainSpecParser.parse(&cli.node.chain)?;
    let chain_id = chain_spec.chain.id();

    // Validate this is an XDC chain
    if !is_xdc_chain(chain_id) {
        warn!(
            chain_id,
            "Running on non-XDC chain. XDPoS consensus may not work correctly."
        );
    }

    // Print startup banner
    let launcher = XdcNodeLauncher::new(chain_id, cli.node.validator, cli.node.cache_size);
    launcher.print_banner();

    // Log V2 consensus info
    if let Some(v2_switch) = launcher.v2_switch_block() {
        info!(v2_switch, "XDPoS V2 consensus will activate at this block");
    } else {
        warn!("No V2 switch block configured for this chain");
    }

    // Create CLI runner
    let runner = CliRunner::try_default_runtime()?;

    // Create node
    let node = XdcNode::new();

    // Parse arguments and run
    let builder = NodeBuilder::new(chain_spec)
        .with_config_and_database_path(None, None)
        .await?;

    // Launch the node
    let NodeHandle { node: node_handle, node_exit_future } = builder
        .node(node)
        .launch()
        .await?;

    info!(
        local_addr = ?node_handle.network().local_address(),
        peer_id = %node_handle.network().peer_id(),
        "XDC node launched successfully"
    );

    // Add validator-specific initialization if needed
    if cli.node.validator {
        info!("Validator mode enabled");

        // TODO: Initialize validator state
        // - Load masternode key
        // - Connect to validator network
        // - Start block production when in-turn

        if let Some(key) = &cli.node.masternode_key {
            info!("Using provided masternode key");
            // In production, validate key and register as validator
            // For now, just log
            debug!("Masternode key configured");
        } else {
            warn!("Validator mode enabled but no masternode key provided");
            warn!("Set XDC_MN_SECRET_KEY environment variable or use --masternode-key");
        }
    }

    // Wait for node to exit
    info!("XDC node running. Press Ctrl+C to stop.");
    node_exit_future.await?;

    info!("XDC node shutdown complete");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chain_id_detection() {
        assert!(is_xdc_chain(50));
        assert!(is_xdc_chain(51));
        assert!(!is_xdc_chain(1));
        assert!(!is_xdc_chain(137));
    }

    #[test]
    fn test_launcher_chain_name() {
        let launcher = XdcNodeLauncher::new(50, false, 1000);
        assert_eq!(launcher.chain_name(), "XDC Mainnet");

        let launcher = XdcNodeLauncher::new(51, false, 1000);
        assert_eq!(launcher.chain_name(), "XDC Apothem Testnet");

        let launcher = XdcNodeLauncher::new(1, false, 1000);
        assert_eq!(launcher.chain_name(), "Unknown");
    }

    #[test]
    fn test_v2_switch_blocks() {
        let mainnet = XdcNodeLauncher::new(50, false, 1000);
        assert_eq!(mainnet.v2_switch_block(), Some(56_857_600));

        let apothem = XdcNodeLauncher::new(51, false, 1000);
        assert_eq!(apothem.v2_switch_block(), Some(23_556_600));

        let unknown = XdcNodeLauncher::new(1, false, 1000);
        assert_eq!(unknown.v2_switch_block(), None);
    }
}
