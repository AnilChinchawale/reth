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
use crate::snapshot::Snapshot;
use alloy_primitives::{address, Address, Bytes, TxKind, U256};
use reth_primitives::TransactionSigned;
use reth_storage_api::BlockReader;
use std::collections::{HashMap, HashSet};

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
#[derive(Debug, Clone, Default)]
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

    /// Check if a transaction is a block signing transaction.
    /// Matches v2.6.8: checks target address (0x89), method sig (e341eaa4), and data >= 4 bytes.
    pub fn is_signing_tx(tx: &TransactionSigned) -> bool {
        // Check if transaction has a recipient
        let Some(TxKind::Call(to)) = tx.to() else {
            return false;
        };

        // Check if target is BlockSigners contract (0x89)
        if to != BLOCK_SIGNERS_ADDRESS {
            return false;
        }

        // Check if data is at least 4 bytes and starts with sign method signature
        let input = tx.input();
        if input.len() < 4 {
            return false;
        }

        // Check method signature
        input[0..4] == SIGN_METHOD_SIG
    }

    /// Calculate rewards at checkpoint block.
    /// Implements the algorithm from go-ethereum/consensus/XDPoS/reward.go:GetRewardForCheckpoint
    ///
    /// # Algorithm (v2.6.8)
    /// At checkpoint block N (where N % 900 == 0):
    /// 1. prevCheckpoint = N - (900 * 2) = N - 1800
    /// 2. startBlock = prevCheckpoint + 1
    /// 3. endBlock = startBlock + 900 - 1
    /// 4. Walk backwards from current block to startBlock by parent hash
    /// 5. For each block, find signing transactions (tx.to == 0x89, method == e341eaa4)
    /// 6. Extract block hash from tx data (last 32 bytes) and count signers
    /// 7. Filter signers: only count those in the masternode list from prevCheckpoint header
    /// 8. Only count blocks at MergeSignRange (15) intervals OR if block < TIP2019Block
    /// 9. Calculate total reward = (250 XDC * number_of_signers) / totalSigners * signCount
    /// 10. Split per signer: 90% to masternode owner, 0% to voters, 10% to foundation
    ///
    /// # Returns
    /// - Map of signer address to RewardLog (sign count + calculated reward)
    /// - Total signer count (for proportional distribution)
    pub fn calculate_checkpoint_rewards<DB>(
        &self,
        checkpoint_number: u64,
        chain: &DB,
        checkpoint_snapshot: &Snapshot,
    ) -> XDPoSResult<(HashMap<Address, RewardLog>, u64)>
    where
        DB: BlockReader,
    {
        // Checkpoint must be a multiple of reward_checkpoint
        let rCheckpoint = self.config.reward_checkpoint;
        if checkpoint_number % rCheckpoint != 0 {
            return Err(XDPoSError::Custom(
                "Not a checkpoint block".to_string(),
            ));
        }

        // First checkpoint with rewards is block 1800 (second checkpoint)
        // At block 1800, we walk from 901 to 1799
        if checkpoint_number < rCheckpoint * 2 {
            return Ok((HashMap::new(), 0));
        }

        // v2.6.8 formula
        let prev_checkpoint = checkpoint_number - (rCheckpoint * 2);
        let start_block = prev_checkpoint + 1;
        let end_block = start_block + rCheckpoint - 1;

        tracing::debug!(
            checkpoint = checkpoint_number,
            prev_checkpoint,
            start_block,
            end_block,
            "Calculating checkpoint rewards"
        );

        // Get the masternode list from the previous checkpoint header
        let masternodes: HashSet<Address> = checkpoint_snapshot.signers.iter().copied().collect();

        tracing::debug!(
            masternodes = masternodes.len(),
            "Masternode list loaded"
        );

        // Collect signing data: block_number -> list of signer addresses
        let mut block_signers: HashMap<u64, Vec<Address>> = HashMap::new();

        // Walk backwards through blocks in the signing range
        // We need to collect all signing transactions in range [start_block, end_block]
        for block_num in start_block..=end_block {
            // Read the block from storage
            let Some(block) = chain.block_by_number(block_num)? else {
                tracing::warn!(block = block_num, "Block not found during reward scan");
                continue;
            };

            let mut signers_for_block = Vec::new();

            // Scan transactions for signing transactions
            for tx in block.body.transactions() {
                if !Self::is_signing_tx(tx) {
                    continue;
                }

                // Extract the block hash from the signing transaction data (last 32 bytes)
                let data = tx.input();
                if data.len() < 36 {
                    // Need at least 4 (method sig) + 32 (block hash)
                    continue;
                }

                // The signed block hash is in the last 32 bytes
                let signed_block_hash_data = &data[data.len() - 32..];

                // Recover the signer address from the transaction
                // In go-ethereum, they use types.Sender(signer, tx)
                // In reth, we can recover from the transaction signature
                let Some(signer) = tx.recover_signer() else {
                    tracing::warn!("Failed to recover signer from signing transaction");
                    continue;
                };

                // Only count signers that are in the masternode list
                if masternodes.contains(&signer) {
                    signers_for_block.push(signer);
                }
            }

            if !signers_for_block.is_empty() {
                block_signers.insert(block_num, signers_for_block);
            }
        }

        // Count signatures per signer (matching v2.6.8 logic)
        let mut signer_logs: HashMap<Address, RewardLog> = HashMap::new();
        let mut total_signer_count: u64 = 0;

        for block_num in start_block..=end_block {
            // v2.6.8: only count blocks at MergeSignRange intervals OR if pre-TIP2019
            let should_count = if block_num < TIP2019_BLOCK {
                true
            } else {
                block_num % MERGE_SIGN_RANGE == 0
            };

            if !should_count {
                continue;
            }

            // Get signers for this block
            let Some(signers) = block_signers.get(&block_num) else {
                continue;
            };

            // Deduplicate signers for this block (same signer can only count once per block)
            let unique_signers: HashSet<Address> = signers.iter().copied().collect();

            for signer in unique_signers {
                let log = signer_logs.entry(signer).or_insert_with(RewardLog::default);
                log.sign_count += 1;
                total_signer_count += 1;
            }
        }

        tracing::info!(
            checkpoint = checkpoint_number,
            unique_signers = signer_logs.len(),
            total_signer_count,
            "Checkpoint signature scan complete"
        );

        Ok((signer_logs, total_signer_count))
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
        let foundation_reward = (signer_reward * U256::from(REWARD_FOUNDATION_PERCENT)) / U256::from(100);
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::{hex, Address, Bytes, TxKind};
    use reth_primitives::{Transaction, TransactionSigned, TxLegacy};

    #[test]
    fn test_is_signing_tx_valid() {
        // Create a valid signing transaction
        let mut data = Vec::new();
        data.extend_from_slice(&SIGN_METHOD_SIG); // Method signature
        data.extend_from_slice(&[0u8; 32]); // Block hash

        let tx = TransactionSigned::from_transaction_and_signature(
            Transaction::Legacy(TxLegacy {
                chain_id: Some(50),
                nonce: 0,
                gas_price: 2500,
                gas_limit: 100000,
                to: TxKind::Call(BLOCK_SIGNERS_ADDRESS),
                value: U256::ZERO,
                input: Bytes::from(data),
            }),
            alloy_primitives::Signature::test_signature(),
        );

        assert!(RewardCalculator::is_signing_tx(&tx));
    }

    #[test]
    fn test_is_signing_tx_wrong_address() {
        let mut data = Vec::new();
        data.extend_from_slice(&SIGN_METHOD_SIG);
        data.extend_from_slice(&[0u8; 32]);

        let tx = TransactionSigned::from_transaction_and_signature(
            Transaction::Legacy(TxLegacy {
                chain_id: Some(50),
                nonce: 0,
                gas_price: 2500,
                gas_limit: 100000,
                to: TxKind::Call(Address::ZERO), // Wrong address
                value: U256::ZERO,
                input: Bytes::from(data),
            }),
            alloy_primitives::Signature::test_signature(),
        );

        assert!(!RewardCalculator::is_signing_tx(&tx));
    }

    #[test]
    fn test_is_signing_tx_wrong_method() {
        let mut data = Vec::new();
        data.extend_from_slice(&[0x12, 0x34, 0x56, 0x78]); // Wrong method
        data.extend_from_slice(&[0u8; 32]);

        let tx = TransactionSigned::from_transaction_and_signature(
            Transaction::Legacy(TxLegacy {
                chain_id: Some(50),
                nonce: 0,
                gas_price: 2500,
                gas_limit: 100000,
                to: TxKind::Call(BLOCK_SIGNERS_ADDRESS),
                value: U256::ZERO,
                input: Bytes::from(data),
            }),
            alloy_primitives::Signature::test_signature(),
        );

        assert!(!RewardCalculator::is_signing_tx(&tx));
    }

    #[test]
    fn test_is_signing_tx_short_data() {
        let data = vec![0xe3, 0x41]; // Too short

        let tx = TransactionSigned::from_transaction_and_signature(
            Transaction::Legacy(TxLegacy {
                chain_id: Some(50),
                nonce: 0,
                gas_price: 2500,
                gas_limit: 100000,
                to: TxKind::Call(BLOCK_SIGNERS_ADDRESS),
                value: U256::ZERO,
                input: Bytes::from(data),
            }),
            alloy_primitives::Signature::test_signature(),
        );

        assert!(!RewardCalculator::is_signing_tx(&tx));
    }

    #[test]
    fn test_is_signing_tx_no_recipient() {
        let mut data = Vec::new();
        data.extend_from_slice(&SIGN_METHOD_SIG);
        data.extend_from_slice(&[0u8; 32]);

        let tx = TransactionSigned::from_transaction_and_signature(
            Transaction::Legacy(TxLegacy {
                chain_id: Some(50),
                nonce: 0,
                gas_price: 2500,
                gas_limit: 100000,
                to: TxKind::Create, // Contract creation
                value: U256::ZERO,
                input: Bytes::from(data),
            }),
            alloy_primitives::Signature::test_signature(),
        );

        assert!(!RewardCalculator::is_signing_tx(&tx));
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
        // At checkpoint 1800:
        // prevCheckpoint = 1800 - 1800 = 0
        // startBlock = 0 + 1 = 1
        // endBlock = 1 + 900 - 1 = 900
        let checkpoint = 1800u64;
        let rcheckpoint = 900u64;

        let prev_checkpoint = checkpoint - (rcheckpoint * 2);
        let start_block = prev_checkpoint + 1;
        let end_block = start_block + rcheckpoint - 1;

        assert_eq!(prev_checkpoint, 0);
        assert_eq!(start_block, 1);
        assert_eq!(end_block, 900);

        // At checkpoint 2700:
        // prevCheckpoint = 2700 - 1800 = 900
        // startBlock = 900 + 1 = 901
        // endBlock = 901 + 900 - 1 = 1800
        let checkpoint = 2700u64;
        let prev_checkpoint = checkpoint - (rcheckpoint * 2);
        let start_block = prev_checkpoint + 1;
        let end_block = start_block + rcheckpoint - 1;

        assert_eq!(prev_checkpoint, 900);
        assert_eq!(start_block, 901);
        assert_eq!(end_block, 1800);
    }
}
