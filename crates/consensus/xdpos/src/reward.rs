//! Reward calculation for XDPoS

use crate::config::XDPoSConfig;
use alloy_primitives::{Address, U256};

/// Reward distribution percentages
pub const REWARD_MASTER_PERCENT: u64 = 90;
pub const REWARD_FOUNDATION_PERCENT: u64 = 10;

/// Calculate block reward
pub fn calculate_reward(
    _block_number: u64,
    config: &XDPoSConfig,
) -> U256 {
    U256::from(config.reward)
}

/// Calculate rewards for checkpoint distribution
pub fn calculate_checkpoint_rewards(
    _checkpoint_number: u64,
    _signers: Vec<Address>,
    config: &XDPoSConfig,
) -> Vec<(Address, U256)> {
    // TODO: Implement full reward distribution logic
    // 1. Calculate total reward
    // 2. Distribute 90% to masternodes
    // 3. Distribute 10% to foundation

    let total_reward = U256::from(config.reward);
    let foundation_reward = total_reward * U256::from(REWARD_FOUNDATION_PERCENT) / U256::from(100);
    let _master_reward = total_reward * U256::from(REWARD_MASTER_PERCENT) / U256::from(100);

    vec![
        (config.foundation_wallet, foundation_reward),
    ]
}
