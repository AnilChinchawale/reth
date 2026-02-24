//! XDPoS V2 BFT Consensus Tests
//!
//! Comprehensive tests for V2 consensus including:
//! - QC/TC verification
//! - Round management
//! - Proposer selection
//! - Extra data encoding/decoding
//! - Epoch switch handling

#[cfg(test)]
mod v2_engine_tests {
    use crate::{
        config::{V2Config, XDPoSConfig},
        errors::XDPoSError,
        v2::{
            engine::XDPoSV2Engine,
            proposer::{select_proposer, is_validator},
            types::{vote_sig_hash, timeout_sig_hash},
            verification::{verify_qc, verify_tc, unique_signatures, CERT_THRESHOLD},
            BlockInfo, QuorumCert, TimeoutCert, VoteForSign, TimeoutForSign,
        },
    };
    use alloy_primitives::{Address, B256};

    fn make_test_config() -> XDPoSConfig {
        XDPoSConfig {
            epoch: 900,
            v2: Some(V2Config {
                switch_block: 23556600,
                cert_threshold: 67, // 67%
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    fn make_validators(count: usize) -> Vec<Address> {
        (0..count)
            .map(|i| Address::with_last_byte(i as u8))
            .collect()
    }

    #[test]
    fn test_v2_engine_initialization() {
        let config = make_test_config();
        let engine = XDPoSV2Engine::new(config);
        
        assert_eq!(engine.current_round(), 0);
        assert!(engine.highest_qc().is_none());
    }

    #[test]
    fn test_v2_block_detection() {
        let config = make_test_config();
        let engine = XDPoSV2Engine::new(config);
        
        // Before V2 switch block
        assert!(!engine.is_v2_block(1000000));
        assert!(!engine.is_v2_block(23556599));
        
        // At and after V2 switch block
        assert!(engine.is_v2_block(23556600));
        assert!(engine.is_v2_block(23556601));
        assert!(engine.is_v2_block(30000000));
    }

    #[test]
    fn test_epoch_boundaries() {
        let config = make_test_config();
        let engine = XDPoSV2Engine::new(config);
        
        // Epoch boundaries
        assert!(engine.is_epoch_switch(0));
        assert!(engine.is_epoch_switch(900));
        assert!(engine.is_epoch_switch(1800));
        assert!(engine.is_epoch_switch(2700));
        
        // Non-epoch blocks
        assert!(!engine.is_epoch_switch(1));
        assert!(!engine.is_epoch_switch(899));
        assert!(!engine.is_epoch_switch(901));
        assert!(!engine.is_epoch_switch(1799));
        
        // Epoch numbers
        assert_eq!(engine.get_epoch(0), 0);
        assert_eq!(engine.get_epoch(899), 0);
        assert_eq!(engine.get_epoch(900), 1);
        assert_eq!(engine.get_epoch(1800), 2);
        assert_eq!(engine.get_epoch(23556600), 26174);
    }

    #[test]
    fn test_round_management() {
        let config = make_test_config();
        let engine = XDPoSV2Engine::new(config);
        
        assert_eq!(engine.current_round(), 0);
        
        engine.set_current_round(100);
        assert_eq!(engine.current_round(), 100);
        
        engine.set_current_round(1000);
        assert_eq!(engine.current_round(), 1000);
    }

    #[test]
    fn test_highest_qc_tracking() {
        let config = make_test_config();
        let engine = XDPoSV2Engine::new(config);
        
        // Initially no QC
        assert!(engine.highest_qc().is_none());
        
        // Set first QC
        let block_info_1 = BlockInfo::new(B256::with_last_byte(1), 100, 1000);
        let qc_1 = QuorumCert::new(block_info_1, 500);
        engine.set_highest_qc(qc_1.clone());
        
        let highest = engine.highest_qc().unwrap();
        assert_eq!(highest.proposed_block_info.round, 100);
        
        // Try lower round - should not update
        let block_info_2 = BlockInfo::new(B256::with_last_byte(2), 50, 900);
        let qc_2 = QuorumCert::new(block_info_2, 500);
        engine.set_highest_qc(qc_2);
        
        let highest = engine.highest_qc().unwrap();
        assert_eq!(highest.proposed_block_info.round, 100);
        
        // Higher round - should update
        let block_info_3 = BlockInfo::new(B256::with_last_byte(3), 200, 1100);
        let qc_3 = QuorumCert::new(block_info_3, 500);
        engine.set_highest_qc(qc_3);
        
        let highest = engine.highest_qc().unwrap();
        assert_eq!(highest.proposed_block_info.round, 200);
    }

    #[test]
    fn test_extra_data_encode_decode_with_qc() {
        let config = make_test_config();
        let engine = XDPoSV2Engine::new(config);
        
        let vanity = [0x42u8; 32];
        let round = 150u64;
        
        // Create a QC
        let block_info = BlockInfo::new(B256::with_last_byte(99), 149, 1000);
        let mut qc = QuorumCert::new(block_info, 500);
        qc.add_signature(vec![1u8; 65]);
        qc.add_signature(vec![2u8; 65]);
        
        let seal = [0xFFu8; 65];
        
        // Encode
        let extra = engine.encode_extra_fields(&vanity, round, Some(&qc), &seal);
        
        // Verify structure
        assert_eq!(&extra[0..32], &vanity);
        assert_eq!(extra[32], 2); // Version byte
        assert_eq!(&extra[extra.len()-65..], &seal);
        
        // Decode
        let decoded = engine.decode_extra_fields(&extra).unwrap();
        assert_eq!(decoded.round, round);
        
        let decoded_qc = decoded.quorum_cert.unwrap();
        assert_eq!(decoded_qc.proposed_block_info.round, 149);
        assert_eq!(decoded_qc.signatures.len(), 2);
    }

    #[test]
    fn test_extra_data_encode_decode_without_qc() {
        let config = make_test_config();
        let engine = XDPoSV2Engine::new(config);
        
        let vanity = [0u8; 32];
        let round = 0u64; // Switch block
        let seal = [0u8; 65];
        
        // Encode without QC
        let extra = engine.encode_extra_fields(&vanity, round, None, &seal);
        
        // Decode
        let decoded = engine.decode_extra_fields(&extra).unwrap();
        assert_eq!(decoded.round, 0);
        assert!(decoded.quorum_cert.is_none());
    }

    #[test]
    fn test_seal_extraction() {
        let config = make_test_config();
        let engine = XDPoSV2Engine::new(config);
        
        let vanity = [0u8; 32];
        let round = 100u64;
        let mut seal = [0u8; 65];
        for i in 0..65 {
            seal[i] = (i + 10) as u8;
        }
        
        let extra = engine.encode_extra_fields(&vanity, round, None, &seal);
        
        let extracted = engine.extract_seal(&extra).unwrap();
        assert_eq!(extracted, seal);
    }

    #[test]
    fn test_proposer_verification() {
        let config = make_test_config();
        let engine = XDPoSV2Engine::new(config);
        let validators = make_validators(18);
        
        // Round 0 -> validator 0
        assert!(engine.verify_proposer(0, &validators[0], &validators).is_ok());
        assert!(engine.verify_proposer(0, &validators[1], &validators).is_err());
        
        // Round 5 -> validator 5
        assert!(engine.verify_proposer(5, &validators[5], &validators).is_ok());
        assert!(engine.verify_proposer(5, &validators[0], &validators).is_err());
        
        // Round 18 -> validator 0 (wraps around)
        assert!(engine.verify_proposer(18, &validators[0], &validators).is_ok());
        assert!(engine.verify_proposer(18, &validators[1], &validators).is_err());
        
        // Round 23 -> validator 5 (23 % 18 = 5)
        assert!(engine.verify_proposer(23, &validators[5], &validators).is_ok());
    }

    #[test]
    fn test_round_monotonicity() {
        let config = make_test_config();
        let engine = XDPoSV2Engine::new(config);
        
        // Valid: increasing rounds
        assert!(engine.verify_round_monotonicity(100, 99).is_ok());
        assert!(engine.verify_round_monotonicity(1000, 1).is_ok());
        
        // Invalid: equal rounds
        assert!(engine.verify_round_monotonicity(100, 100).is_err());
        
        // Invalid: decreasing rounds
        assert!(engine.verify_round_monotonicity(99, 100).is_err());
    }

    #[test]
    fn test_qc_parent_verification() {
        let config = make_test_config();
        let engine = XDPoSV2Engine::new(config);
        
        let parent_hash = B256::with_last_byte(42);
        let parent_number = 1000;
        let parent_round = 150;
        
        // Create QC that matches parent
        let block_info = BlockInfo::new(parent_hash, parent_round, parent_number);
        let qc = QuorumCert::new(block_info, 500);
        
        // Valid: QC matches parent
        assert!(engine.verify_qc_parent(&qc, &parent_hash, parent_number, parent_round).is_ok());
        
        // Invalid: wrong hash
        let wrong_hash = B256::with_last_byte(99);
        assert!(engine.verify_qc_parent(&qc, &wrong_hash, parent_number, parent_round).is_err());
        
        // Invalid: wrong number
        assert!(engine.verify_qc_parent(&qc, &parent_hash, 999, parent_round).is_err());
        
        // Invalid: wrong round
        assert!(engine.verify_qc_parent(&qc, &parent_hash, parent_number, 149).is_err());
    }

    #[test]
    fn test_vote_signature_hash() {
        let block_info = BlockInfo::new(B256::with_last_byte(1), 100, 1000);
        let vote = VoteForSign {
            proposed_block_info: block_info.clone(),
            gap_number: 500,
        };
        
        let hash1 = vote_sig_hash(&vote);
        let hash2 = vote_sig_hash(&vote);
        
        // Deterministic
        assert_eq!(hash1, hash2);
        
        // Not empty
        assert_ne!(hash1, B256::ZERO);
        
        // Different input produces different hash
        let vote2 = VoteForSign {
            proposed_block_info: block_info,
            gap_number: 501,
        };
        let hash3 = vote_sig_hash(&vote2);
        assert_ne!(hash1, hash3);
    }

    #[test]
    fn test_timeout_signature_hash() {
        let timeout = TimeoutForSign {
            round: 100,
            gap_number: 500,
        };
        
        let hash1 = timeout_sig_hash(&timeout);
        let hash2 = timeout_sig_hash(&timeout);
        
        // Deterministic
        assert_eq!(hash1, hash2);
        assert_ne!(hash1, B256::ZERO);
        
        // Different input
        let timeout2 = TimeoutForSign {
            round: 101,
            gap_number: 500,
        };
        let hash3 = timeout_sig_hash(&timeout2);
        assert_ne!(hash1, hash3);
    }

    #[test]
    fn test_unique_signatures_deduplication() {
        let sig1 = vec![1, 2, 3, 4, 5];
        let sig2 = vec![6, 7, 8, 9, 10];
        let sig3 = vec![1, 2, 3, 4, 5]; // Duplicate of sig1
        let sig4 = vec![11, 12, 13, 14, 15];
        
        let signatures = vec![sig1.clone(), sig2.clone(), sig3, sig4.clone()];
        let (unique, duplicates) = unique_signatures(&signatures);
        
        assert_eq!(unique.len(), 3);
        assert_eq!(duplicates.len(), 1);
        
        // Unique should contain sig1, sig2, sig4
        assert!(unique.contains(&sig1));
        assert!(unique.contains(&sig2));
        assert!(unique.contains(&sig4));
    }

    #[test]
    fn test_qc_verification_insufficient_signatures() {
        let validators = make_validators(18);
        
        let block_info = BlockInfo::new(B256::with_last_byte(1), 100, 1000);
        let mut qc = QuorumCert::new(block_info, 500);
        
        // Add only 5 signatures (need 12 for 18 validators with 67% threshold)
        for i in 0..5 {
            qc.add_signature(vec![i; 65]);
        }
        
        let result = verify_qc(&qc, &validators, None);
        assert!(result.is_err());
        
        match result {
            Err(XDPoSError::InsufficientSignatures { have, need }) => {
                assert_eq!(have, 5);
                assert_eq!(need, 12); // ceil(18 * 0.667) = 12
            }
            _ => panic!("Expected InsufficientSignatures error"),
        }
    }

    #[test]
    fn test_qc_verification_round_zero() {
        let validators = make_validators(18);
        
        // Round 0 (genesis/switch block) should pass without signatures
        let block_info = BlockInfo::new(B256::with_last_byte(0), 0, 0);
        let qc = QuorumCert::new(block_info, 0);
        
        let result = verify_qc(&qc, &validators, None);
        assert!(result.is_ok());
    }

    #[test]
    fn test_tc_verification_insufficient_signatures() {
        let validators = make_validators(18);
        
        let mut tc = TimeoutCert::new(200, 500);
        
        // Add only 3 signatures
        for i in 0..3 {
            tc.add_signature(vec![i; 65]);
        }
        
        let result = verify_tc(&tc, &validators, None);
        assert!(result.is_err());
    }

    #[test]
    fn test_custom_threshold() {
        let validators = make_validators(10);
        
        let block_info = BlockInfo::new(B256::with_last_byte(1), 100, 1000);
        let mut qc = QuorumCert::new(block_info, 500);
        
        // Add 5 signatures
        for i in 0..5 {
            qc.add_signature(vec![i; 65]);
        }
        
        // Default threshold (67%) needs ceil(10 * 0.667) = 7 signatures
        assert!(verify_qc(&qc, &validators, None).is_err());
        
        // Custom threshold 50% needs ceil(10 * 0.5) = 5 signatures
        assert!(verify_qc(&qc, &validators, Some(0.5)).is_ok());
        
        // Custom threshold 60% needs ceil(10 * 0.6) = 6 signatures
        assert!(verify_qc(&qc, &validators, Some(0.6)).is_err());
    }

    #[test]
    fn test_proposer_selection_pattern() {
        let validators = make_validators(18);
        
        // Test first cycle
        for round in 0..18 {
            let proposer = select_proposer(round, &validators).unwrap();
            assert_eq!(proposer, validators[round as usize]);
        }
        
        // Test second cycle (wraps around)
        for round in 18..36 {
            let proposer = select_proposer(round, &validators).unwrap();
            let expected_idx = (round % 18) as usize;
            assert_eq!(proposer, validators[expected_idx]);
        }
    }

    #[test]
    fn test_validator_membership() {
        let validators = make_validators(5);
        
        assert!(is_validator(&validators[0], &validators));
        assert!(is_validator(&validators[4], &validators));
        
        let non_validator = Address::with_last_byte(99);
        assert!(!is_validator(&non_validator, &validators));
    }

    #[test]
    fn test_cert_threshold_constant() {
        // Verify threshold is set correctly
        assert_eq!(CERT_THRESHOLD, 0.667);
        
        // Test threshold calculation for different validator counts
        let test_cases = vec![
            (18, 12), // 18 * 0.667 = 12.006 -> ceil = 12
            (21, 14), // 21 * 0.667 = 14.007 -> ceil = 14
            (10, 7),  // 10 * 0.667 = 6.67 -> ceil = 7
            (3, 2),   // 3 * 0.667 = 2.001 -> ceil = 2
        ];
        
        for (validator_count, expected_min) in test_cases {
            let min_sigs = (validator_count as f64 * CERT_THRESHOLD).ceil() as usize;
            assert_eq!(min_sigs, expected_min);
        }
    }

    #[test]
    fn test_multiple_epoch_switches() {
        let config = make_test_config();
        let engine = XDPoSV2Engine::new(config);
        
        // Test first 5 epochs
        for epoch in 0..5 {
            let block = epoch * 900;
            assert!(engine.is_epoch_switch(block));
            assert_eq!(engine.get_epoch(block), epoch);
            
            // One before and after should not be epoch switch
            if block > 0 {
                assert!(!engine.is_epoch_switch(block - 1));
            }
            assert!(!engine.is_epoch_switch(block + 1));
        }
    }

    #[test]
    fn test_invalid_extra_data_cases() {
        let config = make_test_config();
        let engine = XDPoSV2Engine::new(config);
        
        // Too short
        let short = vec![0u8; 50];
        assert!(engine.decode_extra_fields(&short).is_err());
        
        // V1 version byte
        let mut v1_extra = vec![0u8; 200];
        v1_extra[32] = 1; // V1 version at position 32
        assert!(engine.decode_extra_fields(&v1_extra).is_err());
        
        // Empty
        assert!(engine.decode_extra_fields(&[]).is_err());
    }
}
