//! Extra Data Parsing for XDPoS V1
//!
//! Handles parsing and verification of header extra data fields including:
//! - Vanity prefix (32 bytes)
//! - Validator list (20 bytes * N, only at checkpoint blocks)
//! - ECDSA seal signature (65 bytes)

use crate::{
    constants::{EXTRA_SEAL, EXTRA_VANITY},
    errors::{XDPoSError, XDPoSResult},
};
use alloy_consensus::Header;
use alloy_primitives::{Address, B256, keccak256};
use alloy_rlp::Encodable;

/// Parsed V1 extra data structure
#[derive(Debug, Clone, PartialEq)]
pub struct V1ExtraData {
    /// 32 byte vanity prefix
    pub vanity: [u8; 32],
    /// Validator addresses (only present at checkpoint blocks)
    pub validators: Vec<Address>,
    /// 65 byte ECDSA seal signature (R, S, V)
    pub seal: [u8; 65],
}

impl V1ExtraData {
    /// Parse V1 extra data from bytes
    pub fn parse(data: &[u8], is_checkpoint: bool) -> XDPoSResult<Self> {
        // Minimum length check
        if data.len() < EXTRA_VANITY + EXTRA_SEAL {
            return Err(XDPoSError::ExtraDataTooShort);
        }

        // Extract vanity (first 32 bytes)
        let mut vanity = [0u8; 32];
        vanity.copy_from_slice(&data[0..EXTRA_VANITY]);

        // Extract seal (last 65 bytes)
        let mut seal = [0u8; 65];
        seal.copy_from_slice(&data[data.len() - EXTRA_SEAL..]);

        // Extract validators (middle section, only at checkpoints)
        let validators = if is_checkpoint {
            let validators_start = EXTRA_VANITY;
            let validators_end = data.len() - EXTRA_SEAL;
            let validators_data = &data[validators_start..validators_end];

            // Must be multiple of 20 bytes (address size)
            if validators_data.len() % 20 != 0 {
                return Err(XDPoSError::InvalidCheckpointSigners);
            }

            let num_validators = validators_data.len() / 20;
            let mut validators = Vec::with_capacity(num_validators);

            for i in 0..num_validators {
                let start = i * 20;
                let addr = Address::from_slice(&validators_data[start..start + 20]);
                validators.push(addr);
            }

            validators
        } else {
            // Non-checkpoint blocks should have no validator data
            let expected_len = EXTRA_VANITY + EXTRA_SEAL;
            if data.len() != expected_len {
                return Err(XDPoSError::InvalidExtraData);
            }
            Vec::new()
        };

        Ok(Self {
            vanity,
            validators,
            seal,
        })
    }

    /// Encode extra data back to bytes
    pub fn encode(&self) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&self.vanity);
        for validator in &self.validators {
            data.extend_from_slice(validator.as_slice());
        }
        data.extend_from_slice(&self.seal);
        data
    }
}

/// Compute the hash of a header without the seal for signature verification
///
/// This is used to recover the signer's address from the ECDSA signature.
/// The seal is removed from extra_data before hashing.
pub fn hash_without_seal(header: &Header) -> B256 {
    // Create a temporary header with seal removed from extra_data
    let mut temp_header = header.clone();
    
    // Remove the last 65 bytes (seal) from extra_data
    let extra = &header.extra_data;
    if extra.len() >= EXTRA_SEAL {
        temp_header.extra_data = extra[0..extra.len() - EXTRA_SEAL].to_vec().into();
    }

    // Compute keccak256 hash of the RLP-encoded header
    let mut buf = Vec::new();
    temp_header.encode(&mut buf);
    keccak256(&buf)
}

/// Extract the seal signature from header extra data
pub fn extract_seal(header: &Header) -> XDPoSResult<[u8; 65]> {
    let extra = &header.extra_data;
    if extra.len() < EXTRA_SEAL {
        return Err(XDPoSError::MissingSignature);
    }

    let mut seal = [0u8; 65];
    seal.copy_from_slice(&extra[extra.len() - EXTRA_SEAL..]);
    Ok(seal)
}

/// Recover the signer address from a header's seal
///
/// Uses ECDSA recovery on the keccak256 hash of the header (without seal)
pub fn recover_signer(header: &Header) -> XDPoSResult<Address> {
    use secp256k1::{ecdsa::RecoverableSignature, Message, Secp256k1};

    // Get the seal signature
    let seal = extract_seal(header)?;

    // Parse signature: first 64 bytes are R+S, last byte is V (recovery ID)
    let r_s = &seal[0..64];
    let v = seal[64];

    // Convert V to recovery ID (XDC uses Ethereum-style V values)
    // V can be 27/28 (legacy) or chain_id * 2 + 35/36 (EIP-155)
    let recovery_id = if v >= 35 {
        // EIP-155 style
        (v - 35) % 2
    } else {
        // Legacy style (27/28)
        v - 27
    };

    if recovery_id > 3 {
        return Err(XDPoSError::InvalidSignatureFormat);
    }

    // Create recoverable signature
    let sig = RecoverableSignature::from_compact(
        r_s,
        secp256k1::ecdsa::RecoveryId::from_i32(recovery_id as i32)
            .map_err(|_| XDPoSError::InvalidSignatureFormat)?,
    )
    .map_err(|_| XDPoSError::InvalidSignatureFormat)?;

    // Get the message hash (header without seal)
    let msg_hash = hash_without_seal(header);
    let message = Message::from_digest_slice(msg_hash.as_slice())
        .map_err(|_| XDPoSError::InvalidSignatureFormat)?;

    // Recover the public key
    let secp = Secp256k1::new();
    let public_key = secp
        .recover_ecdsa(&message, &sig)
        .map_err(|_| XDPoSError::SignatureVerificationFailed)?;

    // Convert public key to address (last 20 bytes of keccak256(pubkey))
    let pubkey_bytes = public_key.serialize_uncompressed();
    // Skip the first byte (0x04 prefix) and hash the remaining 64 bytes
    let pubkey_hash = keccak256(&pubkey_bytes[1..]);
    
    // Take the last 20 bytes as the address
    Ok(Address::from_slice(&pubkey_hash[12..]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_checkpoint_extra_data() {
        // Create checkpoint extra data: vanity + 2 validators + seal
        let mut data = vec![0u8; EXTRA_VANITY];
        data[0] = 0xaa; // Mark vanity
        
        let val1 = Address::with_last_byte(1);
        let val2 = Address::with_last_byte(2);
        data.extend_from_slice(val1.as_slice());
        data.extend_from_slice(val2.as_slice());
        
        let mut seal = vec![0u8; EXTRA_SEAL];
        seal[0] = 0xff; // Mark seal
        data.extend_from_slice(&seal);

        let parsed = V1ExtraData::parse(&data, true).unwrap();
        
        assert_eq!(parsed.vanity[0], 0xaa);
        assert_eq!(parsed.validators.len(), 2);
        assert_eq!(parsed.validators[0], val1);
        assert_eq!(parsed.validators[1], val2);
        assert_eq!(parsed.seal[0], 0xff);
    }

    #[test]
    fn test_parse_non_checkpoint_extra_data() {
        // Non-checkpoint: only vanity + seal
        let mut data = vec![0u8; EXTRA_VANITY];
        data.extend_from_slice(&vec![0u8; EXTRA_SEAL]);

        let parsed = V1ExtraData::parse(&data, false).unwrap();
        
        assert_eq!(parsed.validators.len(), 0);
    }

    #[test]
    fn test_parse_invalid_checkpoint_extra() {
        // Invalid: checkpoint with non-multiple of 20 bytes for validators
        let mut data = vec![0u8; EXTRA_VANITY];
        data.extend_from_slice(&[0u8; 25]); // 25 bytes, not divisible by 20
        data.extend_from_slice(&[0u8; EXTRA_SEAL]);

        let result = V1ExtraData::parse(&data, true);
        assert!(matches!(result, Err(XDPoSError::InvalidCheckpointSigners)));
    }

    #[test]
    fn test_parse_too_short() {
        let data = vec![0u8; 50]; // Too short
        let result = V1ExtraData::parse(&data, false);
        assert!(matches!(result, Err(XDPoSError::ExtraDataTooShort)));
    }

    #[test]
    fn test_encode_round_trip() {
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
    fn test_hash_without_seal() {
        use alloy_primitives::U256;

        // Create a test header
        let header = Header {
            number: 100,
            timestamp: 1234567890,
            gas_limit: 8_000_000,
            extra_data: {
                let mut data = vec![0u8; EXTRA_VANITY];
                data.extend_from_slice(&[0xff; EXTRA_SEAL]); // Add seal
                data.into()
            },
            ..Default::default()
        };

        // Hash should be deterministic
        let hash1 = hash_without_seal(&header);
        let hash2 = hash_without_seal(&header);
        assert_eq!(hash1, hash2);

        // Hash should not be zero
        assert_ne!(hash1, B256::ZERO);
    }
}
