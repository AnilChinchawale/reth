//! XDC Network XDPoS Consensus Implementation
//!
//! This crate implements the XDPoS (XDC Delegated Proof of Stake) consensus
//! algorithm used by the XDC Network. It supports both V1 (epoch-based) and
//! V2 (BFT with QC/TC) consensus mechanisms.
//!
//! ## Architecture
//!
//! - [`XDPoSConsensus`] - Main consensus engine implementing Reth's `Consensus` trait
//! - [`XDPoSConfig`] - Configuration for XDPoS parameters
//! - [`Snapshot`] - Voting snapshot for validator management
//! - V2 sub-module - BFT consensus with Quorum Certificates

#![doc(
    html_logo_url = "https://raw.githubusercontent.com/paradigmxyz/reth/main/assets/reth-docs.png",
    html_favicon_url = "https://avatars0.githubusercontent.com/u/97369466?s=256",
    issue_tracker_base_url = "https://github.com/paradigmxyz/reth/issues/"
)]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![cfg_attr(docsrs, feature(doc_cfg))]

extern crate alloc;

use alloc::sync::Arc;

mod config;
mod errors;
mod extra_data;
mod reward;
mod snapshot;
pub mod special_tx;
mod validation;
mod v1;
mod v2;
mod xdpos;

pub use config::{V2Config, XDPoSConfig};
pub use errors::XDPoSError;
pub use extra_data::{hash_without_seal, recover_signer, V1ExtraData};
pub use snapshot::{Snapshot, Tally, Vote};
pub use xdpos::XDPoSConsensus;

pub use v2::{
    BlockInfo, ExtraFieldsV2, QuorumCert, Round, Signature, TimeoutCert, XDPoSV2Engine,
};

/// Extra field constants for XDPoS
pub mod constants {
    /// Fixed number of extra-data prefix bytes reserved for signer vanity
    pub const EXTRA_VANITY: usize = 32;

    /// Fixed number of extra-data suffix bytes reserved for signer seal
    pub const EXTRA_SEAL: usize = 65;

    /// Default epoch length (900 blocks)
    pub const DEFAULT_EPOCH: u64 = 900;

    /// Default block period in seconds (2 seconds)
    pub const DEFAULT_PERIOD: u64 = 2;

    /// Default gap before epoch switch (450 blocks)
    pub const DEFAULT_GAP: u64 = 450;

    /// Difficulty for in-turn signatures
    pub const DIFF_IN_TURN: u64 = 2;

    /// Difficulty for out-of-turn signatures
    pub const DIFF_NO_TURN: u64 = 1;

    /// Number of recent vote snapshots to keep in memory
    pub const INMEMORY_SNAPSHOTS: usize = 128;

    /// Number of recent block signatures to keep in memory
    pub const INMEMORY_SIGNATURES: usize = 4096;

    /// Cache limit for block signers
    pub const BLOCK_SIGNERS_CACHE_LIMIT: usize = 10_000_000;

    /// XDC Validator Contract Address (0x88)
    pub const VALIDATOR_CONTRACT_ADDR: &str = "0x0000000000000000000000000000000000000088";

    /// XDC Block Signers Contract Address (0x89)
    pub const BLOCK_SIGNERS_CONTRACT_ADDR: &str = "0x0000000000000000000000000000000000000089";

    /// Default certificate threshold for V2 (2/3 = 67%)
    pub const DEFAULT_CERT_THRESHOLD: f64 = 0.667;

    /// XDC Mainnet V2 Switch Block
    pub const XDC_MAINNET_V2_SWITCH: u64 = 56_857_600;

    /// XDC Mainnet Chain ID
    pub const XDC_MAINNET_CHAIN_ID: u64 = 50;

    /// XDC Apothem Testnet Chain ID
    pub const XDC_APOTHEM_CHAIN_ID: u64 = 51;
}

/// Helper function to calculate if a block is at epoch boundary
pub fn is_epoch_switch(block_number: u64, epoch: u64) -> bool {
    block_number % epoch == 0
}

/// Helper function to calculate the epoch number for a block
pub fn epoch_number(block_number: u64, epoch: u64) -> u64 {
    block_number / epoch
}

/// Helper function to get the epoch start block
pub fn epoch_start_block(epoch: u64, epoch_length: u64) -> u64 {
    epoch * epoch_length
}

/// Helper function to calculate gap number for V2 QC
pub fn calculate_gap_number(epoch_switch_number: u64, epoch: u64, gap: u64) -> u64 {
    if epoch_switch_number <= gap {
        0
    } else {
        epoch_switch_number - gap
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_epoch_switch() {
        assert!(is_epoch_switch(0, 900));
        assert!(is_epoch_switch(900, 900));
        assert!(is_epoch_switch(1800, 900));
        assert!(!is_epoch_switch(1, 900));
        assert!(!is_epoch_switch(899, 900));
    }

    #[test]
    fn test_epoch_number() {
        assert_eq!(epoch_number(0, 900), 0);
        assert_eq!(epoch_number(899, 900), 0);
        assert_eq!(epoch_number(900, 900), 1);
        assert_eq!(epoch_number(56857600, 900), 63175);
    }

    #[test]
    fn test_calculate_gap_number() {
        assert_eq!(calculate_gap_number(900, 900, 450), 450);
        assert_eq!(calculate_gap_number(1800, 900, 450), 1350);
        assert_eq!(calculate_gap_number(450, 900, 450), 0);
    }
}
