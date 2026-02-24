//! Tests for XDPoS V1 seal verification, snapshot system, and extra data parsing

use crate::{
    config::XDPoSConfig,
    constants::{EXTRA_SEAL, EXTRA_VANITY},
    errors::XDPoSError,
    extra_data::{hash_without_seal, recover_signer, V1ExtraData},
    snapshot::Snapshot,
    v1::validate_v1_header,
};
use alloy_consensus::Header;
use alloy_primitives::{Address, B256, U256};

/// Helper to create a test config
fn test_config() -> XDPoSConfig {
    XDPoSConfig {
        period: 2,
        epoch: 900,
        ..Default::default()
    }
}

/// Helper to create test signers
fn test_signers(count: usize) -> Vec<Address> {
    (1..=count).map(|i| Address::with_last_byte(i as u8)).collect()
}

#[test]
fn test_parse_v1_extra_data() {
    // Test checkpoint block with validators
    let mut data = vec![0xaa; EXTRA_VANITY];
    let val1 = Address::with_last_byte(1);
    let val2 = Address::with_last_byte(2);
    data.extend_from_slice(val1.as_slice());
    data.extend_from_slice(val2.as_slice());
    data.extend_from_slice(&vec![0xff; EXTRA_SEAL]);

    let parsed = V1ExtraData::parse(&data, true).unwrap();
    assert_eq!(parsed.vanity[0], 0xaa);
    assert_eq!(parsed.validators.len(), 2);
    assert_eq!(parsed.validators[0], val1);
    assert_eq!(parsed.validators[1], val2);
    assert_eq!(parsed.seal[0], 0xff);

    // Test non-checkpoint block (no validators)
    let mut data_non_cp = vec![0xbb; EXTRA_VANITY];
    data_non_cp.extend_from_slice(&vec![0xcc; EXTRA_SEAL]);

    let parsed_non_cp = V1ExtraData::parse(&data_non_cp, false).unwrap();
    assert_eq!(parsed_non_cp.vanity[0], 0xbb);
    assert_eq!(parsed_non_cp.validators.len(), 0);
    assert_eq!(parsed_non_cp.seal[0], 0xcc);
}

#[test]
fn test_parse_invalid_extra_data() {
    // Too short
    let short_data = vec![0u8; 50];
    assert!(matches!(
        V1ExtraData::parse(&short_data, false),
        Err(XDPoSError::ExtraDataTooShort)
    ));

    // Invalid validator count (not multiple of 20)
    let mut invalid_data = vec![0u8; EXTRA_VANITY];
    invalid_data.extend_from_slice(&[0u8; 25]); // 25 bytes, not divisible by 20
    invalid_data.extend_from_slice(&[0u8; EXTRA_SEAL]);

    assert!(matches!(
        V1ExtraData::parse(&invalid_data, true),
        Err(XDPoSError::InvalidCheckpointSigners)
    ));

    // Non-checkpoint with validator data
    let mut extra_data = vec![0u8; EXTRA_VANITY];
    extra_data.extend_from_slice(&[0u8; 20]); // Extra 20 bytes (should not be there)
    extra_data.extend_from_slice(&[0u8; EXTRA_SEAL]);

    assert!(matches!(
        V1ExtraData::parse(&extra_data, false),
        Err(XDPoSError::InvalidExtraData)
    ));
}

#[test]
fn test_extra_data_encode_decode() {
    let original = V1ExtraData {
        vanity: [0xaa; 32],
        validators: vec![Address::with_last_byte(1), Address::with_last_byte(2)],
        seal: [0xff; 65],
    };

    let encoded = original.encode();
    let decoded = V1ExtraData::parse(&encoded, true).unwrap();

    assert_eq!(original, decoded);
}

#[test]
fn test_hash_without_seal_deterministic() {
    let header = Header {
        number: 100,
        timestamp: 1234567890,
        gas_limit: 8_000_000,
        extra_data: {
            let mut data = vec![0u8; EXTRA_VANITY];
            data.extend_from_slice(&[0xff; EXTRA_SEAL]);
            data.into()
        },
        ..Default::default()
    };

    let hash1 = hash_without_seal(&header);
    let hash2 = hash_without_seal(&header);

    assert_eq!(hash1, hash2);
    assert_ne!(hash1, B256::ZERO);
}

#[test]
fn test_hash_without_seal_removes_signature() {
    let mut header1 = Header {
        number: 100,
        extra_data: {
            let mut data = vec![0xaa; EXTRA_VANITY];
            data.extend_from_slice(&[0x11; EXTRA_SEAL]); // Signature 1
            data.into()
        },
        ..Default::default()
    };

    let mut header2 = header1.clone();
    header2.extra_data = {
        let mut data = vec![0xaa; EXTRA_VANITY];
        data.extend_from_slice(&[0x22; EXTRA_SEAL]); // Signature 2 (different)
        data.into()
    };

    // Hashes should be the same since seal is removed
    let hash1 = hash_without_seal(&header1);
    let hash2 = hash_without_seal(&header2);
    assert_eq!(hash1, hash2);
}

#[test]
fn test_snapshot_apply() {
    let signers = test_signers(3);
    let snapshot = Snapshot::new(0, B256::ZERO, signers.clone());

    // Create a header at block 1
    let header = Header {
        number: 1,
        parent_hash: B256::ZERO,
        timestamp: 1000,
        beneficiary: Address::ZERO, // No vote
        extra_data: {
            let mut data = vec![0u8; EXTRA_VANITY];
            data.extend_from_slice(&[0u8; EXTRA_SEAL]);
            data.into()
        },
        ..Default::default()
    };

    // Apply the header
    let new_snap = snapshot.apply(&header, None, 900).unwrap();

    assert_eq!(new_snap.number, 1);
    assert_eq!(new_snap.signer_count(), 3);
}

#[test]
fn test_snapshot_apply_checkpoint() {
    let initial_signers = test_signers(3);
    let snapshot = Snapshot::new(0, B256::ZERO, initial_signers);

    // Create a checkpoint header at block 900
    let new_signers = test_signers(5); // Different validator set
    let header = Header {
        number: 900,
        beneficiary: Address::ZERO,
        extra_data: {
            let mut data = vec![0u8; EXTRA_VANITY];
            for signer in &new_signers {
                data.extend_from_slice(signer.as_slice());
            }
            data.extend_from_slice(&[0u8; EXTRA_SEAL]);
            data.into()
        },
        ..Default::default()
    };

    // Apply checkpoint
    let new_snap = snapshot
        .apply(&header, Some(new_signers.clone()), 900)
        .unwrap();

    assert_eq!(new_snap.number, 900);
    assert_eq!(new_snap.signer_count(), 5);
    assert!(new_snap.is_signer(&Address::with_last_byte(5)));
}

#[test]
fn test_anti_spam() {
    let signers = test_signers(3);
    let mut snapshot = Snapshot::new(0, B256::ZERO, signers.clone());

    let signer1 = Address::with_last_byte(1);

    // Signer signs block 1
    snapshot.add_recent(1, signer1);

    // Signer should not be able to sign within limit
    assert!(snapshot.recently_signed(2, &signer1));
    assert!(snapshot.recently_signed(3, &signer1));

    // After limit (3 signers means limit = 3 blocks)
    assert!(!snapshot.recently_signed(5, &signer1));
}

#[test]
fn test_anti_spam_with_apply() {
    let signers = test_signers(3);
    let snapshot = Snapshot::new(0, B256::ZERO, signers.clone());

    let signer1 = Address::with_last_byte(1);

    // Create header at block 1
    let header1 = Header {
        number: 1,
        beneficiary: Address::ZERO,
        extra_data: {
            let mut data = vec![0u8; EXTRA_VANITY];
            data.extend_from_slice(&[0u8; EXTRA_SEAL]);
            data.into()
        },
        ..Default::default()
    };

    // Apply with signer1
    let snap1 = snapshot
        .apply_with_signer(&header1, signer1, None, 900)
        .unwrap();

    // Signer1 should be in recents
    assert!(snap1.recently_signed(2, &signer1));

    // Try to apply again with same signer (should fail)
    let header2 = Header {
        number: 2,
        beneficiary: Address::ZERO,
        extra_data: {
            let mut data = vec![0u8; EXTRA_VANITY];
            data.extend_from_slice(&[0u8; EXTRA_SEAL]);
            data.into()
        },
        ..Default::default()
    };

    let result = snap1.apply_with_signer(&header2, signer1, None, 900);
    assert!(matches!(result, Err(XDPoSError::Unauthorized)));
}

#[test]
fn test_inturn_calculation() {
    let signers = test_signers(3);
    let snapshot = Snapshot::new(0, B256::ZERO, signers);

    // Block 0: signer 0 (address ending in 1) is in-turn
    assert!(snapshot.inturn(0, &Address::with_last_byte(1)));
    assert!(!snapshot.inturn(0, &Address::with_last_byte(2)));

    // Block 1: signer 1 (address ending in 2) is in-turn
    assert!(!snapshot.inturn(1, &Address::with_last_byte(1)));
    assert!(snapshot.inturn(1, &Address::with_last_byte(2)));

    // Block 3: wraps around to signer 0
    assert!(snapshot.inturn(3, &Address::with_last_byte(1)));
}

#[test]
fn test_checkpoint_validation() {
    let config = test_config();
    let signers = test_signers(3);
    let snapshot = Snapshot::new(0, B256::ZERO, signers);

    // Valid checkpoint header
    let checkpoint_header = Header {
        number: 900,
        beneficiary: Address::ZERO, // Must be zero
        mix_hash: B256::ZERO,
        nonce: 0u64.into(),
        timestamp: 1000,
        difficulty: U256::from(2),
        extra_data: {
            let mut data = vec![0u8; EXTRA_VANITY];
            data.extend_from_slice(Address::with_last_byte(1).as_slice());
            data.extend_from_slice(Address::with_last_byte(2).as_slice());
            data.extend_from_slice(&[0u8; EXTRA_SEAL]);
            data.into()
        },
        ..Default::default()
    };

    // Note: We can't fully test validate_v1_header without a real signature,
    // but we can test the checkpoint structure parsing
    let parsed = V1ExtraData::parse(&checkpoint_header.extra_data, true).unwrap();
    assert_eq!(parsed.validators.len(), 2);

    // Invalid checkpoint: non-zero beneficiary
    let invalid_checkpoint = Header {
        number: 900,
        beneficiary: Address::with_last_byte(1), // Should be zero!
        extra_data: checkpoint_header.extra_data.clone(),
        ..Default::default()
    };

    let result = validate_v1_header(&invalid_checkpoint, &config, None, Some(&snapshot));
    assert!(matches!(
        result,
        Err(XDPoSError::InvalidCheckpointBeneficiary)
    ));
}

#[test]
fn test_voting_system() {
    let signers = test_signers(3);
    let mut snapshot = Snapshot::new(0, B256::ZERO, signers);

    let new_candidate = Address::with_last_byte(99);

    // Cast votes to authorize new candidate
    // Need majority: 3/2 + 1 = 2 votes
    assert!(snapshot.cast_vote(new_candidate, true));
    assert_eq!(snapshot.tally.get(&new_candidate).unwrap().votes, 1);

    assert!(snapshot.cast_vote(new_candidate, true));
    assert_eq!(snapshot.tally.get(&new_candidate).unwrap().votes, 2);

    // Apply votes (should reach threshold and add candidate)
    let modified = snapshot.apply_votes();
    assert!(modified);
    assert!(snapshot.is_signer(&new_candidate));
    assert_eq!(snapshot.signer_count(), 4);

    // Vote should be cleared after successful authorization
    assert!(!snapshot.tally.contains_key(&new_candidate));
}

#[test]
fn test_deauthorize_voting() {
    let signers = test_signers(3);
    let mut snapshot = Snapshot::new(0, B256::ZERO, signers.clone());

    let target = Address::with_last_byte(1);

    // Cast votes to deauthorize existing signer
    assert!(snapshot.cast_vote(target, false));
    assert!(snapshot.cast_vote(target, false));

    // Apply votes
    let modified = snapshot.apply_votes();
    assert!(modified);
    assert!(!snapshot.is_signer(&target));
    assert_eq!(snapshot.signer_count(), 2);
}

#[test]
fn test_invalid_votes() {
    let signers = test_signers(3);
    let mut snapshot = Snapshot::new(0, B256::ZERO, signers);

    // Try to authorize already-authorized signer (invalid)
    let existing = Address::with_last_byte(1);
    assert!(!snapshot.valid_vote(&existing, true));

    // Try to deauthorize non-existent signer (invalid)
    let non_existent = Address::with_last_byte(99);
    assert!(!snapshot.valid_vote(&non_existent, false));

    // Valid votes
    assert!(snapshot.valid_vote(&non_existent, true)); // Authorize new
    assert!(snapshot.valid_vote(&existing, false)); // Deauthorize existing
}

#[cfg(test)]
mod seal_verification_tests {
    use super::*;
    use secp256k1::{Message, Secp256k1, SecretKey};
    use alloy_primitives::keccak256;

    /// Create a signed header for testing
    /// Note: This is a simplified version for testing purposes
    fn create_signed_header(
        number: u64,
        secret_key: &SecretKey,
    ) -> (Header, Address) {
        use secp256k1::PublicKey;

        // Create basic header
        let mut header = Header {
            number,
            timestamp: 1000 + number,
            gas_limit: 8_000_000,
            extra_data: vec![0u8; EXTRA_VANITY].into(),
            ..Default::default()
        };

        // Get the hash to sign
        let msg_hash = hash_without_seal(&header);
        let message = Message::from_digest_slice(msg_hash.as_slice()).unwrap();

        // Sign with secret key
        let secp = Secp256k1::new();
        let sig = secp.sign_ecdsa_recoverable(&message, secret_key);
        let (recovery_id, sig_bytes) = sig.serialize_compact();

        // Create seal: signature (64 bytes) + recovery_id (1 byte)
        let mut seal = vec![0u8; EXTRA_VANITY];
        seal.extend_from_slice(&sig_bytes);
        seal.push(recovery_id.to_i32() as u8 + 27); // Add 27 for Ethereum compatibility
        header.extra_data = seal.into();

        // Derive address from public key
        let public_key = PublicKey::from_secret_key(&secp, secret_key);
        let pubkey_bytes = public_key.serialize_uncompressed();
        let pubkey_hash = keccak256(&pubkey_bytes[1..]);
        let address = Address::from_slice(&pubkey_hash[12..]);

        (header, address)
    }

    #[test]
    fn test_seal_verification_roundtrip() {
        // Create a secret key for testing
        let secret_key = SecretKey::from_slice(&[1u8; 32]).unwrap();

        // Create and sign a header
        let (header, expected_address) = create_signed_header(1, &secret_key);

        // Recover the signer
        let recovered_address = recover_signer(&header).unwrap();

        // Should match the expected address
        assert_eq!(recovered_address, expected_address);
    }

    #[test]
    fn test_seal_verification_different_keys() {
        let key1 = SecretKey::from_slice(&[1u8; 32]).unwrap();
        let key2 = SecretKey::from_slice(&[2u8; 32]).unwrap();

        let (header1, addr1) = create_signed_header(1, &key1);
        let (header2, addr2) = create_signed_header(1, &key2);

        // Different keys should produce different addresses
        assert_ne!(addr1, addr2);

        // Recovery should work for both
        assert_eq!(recover_signer(&header1).unwrap(), addr1);
        assert_eq!(recover_signer(&header2).unwrap(), addr2);
    }
}
