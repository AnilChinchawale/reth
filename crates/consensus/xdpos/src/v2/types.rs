//! XDPoS V2 BFT Types with RLP Encoding/Decoding
//!
//! This module implements RLP encoding/decoding for XDPoS V2 consensus types
//! including Vote, Timeout, QuorumCert, and TimeoutCert.

use super::{BlockInfo, QuorumCert, Round, TimeoutCert, VoteForSign, TimeoutForSign};
use alloy_primitives::{keccak256, B256};
use alloy_rlp::{Decodable, RlpDecodable, RlpEncodable};

/// RLP-encodable/decodable BlockInfo
#[derive(Debug, Clone, PartialEq, Eq, RlpEncodable, RlpDecodable)]
pub struct BlockInfoRlp {
    pub hash: B256,
    pub round: u64,
    pub number: u64,
}

impl From<&BlockInfo> for BlockInfoRlp {
    fn from(info: &BlockInfo) -> Self {
        Self {
            hash: info.hash,
            round: info.round,
            number: info.number,
        }
    }
}

impl From<BlockInfoRlp> for BlockInfo {
    fn from(rlp: BlockInfoRlp) -> Self {
        Self {
            hash: rlp.hash,
            round: rlp.round,
            number: rlp.number,
        }
    }
}

/// RLP-encodable/decodable QuorumCert
#[derive(Debug, Clone, PartialEq, Eq, RlpEncodable, RlpDecodable)]
pub struct QuorumCertRlp {
    pub proposed_block_info: BlockInfoRlp,
    pub signatures: Vec<Vec<u8>>,
    pub gap_number: u64,
}

impl From<&QuorumCert> for QuorumCertRlp {
    fn from(qc: &QuorumCert) -> Self {
        Self {
            proposed_block_info: (&qc.proposed_block_info).into(),
            signatures: qc.signatures.clone(),
            gap_number: qc.gap_number,
        }
    }
}

impl From<QuorumCertRlp> for QuorumCert {
    fn from(rlp: QuorumCertRlp) -> Self {
        QuorumCert {
            proposed_block_info: rlp.proposed_block_info.into(),
            signatures: rlp.signatures,
            gap_number: rlp.gap_number,
        }
    }
}

/// RLP-encodable/decodable TimeoutCert
#[derive(Debug, Clone, PartialEq, Eq, RlpEncodable, RlpDecodable)]
pub struct TimeoutCertRlp {
    pub round: u64,
    pub signatures: Vec<Vec<u8>>,
    pub gap_number: u64,
}

impl From<&TimeoutCert> for TimeoutCertRlp {
    fn from(tc: &TimeoutCert) -> Self {
        Self {
            round: tc.round,
            signatures: tc.signatures.clone(),
            gap_number: tc.gap_number,
        }
    }
}

impl From<TimeoutCertRlp> for TimeoutCert {
    fn from(rlp: TimeoutCertRlp) -> Self {
        TimeoutCert {
            round: rlp.round,
            signatures: rlp.signatures,
            gap_number: rlp.gap_number,
        }
    }
}

/// RLP-encodable/decodable VoteForSign
#[derive(Debug, Clone, PartialEq, Eq, RlpEncodable, RlpDecodable)]
pub struct VoteForSignRlp {
    pub proposed_block_info: BlockInfoRlp,
    pub gap_number: u64,
}

impl From<&VoteForSign> for VoteForSignRlp {
    fn from(v: &VoteForSign) -> Self {
        Self {
            proposed_block_info: (&v.proposed_block_info).into(),
            gap_number: v.gap_number,
        }
    }
}

/// RLP-encodable/decodable TimeoutForSign
#[derive(Debug, Clone, PartialEq, Eq, RlpEncodable, RlpDecodable)]
pub struct TimeoutForSignRlp {
    pub round: u64,
    pub gap_number: u64,
}

impl From<&TimeoutForSign> for TimeoutForSignRlp {
    fn from(t: &TimeoutForSign) -> Self {
        Self {
            round: t.round,
            gap_number: t.gap_number,
        }
    }
}

/// Compute the signature hash for a Vote
pub fn vote_sig_hash(vote: &VoteForSign) -> B256 {
    let rlp_vote: VoteForSignRlp = vote.into();
    let encoded = alloy_rlp::encode(&rlp_vote);
    keccak256(&encoded)
}

/// Compute the signature hash for a Timeout
pub fn timeout_sig_hash(timeout: &TimeoutForSign) -> B256 {
    let rlp_timeout: TimeoutForSignRlp = timeout.into();
    let encoded = alloy_rlp::encode(&rlp_timeout);
    keccak256(&encoded)
}

/// ExtraFields_v2 RLP structure for block headers
#[derive(Debug, Clone, PartialEq, Eq, RlpEncodable, RlpDecodable)]
#[rlp(trailing)]
pub struct ExtraFieldsV2Rlp {
    pub round: u64,
    pub quorum_cert: Option<QuorumCertRlp>,
}

/// Encode ExtraFields_v2 to bytes with version prefix (2)
pub fn encode_extra_fields_v2(round: Round, quorum_cert: Option<&QuorumCert>) -> Vec<u8> {
    let qc_rlp = quorum_cert.map(|qc| qc.into());
    let fields = ExtraFieldsV2Rlp {
        round,
        quorum_cert: qc_rlp,
    };
    
    let encoded = alloy_rlp::encode(&fields);
    let mut result = Vec::with_capacity(1 + encoded.len());
    result.push(2); // Version byte
    result.extend_from_slice(&encoded);
    result
}

/// Decode ExtraFields_v2 from bytes (with version prefix)
pub fn decode_extra_fields_v2(bytes: &[u8]) -> Result<(Round, Option<QuorumCert>), String> {
    if bytes.is_empty() {
        return Err("extra field is empty".to_string());
    }
    
    let version = bytes[0];
    if version < 2 {
        return Err(format!("not a V2 block, version: {}", version));
    }
    
    let fields = ExtraFieldsV2Rlp::decode(&mut &bytes[1..])
        .map_err(|e| format!("failed to decode extra fields: {}", e))?;
    
    let qc = fields.quorum_cert.map(|qc_rlp| qc_rlp.into());
    Ok((fields.round, qc))
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::Address;

    #[test]
    fn test_block_info_rlp_roundtrip() {
        let info = BlockInfo::new(B256::with_last_byte(1), 100, 1000);
        let rlp: BlockInfoRlp = (&info).into();
        let encoded = alloy_rlp::encode(&rlp);
        let decoded = BlockInfoRlp::decode(&mut &encoded[..]).unwrap();
        assert_eq!(rlp, decoded);
    }

    #[test]
    fn test_quorum_cert_rlp_roundtrip() {
        let block_info = BlockInfo::new(B256::with_last_byte(1), 100, 1000);
        let mut qc = QuorumCert::new(block_info, 500);
        qc.add_signature(vec![1, 2, 3, 4, 5]);
        
        let rlp: QuorumCertRlp = (&qc).into();
        let encoded = alloy_rlp::encode(&rlp);
        let decoded = QuorumCertRlp::decode(&mut &encoded[..]).unwrap();
        assert_eq!(rlp, decoded);
    }

    #[test]
    fn test_timeout_cert_rlp_roundtrip() {
        let mut tc = TimeoutCert::new(200, 500);
        tc.add_signature(vec![1, 2, 3]);
        
        let rlp: TimeoutCertRlp = (&tc).into();
        let encoded = alloy_rlp::encode(&rlp);
        let decoded = TimeoutCertRlp::decode(&mut &encoded[..]).unwrap();
        assert_eq!(rlp, decoded);
    }

    #[test]
    fn test_vote_sig_hash() {
        let block_info = BlockInfo::new(B256::with_last_byte(42), 100, 1000);
        let vote_for_sign = VoteForSign {
            proposed_block_info: block_info,
            gap_number: 500,
        };
        
        let hash1 = vote_sig_hash(&vote_for_sign);
        let hash2 = vote_sig_hash(&vote_for_sign);
        assert_eq!(hash1, hash2); // Deterministic
        assert_ne!(hash1, B256::ZERO); // Not empty
    }

    #[test]
    fn test_timeout_sig_hash() {
        let timeout_for_sign = TimeoutForSign {
            round: 200,
            gap_number: 500,
        };
        
        let hash1 = timeout_sig_hash(&timeout_for_sign);
        let hash2 = timeout_sig_hash(&timeout_for_sign);
        assert_eq!(hash1, hash2);
        assert_ne!(hash1, B256::ZERO);
    }

    #[test]
    fn test_extra_fields_v2_encode_decode() {
        let block_info = BlockInfo::new(B256::with_last_byte(1), 100, 1000);
        let qc = QuorumCert::new(block_info, 500);
        
        // Test with QC
        let encoded = encode_extra_fields_v2(100, Some(&qc));
        assert_eq!(encoded[0], 2); // Version byte
        
        let (round, decoded_qc) = decode_extra_fields_v2(&encoded).unwrap();
        assert_eq!(round, 100);
        assert!(decoded_qc.is_some());
        
        // Test without QC (switch block)
        let encoded_no_qc = encode_extra_fields_v2(0, None);
        assert_eq!(encoded_no_qc[0], 2);
        
        let (round, decoded_qc) = decode_extra_fields_v2(&encoded_no_qc).unwrap();
        assert_eq!(round, 0);
        assert!(decoded_qc.is_none());
    }

    #[test]
    fn test_decode_invalid_version() {
        let invalid = vec![1, 0, 0, 0]; // Version 1
        assert!(decode_extra_fields_v2(&invalid).is_err());
    }

    #[test]
    fn test_decode_empty() {
        assert!(decode_extra_fields_v2(&[]).is_err());
    }
}
