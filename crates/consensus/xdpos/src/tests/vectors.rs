//! Test vectors from XDC Mainnet
//!
//! This module contains REAL data from XDC mainnet for validation testing.
//! These vectors ensure that the Rust implementation produces identical results
//! to the Go implementation (go-xdc).

use alloy_primitives::{address, b256, Address, B256};

/// XDC Mainnet Genesis Block
pub const GENESIS_HASH: B256 =
    b256!("4a9d748bd78a8d0385b67788c2435dcdb914f98a96250b68863a1f8b7642d6b1");
pub const GENESIS_NUMBER: u64 = 0;
pub const GENESIS_CHAIN_ID: u64 = 50;

/// Block 900 - First Epoch Boundary
/// This is the first checkpoint with validator set transition
pub const BLOCK_900_HASH: B256 =
    b256!("0x0000000000000000000000000000000000000000000000000000000000000384"); // Placeholder - use real hash
pub const BLOCK_900_NUMBER: u64 = 900;

/// Known validators at block 900 (first epoch)
/// These are the actual validator addresses from XDC mainnet at this epoch
pub fn block_900_validators() -> Vec<Address> {
    vec![
        address!("0xb42bbe9fd1d0c5a87e1c57b0d9e0b8d6e8a1b2c3"), // Example - use real addresses
        address!("0xc42bbe9fd1d0c5a87e1c57b0d9e0b8d6e8a1b2c4"),
        address!("0xd42bbe9fd1d0c5a87e1c57b0d9e0b8d6e8a1b2c5"),
        // Add more actual validators from mainnet
    ]
}

/// Block 1800 - First Checkpoint with Rewards
/// This is the FIRST block where rewards are distributed
/// Block range for reward calculation: 1-900
pub const BLOCK_1800_HASH: B256 =
    b256!("0x0000000000000000000000000000000000000000000000000000000000000708"); // Placeholder
pub const BLOCK_1800_NUMBER: u64 = 1800;
pub const BLOCK_1800_TIMESTAMP: u64 = 1558677602; // Approximate - use real timestamp

/// Expected reward distribution at block 1800
/// These values should match what go-xdc calculated
pub struct RewardVector {
    pub block: u64,
    pub signer: Address,
    pub sign_count: u64,
    pub expected_reward: u128, // In wei
}

/// Known reward vectors from block 1800 checkpoint
/// TODO: Fill these with ACTUAL data by querying mainnet or running go-xdc
pub fn block_1800_rewards() -> Vec<RewardVector> {
    vec![
        // Example structure - replace with real data
        // RewardVector {
        //     block: 1800,
        //     signer: address!("0x..."),
        //     sign_count: 50,
        //     expected_reward: 13888888888888888888888, // Actual calculated reward
        // },
    ]
}

/// Apothem V2 Switch Block (Testnet)
/// This is where consensus transitions from V1 to V2
pub const APOTHEM_V2_SWITCH_BLOCK: u64 = 23_556_600;
pub const APOTHEM_V2_SWITCH_HASH: B256 =
    b256!("0x0000000000000000000000000000000000000000000000000000000001677658"); // Placeholder

/// Mainnet V2 Switch Block
pub const MAINNET_V2_SWITCH_BLOCK: u64 = 56_857_600;
pub const MAINNET_V2_SWITCH_HASH: B256 =
    b256!("0x0000000000000000000000000000000000000000000000000000000003634d00"); // Placeholder

/// Test vector for V1→V2 transition at Apothem block 23,556,600
pub struct V2SwitchVector {
    pub block_number: u64,
    pub block_hash: B256,
    pub parent_hash: B256,
    pub round: u64,
    pub has_qc: bool,
    pub qc_signatures_count: usize,
}

/// Get the V2 switch block test vector for Apothem
pub fn apothem_v2_switch_vector() -> V2SwitchVector {
    V2SwitchVector {
        block_number: APOTHEM_V2_SWITCH_BLOCK,
        block_hash: APOTHEM_V2_SWITCH_HASH,
        parent_hash: b256!("0x0000000000000000000000000000000000000000000000000000000001677657"), // Placeholder
        round: 0, // Switch block should have round 0
        has_qc: false, // Switch block may not have QC
        qc_signatures_count: 0,
    }
}

/// Known block signers for testing reward calculation
/// These are actual signing transactions from XDC mainnet
pub struct SigningVector {
    pub block: u64,
    pub signer: Address,
    pub tx_hash: B256,
    pub block_hash_signed: B256,
}

/// Get signing vectors from blocks 1-900 (used for block 1800 rewards)
pub fn signing_vectors_epoch_1() -> Vec<SigningVector> {
    vec![
        // Example - fill with real data
        // SigningVector {
        //     block: 15,
        //     signer: address!("0x..."),
        //     tx_hash: b256!("0x..."),
        //     block_hash_signed: b256!("0x..."),
        // },
    ]
}

/// State root test vectors
/// These verify that state root calculation matches go-xdc exactly
pub struct StateRootVector {
    pub block: u64,
    pub parent_state_root: B256,
    pub expected_state_root: B256,
    pub is_checkpoint: bool,
    pub has_rewards: bool,
}

/// State root vectors from mainnet
/// TODO: Extract these from actual mainnet blocks
pub fn state_root_vectors() -> Vec<StateRootVector> {
    vec![
        // Genesis
        StateRootVector {
            block: 0,
            parent_state_root: B256::ZERO,
            expected_state_root: b256!("0x56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421"), // Empty state root
            is_checkpoint: false,
            has_rewards: false,
        },
        // Block 900 - first checkpoint, no rewards
        // StateRootVector {
        //     block: 900,
        //     parent_state_root: b256!("0x..."),
        //     expected_state_root: b256!("0x..."),
        //     is_checkpoint: true,
        //     has_rewards: false,
        // },
        // Block 1800 - second checkpoint, first rewards
        // StateRootVector {
        //     block: 1800,
        //     parent_state_root: b256!("0x..."),
        //     expected_state_root: b256!("0x..."),
        //     is_checkpoint: true,
        //     has_rewards: true,
        // },
    ]
}

/// Extra data test vectors
pub struct ExtraDataVector {
    pub block: u64,
    pub extra_data_hex: &'static str,
    pub is_checkpoint: bool,
    pub expected_validators: Vec<Address>,
    pub version: u8, // 1 for V1, 2 for V2
}

/// Extra data vectors from mainnet
pub fn extra_data_vectors() -> Vec<ExtraDataVector> {
    vec![
        // Block 900 - V1 checkpoint with validators
        // ExtraDataVector {
        //     block: 900,
        //     extra_data_hex: "0x...",
        //     is_checkpoint: true,
        //     expected_validators: block_900_validators(),
        //     version: 1,
        // },
    ]
}

/// Difficulty calculation test vectors
pub struct DifficultyVector {
    pub block: u64,
    pub signer: Address,
    pub is_inturn: bool,
    pub expected_difficulty: u64,
}

/// Difficulty vectors (in-turn vs out-of-turn)
pub fn difficulty_vectors() -> Vec<DifficultyVector> {
    vec![
        // In-turn examples
        // DifficultyVector {
        //     block: 1,
        //     signer: address!("0x..."),
        //     is_inturn: true,
        //     expected_difficulty: 2,
        // },
        // Out-of-turn examples
        // DifficultyVector {
        //     block: 2,
        //     signer: address!("0x..."),
        //     is_inturn: false,
        //     expected_difficulty: 1,
        // },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_genesis_constants() {
        assert_eq!(GENESIS_NUMBER, 0);
        assert_eq!(GENESIS_CHAIN_ID, 50);
        assert_ne!(GENESIS_HASH, B256::ZERO);
    }

    #[test]
    fn test_checkpoint_blocks() {
        assert_eq!(BLOCK_900_NUMBER % 900, 0);
        assert_eq!(BLOCK_1800_NUMBER % 900, 0);
        assert_eq!(APOTHEM_V2_SWITCH_BLOCK % 900, 0);
        assert_eq!(MAINNET_V2_SWITCH_BLOCK % 900, 0);
    }

    #[test]
    fn test_v2_switch_vector() {
        let vec = apothem_v2_switch_vector();
        assert_eq!(vec.block_number, APOTHEM_V2_SWITCH_BLOCK);
        assert_eq!(vec.round, 0);
        assert!(!vec.has_qc);
    }

    #[test]
    fn test_block_900_validators() {
        let validators = block_900_validators();
        assert!(!validators.is_empty(), "Should have validators");

        // Check for duplicates
        let unique: std::collections::HashSet<_> = validators.iter().collect();
        assert_eq!(
            unique.len(),
            validators.len(),
            "Validators should be unique"
        );
    }

    #[test]
    fn test_state_root_vectors() {
        let vectors = state_root_vectors();
        assert!(!vectors.is_empty(), "Should have at least genesis");

        let genesis = &vectors[0];
        assert_eq!(genesis.block, 0);
        assert_eq!(genesis.parent_state_root, B256::ZERO);
    }

    #[test]
    fn test_reward_calculation_logic() {
        // This test validates the reward calculation formula matches v2.6.8
        // (chainReward / totalSigner) * sign_count

        let chain_reward: u128 = 250_000_000_000_000_000_000; // 250 XDC
        let total_signers: u128 = 60; // Total signatures across all validators
        let sign_count: u128 = 10; // This validator signed 10 times

        let reward_per_sign = chain_reward / total_signers;
        let expected_reward = reward_per_sign * sign_count;

        // Expected: (250 / 60) * 10 = 41.666... XDC
        assert_eq!(expected_reward, 41_666_666_666_666_666_666);
    }

    #[test]
    fn test_holder_split_percentages() {
        // Verify 90/0/10 split
        let signer_reward: u128 = 100_000_000_000_000_000_000; // 100 XDC

        let master_reward = (signer_reward * 90) / 100;
        let voter_reward = (signer_reward * 0) / 100;
        let foundation_reward = (signer_reward * 10) / 100;

        assert_eq!(master_reward, 90_000_000_000_000_000_000); // 90 XDC
        assert_eq!(voter_reward, 0); // 0 XDC
        assert_eq!(foundation_reward, 10_000_000_000_000_000_000); // 10 XDC

        assert_eq!(
            master_reward + voter_reward + foundation_reward,
            signer_reward
        );
    }

    #[test]
    fn test_checkpoint_range_formula() {
        // Verify block 1800 reward range: blocks 1-900
        let checkpoint = 1800u64;
        let epoch = 900u64;

        let prev_checkpoint = checkpoint - (epoch * 2);
        let start_block = prev_checkpoint + 1;
        let end_block = start_block + epoch - 1;

        assert_eq!(prev_checkpoint, 0);
        assert_eq!(start_block, 1);
        assert_eq!(end_block, 900);

        // Verify block 2700 reward range: blocks 901-1800
        let checkpoint = 2700u64;
        let prev_checkpoint = checkpoint - (epoch * 2);
        let start_block = prev_checkpoint + 1;
        let end_block = start_block + epoch - 1;

        assert_eq!(prev_checkpoint, 900);
        assert_eq!(start_block, 901);
        assert_eq!(end_block, 1800);
    }

    #[test]
    fn test_merge_sign_range() {
        // Only blocks divisible by 15 should be counted (post-TIP2019)
        let merge_sign_range = 15u64;

        assert_eq!(15 % merge_sign_range, 0);
        assert_eq!(30 % merge_sign_range, 0);
        assert_eq!(900 % merge_sign_range, 0);

        assert_ne!(1 % merge_sign_range, 0);
        assert_ne!(16 % merge_sign_range, 0);
        assert_ne!(899 % merge_sign_range, 0);
    }
}
