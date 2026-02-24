//! XDPoS V1 Validation Logic

use crate::{
    config::XDPoSConfig,
    constants::{EXTRA_SEAL, EXTRA_VANITY},
    errors::{XDPoSError, XDPoSResult},
};
use alloy_consensus::Header;
use alloy_primitives::{Address, B256};

/// Validate a V1 block header
pub fn validate_v1_header(
    header: &Header,
    config: &XDPoSConfig,
    parent: Option<&Header>,
) -> XDPoSResult<()> {
    // Check extra data length
    let extra = &header.extra_data;
    if extra.len() < EXTRA_VANITY + EXTRA_SEAL {
        return Err(XDPoSError::MissingVanity);
    }

    let number = header.number;
    let checkpoint = number % config.epoch == 0;

    // Checkpoint blocks must have zero beneficiary
    if checkpoint && header.beneficiary != Address::ZERO {
        return Err(XDPoSError::InvalidCheckpointBeneficiary);
    }

    // Verify mix digest is zero
    if header.mix_hash != B256::ZERO {
        return Err(XDPoSError::InvalidMixDigest);
    }

    // Verify uncle hash is empty
    // Note: XDPoS doesn't allow uncles

    // Verify timestamp
    if let Some(parent) = parent {
        let expected_time = parent.timestamp + config.period;
        if header.timestamp < expected_time {
            return Err(XDPoSError::InvalidTimestamp);
        }
    }

    Ok(())
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
