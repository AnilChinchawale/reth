//! XDC-specific primitive types for Reth.
//!
//! This crate provides XDC-specific implementations of block headers and other
//! primitives needed for XDC network compatibility.

#![doc(
    html_logo_url = "https://raw.githubusercontent.com/paradigmxyz/reth/main/assets/reth-docs.png",
    html_favicon_url = "https://avatars0.githubusercontent.com/u/97369466?s=256",
    issue_tracker_base_url = "https://github.com/paradigmxyz/reth/issues/"
)]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![cfg_attr(not(feature = "std"), no_std)]

mod header;
pub use header::XdcBlockHeader;

pub use alloy_primitives::{Address, BlockHash, Bloom, Bytes, B256, B64, U256};

// Re-export Ethereum transaction types (XDC uses same transaction format)
pub use reth_ethereum_primitives::{
    PooledTransactionVariant, Receipt, Transaction, TransactionSigned,
};

/// XDC-specific block body type using XdcBlockHeader for ommers
pub type XdcBlockBody = alloy_consensus::BlockBody<TransactionSigned, XdcBlockHeader>;

/// XDC-specific block type using XdcBlockHeader
pub type XdcBlock = alloy_consensus::Block<TransactionSigned, XdcBlockHeader>;

/// Helper struct that specifies XDC [`NodePrimitives`](reth_primitives_traits::NodePrimitives).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct XdcPrimitives;

impl reth_primitives_traits::NodePrimitives for XdcPrimitives {
    type Block = XdcBlock;
    type BlockHeader = XdcBlockHeader;
    type BlockBody = XdcBlockBody;
    type SignedTx = TransactionSigned;
    type Receipt = Receipt;
}

#[cfg(feature = "reth-eth-wire-types")]
pub use network::XdcNetworkPrimitives;

/// Network primitive types for XDC.
#[cfg(feature = "reth-eth-wire-types")]
mod network {
    use super::*;
    
    /// Network primitive types used by XDC networks.
    pub type XdcNetworkPrimitives = 
        reth_eth_wire_types::BasicNetworkPrimitives<XdcPrimitives, PooledTransactionVariant>;
}
