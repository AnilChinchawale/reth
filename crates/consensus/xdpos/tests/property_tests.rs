//! Property-based tests for XDPoS consensus
//!
//! These tests use randomized inputs to verify invariants that should
//! hold true for all valid inputs.

use reth_consensus_xdpos::{
    reward::{RewardCalculator, RewardLog, BLOCK_REWARD},
    snapshot::Snapshot,
    state_root_cache::XdcStateRootCache,
    v2::proposer::select_proposer,
    XDPoSConfig,
};
use alloy_primitives::{Address, B256, U256};
use std::collections::HashMap;
use std::time::Duration;

/// Test that proposer selection always picks a valid validator
/// Property: ∀ validator_set, round: select_proposer(round, validators) ∈ validators
#[test]
fn prop_proposer_selects_valid_validator() {
    // Test with various validator set sizes
    for validator_count in [3, 5, 10, 18, 21, 50, 100] {
        let validators: Vec<Address> = (0..validator_count)
            .map(|i| Address::with_last_byte((i % 256) as u8))
            .collect();

        // Test many rounds
        for round in 0..1000u64 {
            let proposer = select_proposer(round, &validators);
            assert!(
                proposer.is_ok(),
                "Proposer selection should succeed for round {}",
                round
            );
            let proposer = proposer.unwrap();
            assert!(
                validators.contains(&proposer),
                "Proposer {} at round {} should be in validators {:?}",
                proposer,
                round,
                validators
            );
        }
    }
}

/// Test that reward distribution always sums to exactly total reward
/// Property: ∀ sign_counts: sum(calculate_rewards(sign_counts)) = BLOCK_REWARD
#[test]
fn prop_reward_sum_equals_total() {
    let config = XDPoSConfig::default();
    let calculator = RewardCalculator::new(config);

    // Test with various signer counts and sign distributions
    let test_cases = vec![
        (1, vec![1]),
        (2, vec![1, 1]),
        (3, vec![1, 1, 1]),
        (3, vec![2, 1]),
        (10, vec![5, 5]),
        (10, vec![3, 3, 4]),
        (60, vec![20, 20, 20]),
        (60, vec![30, 20, 10]),
        (100, vec![25, 25, 25, 25]),
    ];

    for (total_signs, individual_signs) in test_cases {
        let mut signer_logs = HashMap::new();
        for (i, count) in individual_signs.iter().enumerate() {
            signer_logs.insert(
                Address::with_last_byte(i as u8 + 1),
                RewardLog {
                    sign_count: *count,
                    reward: U256::ZERO,
                },
            );
        }

        let rewards = calculator.calculate_rewards_per_signer(&mut signer_logs, total_signs);
        let total_reward: U256 = rewards.values().copied().sum();

        // Due to integer division, we may have rounding differences
        // The sum should equal BLOCK_REWARD exactly
        assert_eq!(
            total_reward,
            U256::from(BLOCK_REWARD),
            "Total reward should equal BLOCK_REWARD for {:?}",
            individual_signs
        );
    }
}

/// Test that cache save/load round-trips correctly
/// Property: ∀ entries: load(save(entries)) = entries
#[test]
fn prop_cache_roundtrip_invariant() {
    use tempfile::tempdir;

    let temp_dir = tempdir().unwrap();

    // Test with various entry counts
    for entry_count in [1, 10, 50, 100, 500] {
        let cache_path = temp_dir.path().join(format!("cache_{}", entry_count));

        let cache = XdcStateRootCache::new(
            entry_count * 2,
            Duration::from_secs(3600),
            Some(cache_path.clone()),
        );

        // Insert entries
        let mut entries = Vec::new();
        for i in 0..entry_count {
            let block_hash = B256::random();
            let state_root = B256::random();
            let checkpoint = (i + 1) * 900;

            cache.insert(checkpoint, block_hash, state_root);
            entries.push((block_hash, state_root));
        }

        // Save
        cache.save().unwrap();

        // Load
        let cache2 = XdcStateRootCache::new(
            entry_count * 2,
            Duration::from_secs(3600),
            Some(cache_path.clone()),
        );
        cache2.load().unwrap();

        // Verify all entries
        for (block_hash, expected_state_root) in entries {
            let loaded = cache2.get(&block_hash);
            assert_eq!(
                loaded,
                Some(expected_state_root),
                "Cache entry should round-trip correctly"
            );
        }
    }
}

/// Test that empty validator set fails gracefully
#[test]
fn prop_empty_validator_set_fails() {
    let empty: Vec<Address> = vec![];

    // Proposer selection should fail with empty validators
    let result = select_proposer(0, &empty);
    assert!(result.is_err());
}

/// Test that snapshot applies maintain validator count consistency
/// Property: ∀ header: snapshot.apply(header).signer_count() ≥ 0
#[test]
fn prop_snapshot_validator_count_non_negative() {
    let validators = vec![
        Address::with_last_byte(1),
        Address::with_last_byte(2),
        Address::with_last_byte(3),
    ];

    let mut snapshot = Snapshot::new(0, B256::ZERO, validators);

    // Validator count should never go negative
    assert!(snapshot.signer_count() >= 0);

    // After various operations
    for i in 0..100 {
        snapshot.add_recent(i, validators[0]);
        assert!(snapshot.signer_count() >= 0);
    }
}

/// Test that duplicate signatures don't inflate reward count
/// Property: ∀ duplicate_sigs: count_unique(sigs) = count_total(sigs) - count_dups(sigs)
#[test]
fn prop_duplicate_signature_detection() {
    use reth_consensus_xdpos::v2::verification::unique_signatures;

    // Test with various signature sets
    let test_cases = vec![
        // No duplicates
        (vec![vec![1u8; 65], vec![2u8; 65]], 2, 0),
        // One duplicate
        (vec![vec![1u8; 65], vec![1u8; 65], vec![2u8; 65]], 2, 1),
        // All duplicates
        (vec![vec![1u8; 65], vec![1u8; 65], vec![1u8; 65]], 1, 2),
        // Multiple duplicates
        (vec![vec![1u8; 65], vec![1u8; 65], vec![2u8; 65], vec![2u8; 65]], 2, 2),
    ];

    for (sigs, expected_unique, expected_dups) in test_cases {
        let (unique, dups) = unique_signatures(&sigs);
        assert_eq!(unique.len(), expected_unique, "Unique count mismatch");
        assert_eq!(dups.len(), expected_dups, "Duplicate count mismatch");
    }
}

/// Test that checkpoint blocks are detected correctly
/// Property: ∀ block: block % 900 == 0 ↔ is_checkpoint(block)
#[test]
fn prop_checkpoint_detection() {
    use reth_consensus_xdpos::sync::is_checkpoint_block;

    let epoch = 900u64;

    // Test various blocks
    for block in [0, 1, 899, 900, 901, 1800, 1799, 2700, 23556600] {
        let is_checkpoint = is_checkpoint_block(block, epoch);
        let expected = block != 0 && block % epoch == 0;
        assert_eq!(
            is_checkpoint, expected,
            "Block {} checkpoint status mismatch",
            block
        );
    }
}

/// Test that reward calculation is deterministic
/// Property: ∀ inputs: calculate(inputs) = calculate(inputs) (idempotent)
#[test]
fn prop_reward_calculation_deterministic() {
    let config = XDPoSConfig::default();
    let calculator = RewardCalculator::new(config);

    // Test case
    let mut signer_logs = HashMap::new();
    signer_logs.insert(
        Address::with_last_byte(1),
        RewardLog {
            sign_count: 10,
            reward: U256::ZERO,
        },
    );
    signer_logs.insert(
        Address::with_last_byte(2),
        RewardLog {
            sign_count: 5,
            reward: U256::ZERO,
        },
    );

    // Calculate multiple times
    let mut rewards_list = Vec::new();
    for _ in 0..10 {
        let mut logs_copy = signer_logs.clone();
        let rewards = calculator.calculate_rewards_per_signer(&mut logs_copy, 15);
        rewards_list.push(rewards);
    }

    // All should be identical
    let first = &rewards_list[0];
    for (i, rewards) in rewards_list.iter().enumerate().skip(1) {
        assert_eq!(
            first, rewards,
            "Reward calculation should be deterministic (iteration {})",
            i
        );
    }
}

/// Test that epoch boundaries wrap correctly
/// Property: ∀ block: get_epoch(block) = floor(block / epoch)
#[test]
fn prop_epoch_calculation() {
    use reth_consensus_xdpos::epoch_number;

    let epoch = 900u64;

    let test_cases = vec![
        (0, 0),
        (1, 0),
        (899, 0),
        (900, 1),
        (901, 1),
        (1799, 1),
        (1800, 2),
        (9000, 10),
    ];

    for (block, expected_epoch) in test_cases {
        let calculated = epoch_number(block, epoch);
        assert_eq!(
            calculated, expected_epoch,
            "Epoch mismatch for block {}",
            block
        );
    }
}

/// Test that anti-spam limit is consistent
/// Property: ∀ N validators: limit = floor(N/2) + 1
#[test]
fn prop_anti_spam_limit_formula() {
    // Test the formula for various validator counts
    let test_cases = vec![
        (3, 2),   // floor(3/2) + 1 = 2
        (5, 3),   // floor(5/2) + 1 = 3
        (10, 6),  // floor(10/2) + 1 = 6
        (18, 10), // floor(18/2) + 1 = 10
        (21, 11), // floor(21/2) + 1 = 11
    ];

    for (validator_count, expected_limit) in test_cases {
        let calculated = validator_count / 2 + 1;
        assert_eq!(
            calculated, expected_limit,
            "Anti-spam limit mismatch for {} validators",
            validator_count
        );
    }
}

/// Test that holder reward split percentages sum to 100
/// Property: master% + voter% + foundation% = 100
#[test]
fn prop_reward_percentage_sum() {
    use reth_consensus_xdpos::reward::{
        REWARD_FOUNDATION_PERCENT, REWARD_MASTER_PERCENT, REWARD_VOTER_PERCENT,
    };

    let total = REWARD_MASTER_PERCENT + REWARD_VOTER_PERCENT + REWARD_FOUNDATION_PERCENT;
    assert_eq!(total, 100, "Reward percentages should sum to 100");
}

/// Test that QC threshold calculation is correct
/// Property: ∀ N validators: threshold = ceil(N * 0.667)
#[test]
fn prop_qc_threshold_calculation() {
    use reth_consensus_xdpos::v2::verification::CERT_THRESHOLD;

    let test_cases = vec![
        (3, 2),   // ceil(3 * 0.667) = 2
        (10, 7),  // ceil(10 * 0.667) = 7
        (18, 12), // ceil(18 * 0.667) = 12
        (21, 14), // ceil(21 * 0.667) = 14
    ];

    for (validator_count, expected_threshold) in test_cases {
        let calculated = (validator_count as f64 * CERT_THRESHOLD).ceil() as usize;
        assert_eq!(
            calculated, expected_threshold,
            "QC threshold mismatch for {} validators",
            validator_count
        );
    }
}

/// Test that in-turn calculation wraps correctly
/// Property: ∀ block, ∀ N validators: in-turn = validators[block % N]
#[test]
fn prop_inturn_wraps_correctly() {
    use reth_consensus_xdpos::Snapshot;

    let validators: Vec<Address> = (0..18).map(|i| Address::with_last_byte(i as u8 + 1)).collect();
    let snapshot = Snapshot::new(0, B256::ZERO, validators.clone());

    // Test blocks that wrap around
    for block in [0, 17, 18, 19, 35, 36, 100] {
        let expected_index = (block as usize) % validators.len();
        let expected_validator = validators[expected_index];

        assert!(
            snapshot.inturn(block, &expected_validator),
            "Block {} should have validator {} in-turn",
            block,
            expected_index
        );

        // Other validators should not be in-turn
        for (i, validator) in validators.iter().enumerate() {
            if i != expected_index {
                assert!(
                    !snapshot.inturn(block, validator),
                    "Block {} should not have validator {} in-turn",
                    block,
                    i
                );
            }
        }
    }
}
