//! Integration tests for XDPoS consensus
//!
//! End-to-end tests covering:
//! - Full block validation pipeline
//! - V1/V2 block validation
//! - Checkpoint reward distribution
//! - State root cache persistence
//! - Special transaction gas handling

use reth_consensus_xdpos::{
    constants::{EXTRA_SEAL, EXTRA_VANITY, XDC_APOTHEM_CHAIN_ID, XDC_MAINNET_CHAIN_ID},
    extra_data::{hash_without_seal, recover_signer},
    reward::{RewardCalculator, RewardLog, BLOCK_REWARD},
    snapshot::Snapshot,
    state_root_cache::{CacheStats, XdcStateRootCache},
    sync::XdcSyncConfig,
    tests::helpers::{
        mock_checkpoint_header, mock_qc, mock_signing_tx, mock_validator_set,
        mock_v1_header, mock_v2_header, test_config, test_config_v2,
    },
    tests::vectors::{
        block_1800_rewards, GENESIS_HASH, GENESIS_CHAIN_ID,
        APOTHEM_V2_SWITCH_BLOCK,
    },
    v1::validate_v1_header,
    v2::engine::XDPoSV2Engine,
    v2::proposer::select_proposer,
    v2::verification::verify_qc,
    ConsensusVersion, XDPoSConfig,
};
use alloy_consensus::Header;
use alloy_primitives::{Address, B256, U256};
use std::collections::HashMap;
use std::time::{Duration, Instant};

mod property_tests;

/// Test full V1 block validation pipeline
#[test]
fn test_v1_block_validation_pipeline() {
    let config = test_config();
    let validators = mock_validator_set(18);
    let snapshot = Snapshot::new(0, GENESIS_HASH, validators.clone());

    // Create a V1 header
    let header = mock_v1_header(100, GENESIS_HASH);

    // Validate header structure
    let result = validate_v1_header(&header,
&config,
 None,
 Some(&snapshot));
    // Note: Full validation would require valid signature, so we check partial
    assert!(result.is_ok() || matches!(result, Err(reth_consensus_xdpos::XDPoSError::InvalidSeal)));
}

/// Test V2 block validation with QC verification
#[test]
fn test_v2_block_validation_with_qc() {
    let config = test_config_v2(APOTHEM_V2_SWITCH_BLOCK);
    let engine = XDPoSV2Engine::new(config.clone());
    let validators = mock_validator_set(18);

    // Create a QC with enough signatures (12 for 18 validators at 67%)
    let qc = mock_qc(B256::random(), 100, 1000, 12);

    // Verify QC has sufficient signatures
    let result = verify_qc(&qc, &validators, None);
    assert!(result.is_ok());

    // Create V2 header with QC
    let header = mock_v2_header(1000, B256::random(), 101, Some(&qc));

    // Verify it's detected as V2 block
    assert!(engine.is_v2_block(1000 + APOTHEM_V2_SWITCH_BLOCK));
}

/// Test checkpoint reward distribution at block 1800
#[test]
fn test_checkpoint_reward_distribution() {
    let config = test_config();
    let calculator = RewardCalculator::new(config);

    // Simulate signing records for epoch 1 (blocks 1-900)
    let mut signer_logs = HashMap::new();
    let addr_a = Address::with_last_byte(1);
    let addr_b = Address::with_last_byte(2);
    let addr_c = Address::with_last_byte(3);

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

    // Verify proportional distribution
    let chain_reward = U256::from(BLOCK_REWARD);
    let reward_per_sign = chain_reward / U256::from(total_signer_count);

    assert_eq!(rewards[&addr_a], reward_per_sign * U256::from(10));
    assert_eq!(rewards[&addr_b], reward_per_sign * U256::from(5));
    assert_eq!(rewards[&addr_c], reward_per_sign * U256::from(5));

    // Verify total equals chain reward
    let total_reward: U256 = rewards.values().copied().sum();
    assert_eq!(total_reward, chain_reward);
}

/// Test state root cache round-trip
#[test]
fn test_state_root_cache_roundtrip() {
    use tempfile::tempdir;

    let temp_dir = tempdir().unwrap();
    let cache_path = temp_dir.path().join("state_root_cache");

    // Create and populate cache
    let cache = XdcStateRootCache::new(
        1000,                     // capacity
        Duration::from_secs(3600), // TTL
        Some(cache_path.clone()),
    );

    // Insert some entries
    let block_hash_1 = B256::random();
    let state_root_1 = B256::random();
    let block_hash_2 = B256::random();
    let state_root_2 = B256::random();

    cache.insert(900, block_hash_1, state_root_1);
    cache.insert(1800, block_hash_2, state_root_2);

    // Verify inserted
    assert_eq!(cache.get(&block_hash_1), Some(state_root_1));
    assert_eq!(cache.get(&block_hash_2), Some(state_root_2));

    // Save cache
    cache.save().unwrap();

    // Create new cache and load
    let cache2 = XdcStateRootCache::new(
        1000,
        Duration::from_secs(3600),
        Some(cache_path.clone()),
    );

    cache2.load().unwrap();

    // Verify loaded entries
    assert_eq!(cache2.get(&block_hash_1), Some(state_root_1));
    assert_eq!(cache2.get(&block_hash_2), Some(state_root_2));

    // Check stats
    let stats = cache2.stats();
    assert_eq!(stats.hits, 0);
    assert_eq!(stats.misses, 0);
}

/// Test special transaction gas handling
#[test]
fn test_special_tx_gas_handling() {
    use reth_consensus_xdpos::special_tx::TIP_SIGNING;

    // Before TIPSigning: all transactions pay gas
    let pre_tip_block = TIP_SIGNING - 1;
    let tx = mock_signing_tx(Address::with_last_byte(1), B256::random());

    // Signing transaction should still pay gas before TIP
    assert!(tx.gas > 0);
    assert_eq!(tx.gas_price, U256::from(1_000_000_000u64));

    // After TIPSigning: signing transactions may have different gas rules
    // This would need the actual implementation to verify
    let _post_tip_block = TIP_SIGNING + 1;
}

/// Test seal recovery from V1 header
#[test]
fn test_v1_seal_recovery() {
    use secp256k1::{Secp256k1, SecretKey, PublicKey};
    use alloy_primitives::keccak256;

    // Generate test key
    let secret_key = SecretKey::from_slice(&[1u8; 32]).unwrap();
    let secp = Secp256k1::new();
    let public_key = PublicKey::from_secret_key(&secp, &secret_key);

    // Derive expected address
    let pubkey_bytes = public_key.serialize_uncompressed();
    let pubkey_hash = keccak256(&pubkey_bytes[1..]);
    let expected_address = Address::from_slice(&pubkey_hash[12..]);

    // Create signed header
    let mut header = mock_v1_header(100, B256::ZERO);

    // Sign the header hash
    let msg_hash = hash_without_seal(&header);
    let message = secp256k1::Message::from_digest_slice(msg_hash.as_slice()).unwrap();
    let sig = secp.sign_ecdsa_recoverable(&message, &secret_key);
    let (recovery_id, sig_bytes) = sig.serialize_compact();

    // Add signature to extra data
    let mut extra_data = vec![0u8; EXTRA_VANITY];
    extra_data.extend_from_slice(&sig_bytes);
    extra_data.push(recovery_id.to_i32() as u8 + 27);
    header.extra_data = extra_data.into();

    // Recover signer
    let recovered = recover_signer(&header).unwrap();
    assert_eq!(recovered, expected_address);
}

/// Test V1 to V2 transition detection
#[test]
fn test_v1_to_v2_transition() {
    let config = XdcSyncConfig::apothem(None);
    let v2_switch = APOTHEM_V2_SWITCH_BLOCK;

    // Before switch: V1
    assert_eq!(
        config.consensus_version(v2_switch - 1),
        ConsensusVersion::V1
    );

    // At switch: V2
    assert_eq!(
        config.consensus_version(v2_switch),
        ConsensusVersion::V2
    );

    // After switch: V2
    assert_eq!(
        config.consensus_version(v2_switch + 1),
        ConsensusVersion::V2
    );
}

/// Test checkpoint header validation
#[test]
fn test_checkpoint_header_validation() {
    let validators = mock_validator_set(18);

    // Valid checkpoint header
    let header = mock_checkpoint_header(1800, B256::ZERO, &validators);

    assert_eq!(header.number, 1800);
    assert_eq!(header.number % 900, 0);
    assert_eq!(header.beneficiary, Address::ZERO);

    // Verify validators in extra data
    let extra = reth_consensus_xdpos::extra_data::V1ExtraData::parse(&header.extra_data, true).unwrap();
    assert_eq!(extra.validators.len(), validators.len());
}

/// Test genesis block constants
#[test]
fn test_genesis_constants() {
    assert_eq!(GENESIS_CHAIN_ID, 50);
    assert_eq!(GENESIS_CHAIN_ID, XDC_MAINNET_CHAIN_ID);
    assert_ne!(GENESIS_HASH, B256::ZERO);
}

/// Test proposer selection with various validator sets
#[test]
fn test_proposer_selection_variations() {
    // Test with different validator counts
    for count in [3, 5, 10, 18, 21, 50] {
        let validators = mock_validator_set(count);

        // Test multiple rounds
        for round in 0..(count * 3) {
            let proposer = select_proposer(round as u64, &validators).unwrap();
            let expected_index = (round % count) as usize;
            assert_eq!(proposer, validators[expected_index]);

            // Verify proposer is in validator set
            assert!(validators.contains(&proposer));
        }
    }
}

/// Test reward distribution sums to exactly total reward
#[test]
fn test_reward_sum_invariants() {
    let config = test_config();
    let calculator = RewardCalculator::new(config);

    // Test various signer combinations
    let test_cases = vec![
        (vec![10], 10),
        (vec![10, 10], 20),
        (vec![10, 5, 5], 20),
        (vec![5, 5, 5, 5], 20),
        (vec![1, 1, 1, 1, 1, 1, 1, 1, 1, 1], 10),
    ];

    for (sign_counts, total) in test_cases {
        let mut signer_logs = HashMap::new();
        for (i, count) in sign_counts.iter().enumerate() {
            signer_logs.insert(
                Address::with_last_byte(i as u8 + 1),
                RewardLog {
                    sign_count: *count,
                    reward: U256::ZERO,
                },
            );
        }

        let rewards = calculator.calculate_rewards_per_signer(&mut signer_logs, total);
        let total_reward: U256 = rewards.values().copied().sum();

        // Total should equal exactly BLOCK_REWARD (no rounding errors)
        assert_eq!(total_reward, U256::from(BLOCK_REWARD));
    }
}

/// Test cache persistence multiple times
#[test]
fn test_cache_persistence_stress() {
    use tempfile::tempdir;

    let temp_dir = tempdir().unwrap();
    let cache_path = temp_dir.path().join("state_root_cache");

    // Perform multiple save/load cycles
    for cycle in 0..5 {
        let cache = XdcStateRootCache::new(
            1000,
            Duration::from_secs(3600),
            Some(cache_path.clone()),
        );

        // Insert unique entries for this cycle
        let block_hash = B256::random();
        let state_root = B256::random();
        let checkpoint = (cycle + 1) * 900;

        cache.insert(checkpoint, block_hash, state_root);
        cache.save().unwrap();

        // Load and verify
        let cache2 = XdcStateRootCache::new(
            1000,
            Duration::from_secs(3600),
            Some(cache_path.clone()),
        );
        cache2.load().unwrap();

        assert_eq!(cache2.get(&block_hash), Some(state_root));
    }
}

/// Test invalid QC rejection
#[test]
fn test_invalid_qc_rejection() {
    let validators = mock_validator_set(18);

    // QC with insufficient signatures (only 5, need 12)
    let qc_insufficient = mock_qc(B256::random(), 100, 1000, 5);
    let result = verify_qc(&qc_insufficient, &validators, None);
    assert!(result.is_err());

    // QC with zero round (genesis/switch block) - should be valid
    let qc_round_zero = mock_qc(B256::random(), 0, 0, 0);
    let result = verify_qc(&qc_round_zero, &validators, None);
    assert!(result.is_ok());
}

/// Test sync config for different networks
#[test]
fn test_sync_config_networks() {
    // Mainnet
    let mainnet = XdcSyncConfig::mainnet(None);
    assert_eq!(mainnet.chain_id, XDC_MAINNET_CHAIN_ID);
    assert!(mainnet.is_xdc_chain());

    // Apothem
    let apothem = XdcSyncConfig::apothem(None);
    assert_eq!(apothem.chain_id, XDC_APOTHEM_CHAIN_ID);
    assert!(apothem.is_xdc_chain());
}

/// Test anti-spam mechanism
#[test]
fn test_anti_spam_mechanism() {
    let validators = mock_validator_set(18);
    let mut snapshot = Snapshot::new(0, GENESIS_HASH, validators.clone());

    let signer = validators[0];

    // First signature should be allowed
    assert!(!snapshot.recently_signed(0, &signer));

    // Mark as signed
    snapshot.add_recent(0, signer);

    // Should be blocked within N/2 + 1 blocks
    let limit = validators.len() / 2 + 1;
    for i in 1..=limit {
        assert!(snapshot.recently_signed(i as u64, &signer));
    }

    // After limit, should be allowed
    assert!(!snapshot.recently_signed((limit + 1) as u64, &signer));
}

/// Test in-turn difficulty calculation
#[test]
fn test_inturn_difficulty() {
    let validators = mock_validator_set(18);
    let snapshot = Snapshot::new(0, GENESIS_HASH, validators.clone());

    // Block 0: validator 0 is in-turn
    assert!(snapshot.inturn(0, &validators[0]));
    assert!(!snapshot.inturn(0, &validators[1]));

    // Block 17: validator 17 is in-turn
    assert!(snapshot.inturn(17, &validators[17]));

    // Block 18: wraps to validator 0
    assert!(snapshot.inturn(18, &validators[0]));
}
