//! XDPoS V2 Signature Verification
//!
//! This module implements signature verification for:
//! - Quorum Certificates (QC)
//! - Timeout Certificates (TC)
//! - Vote messages
//! - Timeout messages

use super::{QuorumCert, TimeoutCert, VoteForSign, TimeoutForSign};
use crate::{
    errors::{XDPoSError, XDPoSResult},
    v2::types::{vote_sig_hash, timeout_sig_hash},
};
use alloy_primitives::{Address, B256, Signature};
use rayon::prelude::*;
use std::collections::HashSet;

/// Threshold for BFT consensus (2/3 + 1)
pub const CERT_THRESHOLD: f64 = 0.667; // 66.7%

/// Recover the signer address from a signature
///
/// # Arguments
/// * `hash` - The message hash that was signed
/// * `signature` - The signature bytes (65 bytes: r, s, v)
///
/// # Returns
/// The recovered signer address or an error
pub fn recover_signer(hash: &B256, signature: &[u8]) -> XDPoSResult<Address> {
    if signature.len() != 65 {
        return Err(XDPoSError::InvalidSignatureFormat);
    }

    // Parse signature
    let sig = Signature::try_from(signature)
        .map_err(|_| XDPoSError::InvalidSignatureFormat)?;

    // Recover the address
    sig.recover_address_from_prehash(hash)
        .map_err(|_| XDPoSError::SignatureVerificationFailed)
}

/// Verify a signature against a list of masternodes
///
/// # Arguments
/// * `hash` - The message hash
/// * `signature` - The signature to verify
/// * `masternodes` - List of valid masternode addresses
///
/// # Returns
/// (is_valid, signer_address)
pub fn verify_signature(
    hash: &B256,
    signature: &[u8],
    masternodes: &[Address],
) -> XDPoSResult<(bool, Address)> {
    if masternodes.is_empty() {
        return Err(XDPoSError::Custom("empty masternode list".to_string()));
    }

    let signer = recover_signer(hash, signature)?;
    let is_valid = masternodes.contains(&signer);

    Ok((is_valid, signer))
}

/// Remove duplicate signatures and return unique ones + duplicates
pub fn unique_signatures(signatures: &[Vec<u8>]) -> (Vec<Vec<u8>>, Vec<Vec<u8>>) {
    let mut seen = HashSet::new();
    let mut unique = Vec::new();
    let mut duplicates = Vec::new();

    for sig in signatures {
        let sig_hash = alloy_primitives::keccak256(sig);
        if seen.insert(sig_hash) {
            unique.push(sig.clone());
        } else {
            duplicates.push(sig.clone());
        }
    }

    (unique, duplicates)
}

/// Recover unique signers from a list of signatures (parallel)
///
/// # Arguments
/// * `hash` - The signed message hash
/// * `signatures` - List of signatures
///
/// # Returns
/// (unique_signatures, duplicate_signatures, unique_signers)
pub fn recover_unique_signers(
    hash: &B256,
    signatures: &[Vec<u8>],
) -> XDPoSResult<(Vec<Vec<u8>>, Vec<Vec<u8>>, Vec<Address>)> {
    if signatures.is_empty() {
        return Ok((Vec::new(), Vec::new(), Vec::new()));
    }

    // Parallel recovery
    let signers: Vec<XDPoSResult<(Vec<u8>, Address)>> = signatures
        .par_iter()
        .map(|sig| {
            let addr = recover_signer(hash, sig)?;
            Ok((sig.clone(), addr))
        })
        .collect();

    // Check for errors
    for result in &signers {
        if let Err(e) = result {
            return Err(e.clone());
        }
    }

    // Extract unique signers
    let mut seen_addresses = HashSet::new();
    let mut unique_sigs = Vec::new();
    let mut duplicate_sigs = Vec::new();
    let mut unique_addrs = Vec::new();

    for result in signers {
        let (sig, addr) = result.unwrap();
        if seen_addresses.insert(addr) {
            unique_sigs.push(sig);
            unique_addrs.push(addr);
        } else {
            duplicate_sigs.push(sig);
        }
    }

    Ok((unique_sigs, duplicate_sigs, unique_addrs))
}

/// Verify a Quorum Certificate
///
/// # Arguments
/// * `qc` - The quorum certificate to verify
/// * `masternodes` - Valid masternode addresses for this epoch
/// * `threshold` - Custom threshold (use CERT_THRESHOLD if None)
///
/// # Returns
/// Ok if QC is valid, Err otherwise
pub fn verify_qc(
    qc: &QuorumCert,
    masternodes: &[Address],
    threshold: Option<f64>,
) -> XDPoSResult<()> {
    if masternodes.is_empty() {
        return Err(XDPoSError::Custom("empty masternode list".to_string()));
    }

    let threshold = threshold.unwrap_or(CERT_THRESHOLD);
    let min_signatures = (masternodes.len() as f64 * threshold).ceil() as usize;

    // Check round 0 (genesis/switch block) - may have no signatures
    if qc.proposed_block_info.round == 0 {
        return Ok(());
    }

    // Remove duplicates
    let (unique_sigs, duplicates) = unique_signatures(&qc.signatures);
    
    if !duplicates.is_empty() {
        tracing::warn!(
            "Found {} duplicate signatures in QC for block {}",
            duplicates.len(),
            qc.proposed_block_info.number
        );
    }

    // Check signature count meets threshold
    if unique_sigs.len() < min_signatures {
        return Err(XDPoSError::InsufficientSignatures {
            have: unique_sigs.len(),
            need: min_signatures,
        });
    }

    // Compute the vote signature hash
    let vote_for_sign = VoteForSign {
        proposed_block_info: qc.proposed_block_info.clone(),
        gap_number: qc.gap_number,
    };
    let sig_hash = vote_sig_hash(&vote_for_sign);

    // Verify each signature in parallel
    let results: Vec<XDPoSResult<bool>> = unique_sigs
        .par_iter()
        .map(|sig| {
            let (is_valid, signer) = verify_signature(&sig_hash, sig, masternodes)?;
            if !is_valid {
                tracing::warn!(
                    "Invalid QC signature from non-masternode: {:?}",
                    signer
                );
                return Ok(false);
            }
            Ok(true)
        })
        .collect();

    // Check for errors and count valid signatures
    let mut valid_count = 0;
    for result in results {
        match result {
            Ok(true) => valid_count += 1,
            Ok(false) => {
                return Err(XDPoSError::InvalidQCSignatures(
                    "signature from non-masternode".to_string()
                ));
            }
            Err(e) => return Err(e),
        }
    }

    // Final check
    if valid_count < min_signatures {
        return Err(XDPoSError::InsufficientSignatures {
            have: valid_count,
            need: min_signatures,
        });
    }

    Ok(())
}

/// Verify a Timeout Certificate
///
/// # Arguments
/// * `tc` - The timeout certificate to verify
/// * `masternodes` - Valid masternode addresses for this epoch
/// * `threshold` - Custom threshold (use CERT_THRESHOLD if None)
///
/// # Returns
/// Ok if TC is valid, Err otherwise
pub fn verify_tc(
    tc: &TimeoutCert,
    masternodes: &[Address],
    threshold: Option<f64>,
) -> XDPoSResult<()> {
    if masternodes.is_empty() {
        return Err(XDPoSError::Custom("empty masternode list".to_string()));
    }

    let threshold = threshold.unwrap_or(CERT_THRESHOLD);
    let min_signatures = (masternodes.len() as f64 * threshold).ceil() as usize;

    // Remove duplicates
    let (unique_sigs, duplicates) = unique_signatures(&tc.signatures);
    
    if !duplicates.is_empty() {
        tracing::warn!(
            "Found {} duplicate signatures in TC for round {}",
            duplicates.len(),
            tc.round
        );
    }

    // Check signature count
    if unique_sigs.len() < min_signatures {
        return Err(XDPoSError::InsufficientSignatures {
            have: unique_sigs.len(),
            need: min_signatures,
        });
    }

    // Compute the timeout signature hash
    let timeout_for_sign = TimeoutForSign {
        round: tc.round,
        gap_number: tc.gap_number,
    };
    let sig_hash = timeout_sig_hash(&timeout_for_sign);

    // Verify each signature in parallel
    let results: Vec<XDPoSResult<bool>> = unique_sigs
        .par_iter()
        .map(|sig| {
            let (is_valid, signer) = verify_signature(&sig_hash, sig, masternodes)?;
            if !is_valid {
                tracing::warn!(
                    "Invalid TC signature from non-masternode: {:?}",
                    signer
                );
                return Ok(false);
            }
            Ok(true)
        })
        .collect();

    // Check results
    let mut valid_count = 0;
    for result in results {
        match result {
            Ok(true) => valid_count += 1,
            Ok(false) => {
                return Err(XDPoSError::InvalidTCSignatures);
            }
            Err(e) => return Err(e),
        }
    }

    if valid_count < min_signatures {
        return Err(XDPoSError::InsufficientSignatures {
            have: valid_count,
            need: min_signatures,
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::hex;

    #[test]
    fn test_cert_threshold() {
        assert_eq!(CERT_THRESHOLD, 0.667);
        
        // Test threshold calculation
        let masternodes_count = 18;
        let min_sigs = (masternodes_count as f64 * CERT_THRESHOLD).ceil() as usize;
        assert_eq!(min_sigs, 12); // 18 * 0.667 = 12.006, ceil = 12
    }

    #[test]
    fn test_recover_signer() {
        // Test vector from Ethereum
        let hash = B256::from(hex!(
            "82ff40c0a986c6a5cfad4ddf4c3aa6996f1a7837f9c398e17e5de5cbd5a12b28"
        ));
        let sig = hex!(
            "3eb24bd327df8c2b614c3f652ec86efe13aa721daf203820241fe6e2c84a2c701d95c02a3c9ce28dc5d1174cda2ea9a85e1bcb95a80ec69c6e39f1"
        );
        
        // This should fail because it's an invalid signature format (wrong length)
        let result = recover_signer(&hash, &sig);
        assert!(result.is_err());
    }

    #[test]
    fn test_unique_signatures() {
        let sig1 = vec![1, 2, 3];
        let sig2 = vec![4, 5, 6];
        let sig3 = vec![1, 2, 3]; // Duplicate of sig1
        
        let signatures = vec![sig1.clone(), sig2.clone(), sig3];
        let (unique, duplicates) = unique_signatures(&signatures);
        
        assert_eq!(unique.len(), 2);
        assert_eq!(duplicates.len(), 1);
    }

    #[test]
    fn test_verify_qc_insufficient_signatures() {
        let block_info = BlockInfo::new(B256::with_last_byte(1), 100, 1000);
        let mut qc = QuorumCert::new(block_info, 500);
        
        // Add only 1 signature (need 12 for 18 masternodes)
        qc.add_signature(vec![1; 65]);
        
        let masternodes: Vec<Address> = (0..18)
            .map(|i| Address::with_last_byte(i))
            .collect();
        
        let result = verify_qc(&qc, &masternodes, None);
        assert!(result.is_err());
        
        match result {
            Err(XDPoSError::InsufficientSignatures { have, need }) => {
                assert_eq!(have, 1);
                assert_eq!(need, 12);
            }
            _ => panic!("Expected InsufficientSignatures error"),
        }
    }

    #[test]
    fn test_verify_qc_round_zero() {
        // Round 0 (genesis/switch block) should pass without signatures
        let block_info = BlockInfo::new(B256::with_last_byte(1), 0, 0);
        let qc = QuorumCert::new(block_info, 0);
        
        let masternodes: Vec<Address> = (0..18)
            .map(|i| Address::with_last_byte(i))
            .collect();
        
        let result = verify_qc(&qc, &masternodes, None);
        assert!(result.is_ok());
    }

    #[test]
    fn test_verify_tc_insufficient_signatures() {
        let mut tc = TimeoutCert::new(200, 500);
        tc.add_signature(vec![1; 65]);
        
        let masternodes: Vec<Address> = (0..18)
            .map(|i| Address::with_last_byte(i))
            .collect();
        
        let result = verify_tc(&tc, &masternodes, None);
        assert!(result.is_err());
    }

    #[test]
    fn test_verify_empty_masternode_list() {
        let block_info = BlockInfo::new(B256::with_last_byte(1), 100, 1000);
        let qc = QuorumCert::new(block_info, 500);
        
        let result = verify_qc(&qc, &[], None);
        assert!(result.is_err());
    }
}
