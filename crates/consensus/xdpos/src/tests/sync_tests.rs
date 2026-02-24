//! Tests for XDC sync engine integration

use crate::{
    execution::{finalize_state_root, should_apply_rewards, ConsensusVersion},
    sync::{is_checkpoint_block, XdcSyncConfig, XdcSyncMode, XdcSyncStats},
    XdcStateRootCache,
};
use alloy_primitives::{B256, U256};

#[test]
fn test_checkpoint_detection() {
    // Block 0 is genesis, not a checkpoint
    assert!(!is_checkpoint_block(0, 900));

    // Regular blocks are not checkpoints
    assert!(!is_checkpoint_block(1, 900));
    assert!(!is_checkpoint_block(899, 900));
    assert!(!is_checkpoint_block(901, 900));

    // Epoch boundaries are checkpoints
    assert!(is_checkpoint_block(900, 900));
    assert!(is_checkpoint_block(1800, 900));
    assert!(is_checkpoint_block(2700, 900));
    assert!(is_checkpoint_block(56857600, 900)); // V2 switch block
}

#[test]
fn test_should_apply_rewards_logic() {
    // Rewards only at checkpoints
    assert!(!should_apply_rewards(0, 900));
    assert!(!should_apply_rewards(899, 900));
    assert!(should_apply_rewards(900, 900));
    assert!(!should_apply_rewards(901, 900));
    assert!(should_apply_rewards(1800, 900));
}

#[test]
fn test_xdc_sync_config_mainnet() {
    let config = XdcSyncConfig::mainnet(None);

    assert_eq!(config.chain_id, 50);
    assert!(config.is_xdc_chain());
    assert_eq!(config.xdpos_config.epoch, 900);
    assert_eq!(config.xdpos_config.period, 2);
    assert_eq!(config.xdpos_config.gap, 450);
    assert_eq!(config.xdpos_config.reward, 250_000_000_000_000_000_000);

    // Check V2 switch block
    assert_eq!(config.xdpos_config.v2_switch_block(), Some(56_857_600));
}

#[test]
fn test_xdc_sync_config_apothem() {
    let config = XdcSyncConfig::apothem(None);

    assert_eq!(config.chain_id, 51);
    assert!(config.is_xdc_chain());
    assert_eq!(config.xdpos_config.epoch, 900);
}

#[test]
fn test_consensus_version_detection() {
    let config = XdcSyncConfig::mainnet(None);
    let switch_block = 56_857_600;

    // Before V2 switch
    assert_eq!(
        config.consensus_version(switch_block - 1),
        ConsensusVersion::V1
    );

    // At V2 switch
    assert_eq!(
        config.consensus_version(switch_block),
        ConsensusVersion::V2
    );

    // After V2 switch
    assert_eq!(
        config.consensus_version(switch_block + 1),
        ConsensusVersion::V2
    );

    // Way before V2
    assert_eq!(config.consensus_version(1000), ConsensusVersion::V1);

    // Way after V2
    assert_eq!(config.consensus_version(60_000_000), ConsensusVersion::V2);
}

#[test]
fn test_should_use_cache_logic() {
    let config = XdcSyncConfig::mainnet(None);

    // Cache only used at checkpoints
    assert!(!config.should_use_cache(0));
    assert!(!config.should_use_cache(899));
    assert!(config.should_use_cache(900));
    assert!(!config.should_use_cache(901));
    assert!(config.should_use_cache(1800));

    // Non-XDC chains shouldn't use cache
    let mut non_xdc_config = config.clone();
    non_xdc_config.chain_id = 1; // Ethereum mainnet
    assert!(!non_xdc_config.should_use_cache(900));
}

#[test]
fn test_xdc_sync_mode() {
    let mode = XdcSyncMode::default();
    assert_eq!(mode, XdcSyncMode::Full);
    assert!(mode.is_full());
    assert_eq!(mode.as_str(), "full");

    // XDC only supports full sync
    let mode = XdcSyncMode::Full;
    assert!(mode.is_full());
}

#[test]
fn test_sync_stats_tracking() {
    let mut stats = XdcSyncStats::new();

    // Initial state
    assert_eq!(stats.blocks_synced, 0);
    assert_eq!(stats.checkpoints_processed, 0);
    assert_eq!(stats.cache_hits, 0);
    assert_eq!(stats.cache_misses, 0);
    assert_eq!(stats.v1_blocks, 0);
    assert_eq!(stats.v2_blocks, 0);

    // Record V1 blocks
    for i in 1..=900 {
        stats.record_block(i, ConsensusVersion::V1, 900);
    }

    assert_eq!(stats.blocks_synced, 900);
    assert_eq!(stats.checkpoints_processed, 1); // Block 900
    assert_eq!(stats.v1_blocks, 900);
    assert_eq!(stats.v2_blocks, 0);

    // Record V2 blocks
    for i in 56_857_600..=56_858_500 {
        stats.record_block(i, ConsensusVersion::V2, 900);
    }

    assert_eq!(stats.blocks_synced, 1801);
    assert_eq!(stats.checkpoints_processed, 2); // Block 900 and 56_858_400
    assert_eq!(stats.v1_blocks, 900);
    assert_eq!(stats.v2_blocks, 901);
}

#[test]
fn test_sync_stats_cache_metrics() {
    let mut stats = XdcSyncStats::new();

    // No cache activity yet
    assert_eq!(stats.cache_hit_rate(), 0.0);

    // Record some cache hits
    stats.record_cache_hit();
    stats.record_cache_hit();
    stats.record_cache_hit();

    assert_eq!(stats.cache_hits, 3);
    assert_eq!(stats.cache_hit_rate(), 1.0);

    // Record some misses
    stats.record_cache_miss();

    assert_eq!(stats.cache_misses, 1);
    assert_eq!(stats.cache_hit_rate(), 0.75);

    // More misses
    stats.record_cache_miss();
    stats.record_cache_miss();

    assert_eq!(stats.cache_misses, 3);
    assert_eq!(stats.cache_hit_rate(), 0.5);
}

#[test]
fn test_sync_stats_reward_tracking() {
    let mut stats = XdcSyncStats::new();

    // No rewards yet
    assert_eq!(stats.total_rewards_applied, U256::ZERO);
    assert_eq!(stats.avg_rewards_per_checkpoint(), U256::ZERO);

    // Record first checkpoint
    stats.record_block(900, ConsensusVersion::V1, 900);
    let epoch_reward = U256::from(250_000_000_000_000_000_000u128); // 250 XDC
    stats.record_rewards(epoch_reward);

    assert_eq!(stats.total_rewards_applied, epoch_reward);
    assert_eq!(stats.avg_rewards_per_checkpoint(), epoch_reward);

    // Record second checkpoint
    stats.record_block(1800, ConsensusVersion::V1, 900);
    stats.record_rewards(epoch_reward);

    assert_eq!(stats.total_rewards_applied, epoch_reward * U256::from(2));
    assert_eq!(stats.avg_rewards_per_checkpoint(), epoch_reward);
}

#[test]
fn test_state_root_finalization_non_checkpoint() {
    let cache = XdcStateRootCache::with_default_size(None);

    let header_root = B256::from([1u8; 32]);
    let computed_root = B256::from([2u8; 32]);

    // Non-checkpoint blocks always use computed root
    let result = finalize_state_root(899, header_root, computed_root, &cache, 900);
    assert_eq!(result, computed_root);

    let result = finalize_state_root(901, header_root, computed_root, &cache, 900);
    assert_eq!(result, computed_root);
}

#[test]
fn test_state_root_finalization_checkpoint_matching() {
    let cache = XdcStateRootCache::with_default_size(None);

    let state_root = B256::from([1u8; 32]);

    // Matching roots at checkpoint
    let result = finalize_state_root(900, state_root, state_root, &cache, 900);
    assert_eq!(result, state_root);

    // Cache should not be populated for matching roots
    assert_eq!(cache.stats().entries, 0);
}

#[test]
fn test_state_root_finalization_checkpoint_new_divergence() {
    let cache = XdcStateRootCache::with_default_size(None);

    let header_root = B256::from([1u8; 32]);
    let computed_root = B256::from([2u8; 32]);

    // First time seeing this divergence
    let result = finalize_state_root(900, header_root, computed_root, &cache, 900);
    assert_eq!(result, computed_root);

    // Mapping should be stored
    assert_eq!(cache.get_local_root(&header_root), Some(computed_root));
    assert_eq!(cache.stats().entries, 1);
}

#[test]
fn test_state_root_finalization_checkpoint_cached_divergence() {
    let cache = XdcStateRootCache::with_default_size(None);

    let header_root = B256::from([1u8; 32]);
    let local_root = B256::from([2u8; 32]);

    // Pre-populate cache
    cache.insert(900, header_root, local_root);

    // Should use cached local root
    let result = finalize_state_root(900, header_root, local_root, &cache, 900);
    assert_eq!(result, local_root);

    // Cache stats should show hit
    assert_eq!(cache.stats().entries, 1);
}

#[test]
fn test_state_root_cache_across_multiple_checkpoints() {
    let cache = XdcStateRootCache::with_default_size(None);

    // Simulate syncing multiple checkpoints with divergences
    for i in 1..=10 {
        let block_num = i * 900;
        let header_root = B256::from([i as u8; 32]);
        let computed_root = B256::from([(i + 100) as u8; 32]);

        let result = finalize_state_root(block_num, header_root, computed_root, &cache, 900);
        assert_eq!(result, computed_root);

        // Verify mapping stored
        assert_eq!(cache.get_local_root(&header_root), Some(computed_root));
    }

    // Should have 10 entries
    assert_eq!(cache.stats().entries, 10);

    // Verify all mappings are still accessible
    for i in 1..=10 {
        let header_root = B256::from([i as u8; 32]);
        let expected_local = B256::from([(i + 100) as u8; 32]);
        assert_eq!(cache.get_local_root(&header_root), Some(expected_local));
    }
}

#[test]
fn test_consensus_version_boundary_behavior() {
    let config = XdcSyncConfig::mainnet(None);
    let switch_block = 56_857_600;

    // Test a range around the switch block
    for offset in -10..=10 {
        let block = (switch_block as i64 + offset) as u64;
        let version = config.consensus_version(block);

        if block < switch_block {
            assert_eq!(
                version,
                ConsensusVersion::V1,
                "Block {} should be V1",
                block
            );
        } else {
            assert_eq!(
                version,
                ConsensusVersion::V2,
                "Block {} should be V2",
                block
            );
        }
    }
}

#[test]
fn test_checkpoint_at_v2_switch() {
    let config = XdcSyncConfig::mainnet(None);
    let switch_block = 56_857_600;

    // V2 switch block is also a checkpoint (divisible by 900)
    assert!(is_checkpoint_block(switch_block, 900));
    assert_eq!(config.consensus_version(switch_block), ConsensusVersion::V2);
    assert!(config.should_use_cache(switch_block));
}

#[test]
fn test_full_sync_simulation_first_epoch() {
    let config = XdcSyncConfig::mainnet(None);
    let cache = &config.state_root_cache;
    let mut stats = XdcSyncStats::new();

    // Simulate syncing first epoch (blocks 1-900)
    for block_num in 1..=900 {
        let version = config.consensus_version(block_num);
        stats.record_block(block_num, version, config.xdpos_config.epoch);

        // Check if rewards should be applied
        if should_apply_rewards(block_num, config.xdpos_config.epoch) {
            // Block 900 is the first checkpoint
            assert_eq!(block_num, 900);

            // Simulate reward application
            let epoch_reward = U256::from(250_000_000_000_000_000_000u128);
            stats.record_rewards(epoch_reward);

            // Simulate state root check
            if config.should_use_cache(block_num) {
                stats.record_cache_miss(); // First time, not in cache
            }
        }
    }

    // Verify stats
    assert_eq!(stats.blocks_synced, 900);
    assert_eq!(stats.checkpoints_processed, 1);
    assert_eq!(stats.v1_blocks, 900);
    assert_eq!(stats.v2_blocks, 0);
    assert_eq!(
        stats.total_rewards_applied,
        U256::from(250_000_000_000_000_000_000u128)
    );
}

#[test]
fn test_sync_config_clone() {
    let config1 = XdcSyncConfig::mainnet(None);
    let config2 = config1.clone();

    assert_eq!(config1.chain_id, config2.chain_id);
    assert_eq!(config1.xdpos_config.epoch, config2.xdpos_config.epoch);
}
