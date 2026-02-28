//! XDPoS Configuration Types

use alloy_primitives::{Address, address};
use serde::{Deserialize, Serialize};

/// XDPoS consensus configuration
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct XDPoSConfig {
    /// Epoch length in blocks (default: 900)
    pub epoch: u64,

    /// Block period in seconds (default: 2)
    pub period: u64,

    /// Gap before epoch switch in blocks (default: 450)
    pub gap: u64,

    /// Block reward in wei (default: 250 XDC)
    pub reward: u128,

    /// Reward checkpoint frequency (default: 900)
    pub reward_checkpoint: u64,

    /// Foundation wallet address for reward distribution
    pub foundation_wallet: Address,

    /// V2 consensus configuration
    pub v2: Option<V2Config>,
}

impl Default for XDPoSConfig {
    fn default() -> Self {
        Self {
            epoch: 900,
            period: 2,
            gap: 450,
            reward: 250_000_000_000_000_000_000, // 250 XDC
            reward_checkpoint: 900,
            foundation_wallet: Address::ZERO,
            v2: None,
        }
    }
}

impl XDPoSConfig {
    /// Create a new XDPoS config with defaults
    pub fn new() -> Self {
        Self::default()
    }

    /// Set epoch length
    pub fn with_epoch(mut self, epoch: u64) -> Self {
        self.epoch = epoch;
        self
    }

    /// Set block period
    pub fn with_period(mut self, period: u64) -> Self {
        self.period = period;
        self
    }

    /// Set gap
    pub fn with_gap(mut self, gap: u64) -> Self {
        self.gap = gap;
        self
    }

    /// Set reward
    pub fn with_reward(mut self, reward: u128) -> Self {
        self.reward = reward;
        self
    }

    /// Set foundation wallet
    pub fn with_foundation_wallet(mut self, wallet: Address) -> Self {
        self.foundation_wallet = wallet;
        self
    }

    /// Set V2 config
    pub fn with_v2(mut self, v2: V2Config) -> Self {
        self.v2 = Some(v2);
        self
    }

    /// Check if V2 is enabled for a given block number
    pub fn is_v2(&self, block_number: u64) -> bool {
        match &self.v2 {
            Some(v2) => block_number >= v2.switch_block,
            None => false,
        }
    }

    /// Get the V2 switch block if configured
    pub fn v2_switch_block(&self) -> Option<u64> {
        self.v2.as_ref().map(|v2| v2.switch_block)
    }
}

/// XDPoS V2 consensus configuration
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct V2Config {
    /// Block number to switch to V2 consensus
    pub switch_block: u64,

    /// Mining period in seconds (default: 2)
    pub mine_period: u64,

    /// Timeout period in seconds (default: 10)
    pub timeout_period: u64,

    /// Certificate threshold percentage (default: 67)
    pub cert_threshold: u64,
}

impl Default for V2Config {
    fn default() -> Self {
        Self {
            switch_block: 0,
            mine_period: 2,
            timeout_period: 10,
            cert_threshold: 67,
        }
    }
}

impl V2Config {
    /// Create new V2 config
    pub fn new(switch_block: u64) -> Self {
        Self {
            switch_block,
            ..Default::default()
        }
    }

    /// Set mine period
    pub fn with_mine_period(mut self, period: u64) -> Self {
        self.mine_period = period;
        self
    }

    /// Set timeout period
    pub fn with_timeout_period(mut self, period: u64) -> Self {
        self.timeout_period = period;
        self
    }

    /// Set certificate threshold
    pub fn with_cert_threshold(mut self, threshold: u64) -> Self {
        self.cert_threshold = threshold;
        self
    }

    /// Get the certificate threshold as a float (e.g., 0.67)
    pub fn cert_threshold_float(&self) -> f64 {
        self.cert_threshold as f64 / 100.0
    }
}

/// XDC Mainnet configuration
pub fn xdc_mainnet_config() -> XDPoSConfig {
    XDPoSConfig {
        epoch: 900,
        period: 2,
        gap: 450,
        reward: 250_000_000_000_000_000_000,
        reward_checkpoint: 900,
        foundation_wallet: address!("0x746249c61f5832c5eed53172776b460491bdcd5c"), // XDC mainnet foundation wallet
        v2: Some(V2Config {
            switch_block: 80_370_000, // TIPV2SwitchBlock — constants.mainnet.go
            mine_period: 2,
            timeout_period: 10,
            cert_threshold: 67,
        }),
    }
}

/// XDC Apothem testnet configuration
pub fn xdc_apothem_config() -> XDPoSConfig {
    XDPoSConfig {
        epoch: 900,
        period: 2,
        gap: 450,
        reward: 250_000_000_000_000_000_000,
        reward_checkpoint: 900,
        foundation_wallet: address!("0x746249c61f5832c5eed53172776b460491bdcd5c"), // Apothem foundation wallet
        v2: Some(V2Config {
            switch_block: 56_828_700, // TIPV2SwitchBlock — constants.testnet.go
            mine_period: 2,
            timeout_period: 10,
            cert_threshold: 67,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = XDPoSConfig::default();
        assert_eq!(config.epoch, 900);
        assert_eq!(config.period, 2);
        assert_eq!(config.gap, 450);
        assert!(!config.is_v2(0));
    }

    #[test]
    fn test_v2_config() {
        let config = XDPoSConfig::new()
            .with_v2(V2Config::new(1000));

        assert!(!config.is_v2(999));
        assert!(config.is_v2(1000));
        assert!(config.is_v2(1001));
    }

    #[test]
    fn test_cert_threshold() {
        let v2 = V2Config::new(1000).with_cert_threshold(67);
        assert_eq!(v2.cert_threshold_float(), 0.67);
    }
}
