//! Reward calculation for XDPoS
//!
//! This module implements the XDPoS reward distribution algorithm, which:
//! 1. At checkpoint blocks (N % 900 == 0), walks backwards through the previous epoch
//! 2. Counts signing transactions per validator
//! 3. Distributes rewards proportionally based on signing participation
//! 4. Splits rewards: 90% to masternode owner, 0% to voters, 10% to foundation
//!
//! CRITICAL: This implementation must match go-ethereum/consensus/XDPoS/reward.go exactly
//! for state root compatibility.

use crate::config::XDPoSConfig;
use crate::errors::{XDPoSError, XDPoSResult};
use alloy_primitives::{address, Address, U256};
use std::collections::HashMap;

/// Reward distribution percentages (from consensus/XDPoS/constants.go)
/// NOTE: These differ from some documentation. This matches v2.6.8 exactly.
pub const REWARD_MASTER_PERCENT: u64 = 90;
pub const REWARD_VOTER_PERCENT: u64 = 0;
pub const REWARD_FOUNDATION_PERCENT: u64 = 10;

/// Block reward in wei (250 XDC)
pub const BLOCK_REWARD: u128 = 250_000_000_000_000_000_000;

/// XDC Block Signers Contract Address (0x89)
pub const BLOCK_SIGNERS_ADDRESS: Address = address!("0000000000000000000000000000000000000089");

/// Sign method signature "e341eaa4" (from HexSignMethod in constants.go)
pub const SIGN_METHOD_SIG: [u8; 4] = [0xe3, 0x41, 0xea, 0xa4];

/// MergeSignRange - only count blocks at this interval (from constants.go)
pub const MERGE_SIGN_RANGE: u64 = 15;

/// TIP2019Block - blocks below this count all signatures (from constants.go)
pub const TIP2019_BLOCK: u64 = 1;

/// Reward log for a signer (matches Go's RewardLog)
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RewardLog {
    /// Number of blocks signed
    pub sign_count: u64,
    /// Calculated reward amount
    pub reward: U256,
}

/// Reward calculator for XDPoS checkpoints
pub struct RewardCalculator {
    config: XDPoSConfig,
}

impl RewardCalculator {
    /// Create a new reward calculator
    pub fn new(config: XDPoSConfig) -> Self {
        Self { config }
    }

    /// Get the config
    pub fn config(&self) -> &XDPoSConfig {
        &self.config
    }

    /// Calculate the reward amount for each signer.
    /// Matches v2.6.8 calculation: (chainReward / totalSigner) * sign_count
    ///
    /// # Arguments
    /// * `signer_logs` - Map of signers to their sign counts
    /// * `total_signer_count` - Total number of signatures across all signers
    ///
    /// # Returns
    /// Map of signer address to their calculated reward
    pub fn calculate_rewards_per_signer(
        &self,
        signer_logs: &mut HashMap<Address, RewardLog>,
        total_signer_count: u64,
    ) -> HashMap<Address, U256> {
        let mut result = HashMap::new();

        if total_signer_count == 0 {
            return result;
        }

        // Chain reward is the base block reward
        let chain_reward = U256::from(self.config.reward);

        for (signer, log) in signer_logs.iter_mut() {
            // Calculate: (chainReward / totalSigner) * sign_count
            let reward_per_sign = chain_reward / U256::from(total_signer_count);
            let total_reward = reward_per_sign * U256::from(log.sign_count);

            log.reward = total_reward;
            result.insert(*signer, total_reward);
        }

        tracing::debug!(
            total_signers = signer_logs.len(),
            total_signer_count,
            chain_reward = %chain_reward,
            "Calculated rewards per signer"
        );

        result
    }

    /// Calculate reward distribution for holders (owner, voters, foundation).
    /// Matches v2.6.8 behavior:
    /// - 90% to masternode owner
    /// - 0% to voters (infrastructure exists but percentage is 0)
    /// - 10% to foundation
    ///
    /// # Arguments
    /// * `owner` - The masternode owner address
    /// * `signer_reward` - Total reward for this signer
    ///
    /// # Returns
    /// Map of recipient addresses to their reward amounts
    pub fn calculate_holder_rewards(
        &self,
        owner: Address,
        signer_reward: U256,
    ) -> HashMap<Address, U256> {
        let mut balances = HashMap::new();

        if signer_reward.is_zero() {
            return balances;
        }

        // Calculate owner portion (90% of the signer's reward)
        let reward_master = (signer_reward * U256::from(REWARD_MASTER_PERCENT)) / U256::from(100);
        balances.insert(owner, reward_master);

        // Voter rewards are 0% currently (infrastructure kept for future)
        // In v2.6.8, voters are still processed but get 0% of rewards

        // Foundation reward (10%)
        let foundation_reward =
            (signer_reward * U256::from(REWARD_FOUNDATION_PERCENT)) / U256::from(100);
        if self.config.foundation_wallet != Address::ZERO {
            balances.insert(self.config.foundation_wallet, foundation_reward);
        }

        balances
    }

    /// Get the reward for a given block number (with halving applied if configured).
    /// Currently XDPoS does not implement halving, but this is here for future extension.
    pub fn get_reward_for_block(&self, _block_number: u64) -> U256 {
        // No halving in current XDPoS implementation
        U256::from(self.config.reward)
    }

    /// Calculate checkpoint block range for reward calculation.
    /// Returns (prev_checkpoint, start_block, end_block)
    ///
    /// # Algorithm (v2.6.8)
    /// At checkpoint block N (where N % 900 == 0):
    /// 1. prevCheckpoint = N - (900 * 2) = N - 1800
    /// 2. startBlock = prevCheckpoint + 1
    /// 3. endBlock = startBlock + 900 - 1
    pub fn calculate_checkpoint_range(
        &self,
        checkpoint_number: u64,
    ) -> XDPoSResult<(u64, u64, u64)> {
        let rcheckpoint = self.config.reward_checkpoint;

        // Checkpoint must be a multiple of reward_checkpoint
        if checkpoint_number % rcheckpoint != 0 {
            return Err(XDPoSError::Custom("Not a checkpoint block".to_string()));
        }

        // First checkpoint with rewards is block 1800 (second checkpoint)
        if checkpoint_number < rcheckpoint * 2 {
            return Err(XDPoSError::Custom(
                "Before second checkpoint".to_string(),
            ));
        }

        // v2.6.8 formula
        let prev_checkpoint = checkpoint_number - (rcheckpoint * 2);
        let start_block = prev_checkpoint + 1;
        let end_block = start_block + rcheckpoint - 1;

        Ok((prev_checkpoint, start_block, end_block))
    }

    /// Check if a block should be counted for reward calculation.
    /// v2.6.8: only count blocks at MergeSignRange intervals OR if pre-TIP2019
    pub fn should_count_block(&self, block_number: u64) -> bool {
        if block_number < TIP2019_BLOCK {
            true
        } else {
            block_number % MERGE_SIGN_RANGE == 0
        }
    }
}

/// Check if transaction data represents a signing transaction.
/// Matches v2.6.8: checks target address (0x89), method sig (e341eaa4), and data >= 4 bytes.
///
/// # Arguments
/// * `to` - The recipient address (should be 0x89)
/// * `data` - Transaction input data
///
/// # Returns
/// `true` if this is a signing transaction
pub fn is_signing_tx(to: &Address, data: &[u8]) -> bool {
    // Check if target is BlockSigners contract (0x89)
    if to != &BLOCK_SIGNERS_ADDRESS {
        return false;
    }

    // Check if data is at least 4 bytes and starts with sign method signature
    if data.len() < 4 {
        return false;
    }

    // Check method signature
    data[0..4] == SIGN_METHOD_SIG
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_signing_tx_valid() {
        let mut data = Vec::new();
        data.extend_from_slice(&SIGN_METHOD_SIG); // Method signature
        data.extend_from_slice(&[0u8; 32]); // Block hash

        assert!(is_signing_tx(&BLOCK_SIGNERS_ADDRESS, &data));
    }

    #[test]
    fn test_is_signing_tx_wrong_address() {
        let mut data = Vec::new();
        data.extend_from_slice(&SIGN_METHOD_SIG);
        data.extend_from_slice(&[0u8; 32]);

        assert!(!is_signing_tx(&Address::ZERO, &data));
    }

    #[test]
    fn test_is_signing_tx_wrong_method() {
        let mut data = Vec::new();
        data.extend_from_slice(&[0x12, 0x34, 0x56, 0x78]); // Wrong method
        data.extend_from_slice(&[0u8; 32]);

        assert!(!is_signing_tx(&BLOCK_SIGNERS_ADDRESS, &data));
    }

    #[test]
    fn test_is_signing_tx_short_data() {
        let data = vec![0xe3, 0x41]; // Too short
        assert!(!is_signing_tx(&BLOCK_SIGNERS_ADDRESS, &data));
    }

    #[test]
    fn test_proportional_distribution() {
        // Test case: 3 signers with different sign counts
        // A=10 signs, B=5 signs, C=5 signs
        // Total = 20 signs
        // A gets 50%, B gets 25%, C gets 25%

        let config = XDPoSConfig {
            reward: BLOCK_REWARD,
            reward_checkpoint: 900,
            ..Default::default()
        };

        let calculator = RewardCalculator::new(config);

        let mut signer_logs = HashMap::new();
        let addr_a = Address::random();
        let addr_b = Address::random();
        let addr_c = Address::random();

        signer_logs.insert(
            addr_a,
            RewardLog {
                sign_count: 10,
                reward: U256::ZERO,
            },
        );
        signer_logs.insert(
            addr_b,
            RewardLog {
                sign_count: 5,
                reward: U256::ZERO,
            },
        );
        signer_logs.insert(
            addr_c,
            RewardLog {
                sign_count: 5,
                reward: U256::ZERO,
            },
        );

        let total_signer_count = 20u64;
        let rewards = calculator.calculate_rewards_per_signer(&mut signer_logs, total_signer_count);

        // Base reward per sign
        let chain_reward = U256::from(BLOCK_REWARD);
        let reward_per_sign = chain_reward / U256::from(total_signer_count);

        // Check proportional distribution
        assert_eq!(rewards[&addr_a], reward_per_sign * U256::from(10)); // 50%
        assert_eq!(rewards[&addr_b], reward_per_sign * U256::from(5)); // 25%
        assert_eq!(rewards[&addr_c], reward_per_sign * U256::from(5)); // 25%

        // Verify total equals chain reward
        let total: U256 = rewards.values().copied().sum();
        assert_eq!(total, chain_reward);
    }

    #[test]
    fn test_holder_reward_split() {
        // Test the 90/0/10 split
        let config = XDPoSConfig {
            foundation_wallet: Address::random(),
            reward: BLOCK_REWARD,
            ..Default::default()
        };

        let calculator = RewardCalculator::new(config.clone());

        let owner = Address::random();
        let signer_reward = U256::from(1000u64);

        let holder_rewards = calculator.calculate_holder_rewards(owner, signer_reward);

        // Master gets 90%
        assert_eq!(holder_rewards[&owner], U256::from(900));

        // Foundation gets 10%
        assert_eq!(holder_rewards[&config.foundation_wallet], U256::from(100));

        // Total should equal signer reward
        let total: U256 = holder_rewards.values().copied().sum();
        assert_eq!(total, signer_reward);
    }

    #[test]
    fn test_reward_percentages() {
        // Verify constants match v2.6.8
        assert_eq!(REWARD_MASTER_PERCENT, 90);
        assert_eq!(REWARD_VOTER_PERCENT, 0);
        assert_eq!(REWARD_FOUNDATION_PERCENT, 10);
        assert_eq!(
            REWARD_MASTER_PERCENT + REWARD_VOTER_PERCENT + REWARD_FOUNDATION_PERCENT,
            100
        );
    }

    #[test]
    fn test_constants() {
        // Verify critical constants
        assert_eq!(BLOCK_REWARD, 250_000_000_000_000_000_000);
        assert_eq!(
            BLOCK_SIGNERS_ADDRESS,
            address!("0000000000000000000000000000000000000089")
        );
        assert_eq!(SIGN_METHOD_SIG, [0xe3, 0x41, 0xea, 0xa4]);
        assert_eq!(MERGE_SIGN_RANGE, 15);
        assert_eq!(TIP2019_BLOCK, 1);
    }

    #[test]
    fn test_checkpoint_calculation_formula() {
        let config = XDPoSConfig {
            reward_checkpoint: 900,
            ..Default::default()
        };
        let calculator = RewardCalculator::new(config);

        // At checkpoint 1800:
        // prevCheckpoint = 1800 - 1800 = 0
        // startBlock = 0 + 1 = 1
        // endBlock = 1 + 900 - 1 = 900
        let (prev, start, end) = calculator.calculate_checkpoint_range(1800).unwrap();
        assert_eq!(prev, 0);
        assert_eq!(start, 1);
        assert_eq!(end, 900);

        // At checkpoint 2700:
        // prevCheckpoint = 2700 - 1800 = 900
        // startBlock = 900 + 1 = 901
        // endBlock = 901 + 900 - 1 = 1800
        let (prev, start, end) = calculator.calculate_checkpoint_range(2700).unwrap();
        assert_eq!(prev, 900);
        assert_eq!(start, 901);
        assert_eq!(end, 1800);
    }

    #[test]
    fn test_checkpoint_range_not_checkpoint() {
        let config = XDPoSConfig {
            reward_checkpoint: 900,
            ..Default::default()
        };
        let calculator = RewardCalculator::new(config);

        // Block 1799 is not a checkpoint
        assert!(calculator.calculate_checkpoint_range(1799).is_err());
    }

    #[test]
    fn test_checkpoint_range_before_second() {
        let config = XDPoSConfig {
            reward_checkpoint: 900,
            ..Default::default()
        };
        let calculator = RewardCalculator::new(config);

        // Block 900 is the first checkpoint (no rewards)
        assert!(calculator.calculate_checkpoint_range(900).is_err());
    }

    #[test]
    fn test_should_count_block() {
        let config = XDPoSConfig::default();
        let calculator = RewardCalculator::new(config);

        // Block 0 should be counted (< TIP2019_BLOCK)
        assert!(calculator.should_count_block(0));

        // Block 15 should be counted (divisible by MERGE_SIGN_RANGE)
        assert!(calculator.should_count_block(15));

        // Block 30 should be counted
        assert!(calculator.should_count_block(30));

        // Block 16 should NOT be counted
        assert!(!calculator.should_count_block(16));

        // Block 901 should NOT be counted
        assert!(!calculator.should_count_block(901));

        // Block 900 should be counted (divisible by 15)
        assert!(calculator.should_count_block(900));
    }
}
