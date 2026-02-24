//! Test helper utilities for XDPoS consensus testing
//!
//! Provides mock constructors and utilities for creating test fixtures,
//! including V1/V2 headers, validator sets, and transactions.

use crate::{
    constants::{EXTRA_SEAL, EXTRA_VANITY},
    v2::{BlockInfo, QuorumCert, XDPoSV2Engine},
    V1ExtraData, XDPoSConfig,
};
use alloy_consensus::Header;
use alloy_primitives::{Address, Bytes, B256, U256};

/// Create a mock V1 header with valid structure
pub fn mock_v1_header(number: u64, parent_hash: B256) -> Header {
    Header {
        number,
        parent_hash,
        timestamp: 1000 + (number * 2), // 2-second block time
        gas_limit: 8_000_000,
        difficulty: U256::from(2), // In-turn difficulty
        mix_hash: B256::ZERO,
        nonce: 0u64.into(),
        beneficiary: Address::ZERO,
        extra_data: {
            let mut data = vec![0u8; EXTRA_VANITY];
            data.extend_from_slice(&[0u8; EXTRA_SEAL]);
            data.into()
        },
        ..Default::default()
    }
}

/// Create a mock V2 header with QC and valid V2 extra data
pub fn mock_v2_header(
    number: u64,
    parent_hash: B256,
    round: u64,
    qc: Option<&QuorumCert>,
) -> Header {
    let engine = XDPoSV2Engine::new(XDPoSConfig::default());
    let vanity = [0u8; 32];
    let seal = [0u8; 65];

    let extra_data = engine.encode_extra_fields(&vanity, round, qc, &seal);

    Header {
        number,
        parent_hash,
        timestamp: 1000 + (number * 2),
        gas_limit: 8_000_000,
        difficulty: U256::from(2),
        mix_hash: B256::ZERO,
        nonce: 0u64.into(),
        beneficiary: Address::ZERO,
        extra_data: extra_data.into(),
        ..Default::default()
    }
}

/// Create a mock checkpoint header with validator list in extra data
pub fn mock_checkpoint_header(
    number: u64,
    parent_hash: B256,
    validators: &[Address],
) -> Header {
    let is_checkpoint = number % 900 == 0;
    assert!(is_checkpoint, "Number must be checkpoint block");

    let mut data = vec![0u8; EXTRA_VANITY];

    // Add validator addresses for checkpoint blocks
    for validator in validators {
        data.extend_from_slice(validator.as_slice());
    }

    data.extend_from_slice(&[0u8; EXTRA_SEAL]);

    Header {
        number,
        parent_hash,
        timestamp: 1000 + (number * 2),
        gas_limit: 8_000_000,
        difficulty: U256::from(2),
        mix_hash: B256::ZERO,
        nonce: 0u64.into(),
        beneficiary: Address::ZERO, // Checkpoint must have zero beneficiary
        extra_data: data.into(),
        ..Default::default()
    }
}

/// Create a mock validator set for testing
pub fn mock_validator_set(count: usize) -> Vec<Address> {
    (0..count)
        .map(|i| Address::with_last_byte(i as u8 + 1))
        .collect()
}

/// Create a mock signing transaction to 0x89 contract
pub fn mock_signing_tx(from: Address, block_hash: B256) -> MockTransaction {
    use crate::reward::SIGN_METHOD_SIG;

    let mut data = Vec::new();
    data.extend_from_slice(&SIGN_METHOD_SIG); // e341eaa4
    data.extend_from_slice(block_hash.as_slice()); // Block hash parameter

    MockTransaction {
        from,
        to: Some(crate::reward::BLOCK_SIGNERS_ADDRESS),
        value: U256::ZERO,
        gas: 21000,
        gas_price: U256::from(1_000_000_000u64), // 1 gwei
        data: data.into(),
    }
}

/// Mock transaction for testing
#[derive(Debug, Clone)]
pub struct MockTransaction {
    pub from: Address,
    pub to: Option<Address>,
    pub value: U256,
    pub gas: u64,
    pub gas_price: U256,
    pub data: Bytes,
}

impl MockTransaction {
    /// Create a simple transfer transaction
    pub fn transfer(from: Address, to: Address, value: U256) -> Self {
        Self {
            from,
            to: Some(to),
            value,
            gas: 21000,
            gas_price: U256::from(1_000_000_000u64),
            data: Bytes::new(),
        }
    }

    /// Create a contract call transaction
    pub fn contract_call(from: Address, to: Address, data: Bytes) -> Self {
        Self {
            from,
            to: Some(to),
            value: U256::ZERO,
            gas: 100_000,
            gas_price: U256::from(1_000_000_000u64),
            data,
        }
    }

    /// Check if this is a signing transaction
    pub fn is_signing_tx(&self) -> bool {
        if let Some(to) = self.to {
            crate::reward::is_signing_tx(&to, &self.data)
        } else {
            false
        }
    }
}

/// Create a mock QC for testing
pub fn mock_qc(block_hash: B256, round: u64, number: u64, signature_count: usize) -> QuorumCert {
    let block_info = BlockInfo::new(block_hash, round, number);
    let mut qc = QuorumCert::new(block_info, 450); // Gap number

    // Add mock signatures
    for i in 0..signature_count {
        let mut sig = vec![0u8; 65];
        sig[0] = i as u8; // Make each signature unique
        qc.add_signature(sig);
    }

    qc
}

/// Create a signed V1 header (for seal verification testing)
pub fn mock_signed_v1_header(
    number: u64,
    parent_hash: B256,
    signer_key: &[u8; 32],
) -> (Header, Address) {
    use crate::extra_data::hash_without_seal;
    use alloy_primitives::keccak256;
    use secp256k1::{Message, PublicKey, Secp256k1, SecretKey};

    let mut header = mock_v1_header(number, parent_hash);

    // Create secret key
    let secret_key = SecretKey::from_slice(signer_key).expect("Valid secret key");

    // Get hash to sign
    let msg_hash = hash_without_seal(&header);
    let message = Message::from_digest_slice(msg_hash.as_slice()).expect("Valid message");

    // Sign
    let secp = Secp256k1::new();
    let sig = secp.sign_ecdsa_recoverable(&message, &secret_key);
    let (recovery_id, sig_bytes) = sig.serialize_compact();

    // Build extra data with signature
    let mut extra_data = vec![0u8; EXTRA_VANITY];
    extra_data.extend_from_slice(&sig_bytes);
    extra_data.push(recovery_id.to_i32() as u8 + 27);
    header.extra_data = extra_data.into();

    // Derive address
    let public_key = PublicKey::from_secret_key(&secp, &secret_key);
    let pubkey_bytes = public_key.serialize_uncompressed();
    let pubkey_hash = keccak256(&pubkey_bytes[1..]);
    let address = Address::from_slice(&pubkey_hash[12..]);

    (header, address)
}

/// Create test config with common settings
pub fn test_config() -> XDPoSConfig {
    XDPoSConfig {
        period: 2,
        epoch: 900,
        reward: 250_000_000_000_000_000_000,
        reward_checkpoint: 900,
        gap: 450,
        foundation_wallet: Address::with_last_byte(0xFF),
        v2: None,
    }
}

/// Create test config with V2 enabled
pub fn test_config_v2(switch_block: u64) -> XDPoSConfig {
    XDPoSConfig {
        period: 2,
        epoch: 900,
        reward: 250_000_000_000_000_000_000,
        reward_checkpoint: 900,
        gap: 450,
        foundation_wallet: Address::with_last_byte(0xFF),
        v2: Some(crate::config::V2Config {
            switch_block,
            cert_threshold: 0.667,
            timeout_period: 30_000,
            min_timeout: 10_000,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_v1_header() {
        let header = mock_v1_header(100, B256::ZERO);
        assert_eq!(header.number, 100);
        assert_eq!(header.parent_hash, B256::ZERO);
        assert_eq!(header.gas_limit, 8_000_000);
        assert_eq!(header.extra_data.len(), EXTRA_VANITY + EXTRA_SEAL);
    }

    #[test]
    fn test_mock_checkpoint_header() {
        let validators = mock_validator_set(5);
        let header = mock_checkpoint_header(900, B256::ZERO, &validators);

        assert_eq!(header.number, 900);
        assert_eq!(header.beneficiary, Address::ZERO);

        // Verify validator addresses are in extra data
        let extra = V1ExtraData::parse(&header.extra_data, true).unwrap();
        assert_eq!(extra.validators.len(), 5);
        assert_eq!(extra.validators, validators);
    }

    #[test]
    fn test_mock_validator_set() {
        let validators = mock_validator_set(18);
        assert_eq!(validators.len(), 18);

        // Check they're unique
        let unique: std::collections::HashSet<_> = validators.iter().collect();
        assert_eq!(unique.len(), 18);
    }

    #[test]
    fn test_mock_signing_tx() {
        let signer = Address::with_last_byte(1);
        let block_hash = B256::random();
        let tx = mock_signing_tx(signer, block_hash);

        assert!(tx.is_signing_tx());
        assert_eq!(tx.to, Some(crate::reward::BLOCK_SIGNERS_ADDRESS));
        assert_eq!(tx.from, signer);
    }

    #[test]
    fn test_mock_qc() {
        let qc = mock_qc(B256::random(), 100, 1000, 12);
        assert_eq!(qc.proposed_block_info.round, 100);
        assert_eq!(qc.proposed_block_info.number, 1000);
        assert_eq!(qc.signatures.len(), 12);
    }

    #[test]
    fn test_mock_signed_v1_header_roundtrip() {
        use crate::extra_data::recover_signer;

        let key = [1u8; 32];
        let (header, expected_address) = mock_signed_v1_header(100, B256::ZERO, &key);

        // Recover signer from header
        let recovered = recover_signer(&header).unwrap();
        assert_eq!(recovered, expected_address);
    }

    #[test]
    fn test_mock_transaction_helpers() {
        let from = Address::with_last_byte(1);
        let to = Address::with_last_byte(2);

        let transfer = MockTransaction::transfer(from, to, U256::from(1000));
        assert_eq!(transfer.value, U256::from(1000));
        assert_eq!(transfer.to, Some(to));
        assert!(!transfer.is_signing_tx());

        let call = MockTransaction::contract_call(from, to, Bytes::from(vec![1, 2, 3]));
        assert_eq!(call.gas, 100_000);
        assert_eq!(call.data.len(), 3);
    }

    #[test]
    fn test_test_config() {
        let config = test_config();
        assert_eq!(config.epoch, 900);
        assert_eq!(config.period, 2);
        assert_eq!(config.gap, 450);
        assert!(config.v2.is_none());
    }

    #[test]
    fn test_test_config_v2() {
        let config = test_config_v2(23556600);
        assert!(config.v2.is_some());
        assert_eq!(config.v2_switch_block(), Some(23556600));
    }

    #[test]
    fn test_mock_v2_header_structure() {
        let qc = mock_qc(B256::random(), 99, 999, 12);
        let header = mock_v2_header(1000, B256::random(), 100, Some(&qc));

        assert_eq!(header.number, 1000);
        assert!(header.extra_data.len() > EXTRA_VANITY + EXTRA_SEAL);

        // Verify it can be decoded
        let engine = XDPoSV2Engine::new(XDPoSConfig::default());
        let decoded = engine.decode_extra_fields(&header.extra_data).unwrap();
        assert_eq!(decoded.round, 100);
        assert!(decoded.quorum_cert.is_some());
    }
}
