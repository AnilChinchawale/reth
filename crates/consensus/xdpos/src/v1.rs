//! XDPoS V1 Validation Logic
//!
//! Implements V1 consensus rules including:
//! - ECDSA seal verification
//! - Checkpoint validation
//! - Difficulty and timing rules

use crate::{
    config::XDPoSConfig,
    constants::{DIFF_IN_TURN, DIFF_NO_TURN, EXTRA_SEAL, EXTRA_VANITY},
    errors::{XDPoSError, XDPoSResult},
    extra_data::{recover_signer, V1ExtraData},
    snapshot::Snapshot,
};
use alloy_consensus::Header;
use alloy_primitives::{Address, B256};

/// Validate a V1 block header with full ECDSA seal verification
pub fn validate_v1_header(
    header: &Header,
    config: &XDPoSConfig,
    parent: Option<&Header>,
    snapshot: Option<&Snapshot>,
) -> XDPoSResult<Address> {
    // Check extra data length
    let extra = &header.extra_data;
    if extra.len() < EXTRA_VANITY + EXTRA_SEAL {
        return Err(XDPoSError::MissingVanity);
    }

    let number = header.number;
    let checkpoint = number % config.epoch == 0;

    // Parse and validate extra data structure
    let _extra_data = V1ExtraData::parse(extra, checkpoint)?;

    // Checkpoint blocks must have zero beneficiary
    if checkpoint && header.beneficiary != Address::ZERO {
        return Err(XDPoSError::InvalidCheckpointBeneficiary);
    }

    // Verify mix digest is zero
    if header.mix_hash != B256::ZERO {
        return Err(XDPoSError::InvalidMixDigest);
    }

    // Verify uncle hash is empty (XDPoS doesn't allow uncles)
    // Note: Could add explicit check here if needed

    // Verify timestamp
    if let Some(parent) = parent {
        let expected_time = parent.timestamp + config.period;
        if header.timestamp < expected_time {
            return Err(XDPoSError::InvalidTimestamp);
        }
    }

    // Recover signer from seal
    let signer = recover_signer(header)?;

    // Verify signer is authorized (if we have a snapshot)
    if let Some(snapshot) = snapshot {
        if !snapshot.is_signer(&signer) {
            return Err(XDPoSError::Unauthorized);
        }

        // Check if signer has signed recently (anti-spam)
        if snapshot.recently_signed(number, &signer) {
            return Err(XDPoSError::Unauthorized);
        }

        // Verify difficulty matches in-turn status
        let expected_difficulty = if snapshot.inturn(number, &signer) {
            DIFF_IN_TURN
        } else {
            DIFF_NO_TURN
        };

        if header.difficulty.to::<u64>() != expected_difficulty {
            return Err(XDPoSError::InvalidDifficulty);
        }
    }

    Ok(signer)
}

/// Verify the seal of a header and return the signer
///
/// This is a lightweight version that only checks the cryptographic signature
/// without validating authorization or other consensus rules.
pub fn verify_seal(header: &Header) -> XDPoSResult<Address> {
    recover_signer(header)
}

/// Extract signers from checkpoint header extra data
pub fn extract_checkpoint_signers(extra: &[u8]) -> XDPoSResult<Vec<Address>> {
    if extra.len() < EXTRA_VANITY + EXTRA_SEAL {
        return Err(XDPoSError::MissingVanity);
    }

    // Signers are stored between vanity and seal
    let signers_data = &extra[EXTRA_VANITY..extra.len() - EXTRA_SEAL];

    if signers_data.len() % 20 != 0 {
        return Err(XDPoSError::InvalidCheckpointSigners);
    }

    let num_signers = signers_data.len() / 20;
    let mut signers = Vec::with_capacity(num_signers);

    for i in 0..num_signers {
        let start = i * 20;
        let addr = Address::from_slice(&signers_data[start..start + 20]);
        signers.push(addr);
    }

    Ok(signers)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_checkpoint_signers() {
        // Create extra data with 2 signers
        let vanity = vec![0u8; EXTRA_VANITY];
        let signer1 = Address::with_last_byte(1);
        let signer2 = Address::with_last_byte(2);
        let seal = vec![0u8; EXTRA_SEAL];

        let mut extra = vanity;
        extra.extend_from_slice(signer1.as_slice());
        extra.extend_from_slice(signer2.as_slice());
        extra.extend_from_slice(&seal);

        let signers = extract_checkpoint_signers(&extra).unwrap();
        assert_eq!(signers.len(), 2);
        assert_eq!(signers[0], signer1);
        assert_eq!(signers[1], signer2);
    }
}
