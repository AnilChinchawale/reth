//! XDPoS Voting Snapshot Management
//!
//! Snapshots track the state of authorized signers and voting at specific block heights.
//! They are used to determine who can sign blocks and manage validator voting.

use alloc::collections::{BTreeMap, BTreeSet};
use alloy_primitives::{Address, B256, U256};
use serde::{Deserialize, Serialize};

/// A vote for adding or removing a validator
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vote {
    /// Signer who cast the vote
    pub signer: Address,
    /// Block number when vote was cast
    pub block: u64,
    /// Address being voted on
    pub address: Address,
    /// True to authorize, false to deauthorize
    pub authorize: bool,
}

/// Vote tally for a candidate
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tally {
    /// Whether this is an authorization vote
    pub authorize: bool,
    /// Number of votes received
    pub votes: usize,
}

/// Snapshot is the state of authorization voting at a point in time
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
    /// Block number of the snapshot
    pub number: u64,
    /// Block hash of the snapshot
    pub hash: B256,
    /// Set of authorized signers
    pub signers: BTreeSet<Address>,
    /// Recent signers for anti-spam (block number => signer)
    pub recents: BTreeMap<u64, Address>,
    /// List of recent votes
    pub votes: Vec<Vote>,
    /// Vote tally per candidate
    pub tally: BTreeMap<Address, Tally>,
}

impl Snapshot {
    /// Create a new snapshot with the given signers
    pub fn new(number: u64, hash: B256, signers: Vec<Address>) -> Self {
        Self {
            number,
            hash,
            signers: signers.into_iter().collect(),
            recents: BTreeMap::new(),
            votes: Vec::new(),
            tally: BTreeMap::new(),
        }
    }

    /// Create a genesis snapshot with a single signer
    pub fn genesis(genesis_hash: B256, signers: Vec<Address>) -> Self {
        Self::new(0, genesis_hash, signers)
    }

    /// Check if a signer is authorized
    pub fn is_signer(&self, signer: &Address) -> bool {
        self.signers.contains(signer)
    }

    /// Get the list of signers sorted by address
    pub fn get_signers(&self) -> Vec<Address> {
        self.signers.iter().copied().collect()
    }

    /// Get the number of signers
    pub fn signer_count(&self) -> usize {
        self.signers.len()
    }

    /// Check if a signer is in-turn for a given block number
    /// Uses round-robin selection based on block number
    pub fn inturn(&self, block_number: u64, signer: &Address) -> bool {
        let signers = self.get_signers();
        if signers.is_empty() {
            return false;
        }

        // Find the position of the signer in the sorted list
        if let Some(pos) = signers.iter().position(|s| s == signer) {
            let turn = (block_number as usize) % signers.len();
            pos == turn
        } else {
            false
        }
    }

    /// Get the expected in-turn signer for a block
    pub fn inturn_signer(&self, block_number: u64) -> Option<Address> {
        let signers = self.get_signers();
        if signers.is_empty() {
            return None;
        }
        let turn = (block_number as usize) % signers.len();
        signers.get(turn).copied()
    }

    /// Check if casting a vote would be valid
    pub fn valid_vote(&self, address: &Address, authorize: bool) -> bool {
        let is_signer = self.signers.contains(address);
        (is_signer && !authorize) || (!is_signer && authorize)
    }

    /// Cast a vote
    pub fn cast_vote(&mut self, address: Address, authorize: bool) -> bool {
        if !self.valid_vote(&address, authorize) {
            return false;
        }

        if let Some(tally) = self.tally.get_mut(&address) {
            tally.votes += 1;
        } else {
            self.tally.insert(address, Tally { authorize, votes: 1 });
        }
        true
    }

    /// Uncast a vote (remove a vote)
    pub fn uncast_vote(&mut self, address: &Address, authorize: bool) -> bool {
        if let Some(tally) = self.tally.get_mut(address) {
            if tally.authorize != authorize {
                return false;
            }
            if tally.votes > 1 {
                tally.votes -= 1;
            } else {
                self.tally.remove(address);
            }
            true
        } else {
            false
        }
    }

    /// Apply votes and update signers based on majority threshold
    /// Returns true if signers were modified
    pub fn apply_votes(&mut self) -> bool {
        let threshold = self.signers.len() / 2 + 1;
        let mut modified = false;
        let mut to_add = Vec::new();
        let mut to_remove = Vec::new();

        // Find addresses that have reached the threshold
        for (address, tally) in &self.tally {
            if tally.votes >= threshold {
                if tally.authorize {
                    to_add.push(*address);
                } else {
                    to_remove.push(*address);
                }
            }
        }

        // Apply changes
        for address in to_add {
            if self.signers.insert(address) {
                modified = true;
                // Remove all votes for this address
                self.tally.remove(&address);
                self.votes.retain(|v| v.address != address);
            }
        }

        for address in to_remove {
            if self.signers.remove(&address) {
                modified = true;
                // Remove all votes for this address
                self.tally.remove(&address);
                self.votes.retain(|v| v.address != address);
                // Remove votes by this signer
                self.votes.retain(|v| v.signer != address);
            }
        }

        modified
    }

    /// Add a recent signer
    pub fn add_recent(&mut self, block_number: u64, signer: Address) {
        self.recents.insert(block_number, signer);

        // Limit recents to avoid unbounded growth
        // Keep only the last epoch worth of recents
        let limit = self.signers.len() as u64;
        if block_number >= limit {
            self.recents.retain(|bn, _| *bn > block_number - limit);
        }
    }

    /// Check if a signer has recently signed (anti-spam)
    pub fn recently_signed(&self, block_number: u64, signer: &Address) -> bool {
        let limit = self.signers.len() as u64;
        let min_block = if block_number >= limit {
            block_number - limit
        } else {
            0
        };

        self.recents
            .iter()
            .filter(|(bn, _)| **bn >= min_block && **bn < block_number)
            .any(|(_, s)| s == signer)
    }

    /// Update the snapshot with a new checkpoint (clears votes)
    pub fn apply_checkpoint(
        &mut self,
        number: u64,
        hash: B256,
        new_signers: Vec<Address>,
    ) {
        self.number = number;
        self.hash = hash;
        self.signers = new_signers.into_iter().collect();
        self.votes.clear();
        self.tally.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_signers() -> Vec<Address> {
        vec![
            Address::with_last_byte(1),
            Address::with_last_byte(2),
            Address::with_last_byte(3),
        ]
    }

    #[test]
    fn test_new_snapshot() {
        let signers = test_signers();
        let snap = Snapshot::new(100, B256::with_last_byte(1), signers.clone());

        assert_eq!(snap.number, 100);
        assert_eq!(snap.signer_count(), 3);
        assert!(snap.is_signer(&Address::with_last_byte(1)));
    }

    #[test]
    fn test_inturn() {
        let signers = test_signers();
        let snap = Snapshot::new(0, B256::ZERO, signers);

        // Block 0: signer 0 is in-turn
        assert!(snap.inturn(0, &Address::with_last_byte(1)));
        assert!(!snap.inturn(0, &Address::with_last_byte(2)));

        // Block 1: signer 1 is in-turn
        assert!(!snap.inturn(1, &Address::with_last_byte(1)));
        assert!(snap.inturn(1, &Address::with_last_byte(2)));

        // Block 3: wraps around to signer 0
        assert!(snap.inturn(3, &Address::with_last_byte(1)));
    }

    #[test]
    fn test_voting() {
        let signers = test_signers();
        let mut snap = Snapshot::new(0, B256::ZERO, signers);

        let new_signer = Address::with_last_byte(4);

        // Cast votes to add new signer
        assert!(snap.cast_vote(new_signer, true));
        assert!(snap.cast_vote(new_signer, true));
        // Need 2 votes for majority with 3 signers (3/2 + 1 = 2)

        // Apply votes
        snap.apply_votes();

        // New signer should now be authorized
        assert!(snap.is_signer(&new_signer));
    }

    #[test]
    fn test_recent_signers() {
        let signers = test_signers();
        let mut snap = Snapshot::new(0, B256::ZERO, signers);

        snap.add_recent(1, Address::with_last_byte(1));
        snap.add_recent(2, Address::with_last_byte(2));

        assert!(snap.recently_signed(3, &Address::with_last_byte(1)));
        assert!(!snap.recently_signed(10, &Address::with_last_byte(99)));
    }
}
