//! XDPoS V2 BFT Consensus
//!
//! This module implements the complete XDPoS V2 BFT consensus including:
//! - Round-based consensus with round numbers
//! - BlockInfo for BFT messages
//! - Quorum Certificates (QC) - proof of 2/3+1 validator agreement
//! - Timeout Certificates (TC) - proof of 2/3+1 validator timeout
//! - V2-specific extra field handling with RLP encoding
//! - Signature verification for QC/TC
//! - Proposer selection (round-robin)

pub mod engine;
pub mod proposer;
pub mod types;
pub mod verification;

// Re-export main engine
pub use engine::XDPoSV2Engine;

use alloc::vec::Vec;
use alloy_primitives::{Address, B256};
use serde::{Deserialize, Serialize};

/// Round number type for V2 consensus
pub type Round = u64;

/// Signature type for BFT messages (65 bytes)
pub type Signature = Vec<u8>;

/// BlockInfo contains metadata about a block for BFT messages
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub struct BlockInfo {
    /// Block hash
    pub hash: B256,
    /// Consensus round
    pub round: Round,
    /// Block number
    pub number: u64,
}

impl BlockInfo {
    /// Create a new BlockInfo
    pub fn new(hash: B256, round: Round, number: u64) -> Self {
        Self { hash, round, number }
    }
}

/// Quorum Certificate (QC) represents 2/3 majority consensus
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuorumCert {
    /// Information about the proposed block
    pub proposed_block_info: BlockInfo,
    /// Signatures from validators
    pub signatures: Vec<Signature>,
    /// Gap number for epoch tracking
    pub gap_number: u64,
}

impl QuorumCert {
    /// Create a new QuorumCert
    pub fn new(proposed_block_info: BlockInfo, gap_number: u64) -> Self {
        Self {
            proposed_block_info,
            signatures: Vec::new(),
            gap_number,
        }
    }

    /// Add a signature
    pub fn add_signature(&mut self, signature: Signature) {
        self.signatures.push(signature);
    }

    /// Get the signature count
    pub fn signature_count(&self) -> usize {
        self.signatures.len()
    }
}

/// Timeout Certificate (TC) represents 2/3 timeout consensus
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimeoutCert {
    /// Round that timed out
    pub round: Round,
    /// Signatures from validators that timed out
    pub signatures: Vec<Signature>,
    /// Gap number for epoch tracking
    pub gap_number: u64,
}

impl TimeoutCert {
    /// Create a new TimeoutCert
    pub fn new(round: Round, gap_number: u64) -> Self {
        Self {
            round,
            signatures: Vec::new(),
            gap_number,
        }
    }

    /// Add a signature
    pub fn add_signature(&mut self, signature: Signature) {
        self.signatures.push(signature);
    }
}

/// SyncInfo is used to sync consensus state between nodes
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncInfo {
    /// Highest known quorum certificate
    pub highest_quorum_cert: QuorumCert,
    /// Highest known timeout certificate (if any)
    pub highest_timeout_cert: Option<TimeoutCert>,
}

/// ExtraFieldsV2 contains parsed V2 extra data from block headers
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtraFieldsV2 {
    /// Consensus round
    pub round: Round,
    /// Quorum certificate (None for switch block)
    pub quorum_cert: Option<QuorumCert>,
}

impl ExtraFieldsV2 {
    /// Create new extra fields
    pub fn new(round: Round, quorum_cert: Option<QuorumCert>) -> Self {
        Self { round, quorum_cert }
    }
}

/// Vote message structure
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vote {
    /// Proposed block information
    pub proposed_block_info: BlockInfo,
    /// Signature
    pub signature: Signature,
    /// Gap number
    pub gap_number: u64,
    /// Signer address (recovered from signature)
    #[serde(skip)]
    signer: Option<Address>,
}

impl Vote {
    /// Create a new vote
    pub fn new(proposed_block_info: BlockInfo, signature: Signature, gap_number: u64) -> Self {
        Self {
            proposed_block_info,
            signature,
            gap_number,
            signer: None,
        }
    }

    /// Set the signer
    pub fn set_signer(&mut self, signer: Address) {
        self.signer = Some(signer);
    }

    /// Get the signer
    pub fn signer(&self) -> Option<Address> {
        self.signer
    }

    /// Generate pool key for grouping votes
    pub fn pool_key(&self) -> alloc::string::String {
        alloc::format!(
            "{}:{}:{}:{:?}",
            self.proposed_block_info.round,
            self.gap_number,
            self.proposed_block_info.number,
            self.proposed_block_info.hash
        )
    }
}

/// Timeout message structure
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Timeout {
    /// Round that timed out
    pub round: Round,
    /// Signature
    pub signature: Signature,
    /// Gap number
    pub gap_number: u64,
    /// Signer address (recovered from signature)
    #[serde(skip)]
    signer: Option<Address>,
}

impl Timeout {
    /// Create a new timeout
    pub fn new(round: Round, signature: Signature, gap_number: u64) -> Self {
        Self {
            round,
            signature,
            gap_number,
            signer: None,
        }
    }

    /// Set the signer
    pub fn set_signer(&mut self, signer: Address) {
        self.signer = Some(signer);
    }

    /// Get the signer
    pub fn signer(&self) -> Option<Address> {
        self.signer
    }

    /// Generate pool key for grouping timeouts
    pub fn pool_key(&self) -> alloc::string::String {
        alloc::format!("{}:{}", self.round, self.gap_number)
    }
}

/// VoteForSign is the structure used to generate vote signature hash
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VoteForSign {
    /// Proposed block information
    pub proposed_block_info: BlockInfo,
    /// Gap number
    pub gap_number: u64,
}

/// TimeoutForSign is the structure used to generate timeout signature hash
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimeoutForSign {
    /// Round
    pub round: Round,
    /// Gap number
    pub gap_number: u64,
}

/// EpochSwitchInfo contains information about epoch boundaries
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EpochSwitchInfo {
    /// List of masternode addresses
    pub masternodes: Vec<Address>,
    /// Number of masternodes
    pub masternodes_len: usize,
    /// Block info for epoch switch block
    pub epoch_switch_block_info: BlockInfo,
    /// Block info for parent of epoch switch block
    pub epoch_switch_parent_block_info: BlockInfo,
    /// Penalties for this epoch
    pub penalties: Vec<Address>,
    /// Standby nodes
    pub standby_nodes: Vec<Address>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_block_info() {
        let info = BlockInfo::new(B256::with_last_byte(1), 100, 1000);
        assert_eq!(info.round, 100);
        assert_eq!(info.number, 1000);
    }

    #[test]
    fn test_quorum_cert() {
        let block_info = BlockInfo::new(B256::with_last_byte(1), 100, 1000);
        let mut qc = QuorumCert::new(block_info, 500);

        assert_eq!(qc.signature_count(), 0);
        qc.add_signature(vec![1, 2, 3]);
        assert_eq!(qc.signature_count(), 1);
    }

    #[test]
    fn test_vote_pool_key() {
        let block_info = BlockInfo::new(B256::with_last_byte(1), 100, 1000);
        let vote = Vote::new(block_info, vec![1, 2, 3], 500);

        let key = vote.pool_key();
        assert!(key.contains("100")); // round
        assert!(key.contains("500")); // gap_number
        assert!(key.contains("1000")); // number
    }
}
