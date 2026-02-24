//! XDPoS V2 Engine Implementation
//!
//! This module implements the V2 BFT consensus engine with:
//! - Quorum Certificate (QC) verification
//! - Timeout Certificate (TC) handling
//! - Epoch switch detection
//! - Block validation
//! - Round management

use super::{
    proposer::select_proposer,
    types::{decode_extra_fields_v2, encode_extra_fields_v2, vote_sig_hash, timeout_sig_hash},
    verification::{verify_qc, verify_tc},
    BlockInfo, EpochSwitchInfo, ExtraFieldsV2, QuorumCert, Round, TimeoutCert,
    TimeoutForSign, VoteForSign,
};
use crate::{
    config::XDPoSConfig,
    errors::{XDPoSError, XDPoSResult},
};
use alloc::sync::Arc;
use alloy_primitives::{Address, B256};
use parking_lot::RwLock;
use std::collections::HashMap;

/// Extra data layout constants
pub const EXTRA_VANITY: usize = 32;
pub const EXTRA_SEAL: usize = 65;
pub const MIN_EXTRA_LENGTH: usize = EXTRA_VANITY + 1 + EXTRA_SEAL; // vanity + version + seal

/// XDPoS V2 Engine
pub struct XDPoSV2Engine {
    config: XDPoSConfig,
    state: RwLock<V2State>,
}

/// Internal state for V2 engine
#[derive(Debug, Clone, Default)]
struct V2State {
    /// Current round
    current_round: Round,
    /// Highest known QC
    highest_quorum_cert: Option<QuorumCert>,
    /// Highest known TC
    highest_timeout_cert: Option<TimeoutCert>,
    /// Locked QC (for safety)
    lock_quorum_cert: Option<QuorumCert>,
    /// Highest committed block
    highest_commit_block: Option<BlockInfo>,
    /// Epoch switch info cache (block_hash -> epoch_info)
    epoch_info_cache: HashMap<B256, EpochSwitchInfo>,
}

impl XDPoSV2Engine {
    /// Create a new V2 engine
    pub fn new(config: XDPoSConfig) -> Arc<Self> {
        Arc::new(Self {
            config,
            state: RwLock::new(V2State::default()),
        })
    }

    /// Check if a block is a V2 block
    pub fn is_v2_block(&self, block_number: u64) -> bool {
        self.config.is_v2(block_number)
    }

    /// Check if a block is at an epoch boundary
    pub fn is_epoch_switch(&self, block_number: u64) -> bool {
        block_number % self.config.epoch == 0
    }

    /// Get the epoch number for a block
    pub fn get_epoch(&self, block_number: u64) -> u64 {
        block_number / self.config.epoch
    }

    /// Get current round
    pub fn current_round(&self) -> Round {
        self.state.read().current_round
    }

    /// Update current round
    pub fn set_current_round(&self, round: Round) {
        self.state.write().current_round = round;
    }

    /// Get highest quorum cert
    pub fn highest_qc(&self) -> Option<QuorumCert> {
        self.state.read().highest_quorum_cert.clone()
    }

    /// Update highest quorum cert
    pub fn set_highest_qc(&self, qc: QuorumCert) {
        let mut state = self.state.write();
        if let Some(ref current_qc) = state.highest_quorum_cert {
            if qc.proposed_block_info.round > current_qc.proposed_block_info.round {
                state.highest_quorum_cert = Some(qc);
            }
        } else {
            state.highest_quorum_cert = Some(qc);
        }
    }

    /// Decode extra fields from block header extra data
    ///
    /// V2 extra data format:
    /// [vanity (32 bytes)][version (1 byte >= 2)][RLP encoded ExtraFields_v2][seal (65 bytes)]
    pub fn decode_extra_fields(&self, extra: &[u8]) -> XDPoSResult<ExtraFieldsV2> {
        if extra.len() < MIN_EXTRA_LENGTH {
            return Err(XDPoSError::ExtraDataTooShort);
        }

        // Check version byte (at position EXTRA_VANITY)
        let version = extra[EXTRA_VANITY];
        if version < 2 {
            return Err(XDPoSError::InvalidExtraData);
        }

        // Extract payload (between vanity+version and seal)
        let payload_start = EXTRA_VANITY + 1;
        let payload_end = extra.len() - EXTRA_SEAL;
        
        if payload_start >= payload_end {
            return Err(XDPoSError::ExtraDataTooShort);
        }

        let payload = &extra[payload_start..payload_end];
        let (round, quorum_cert) = decode_extra_fields_v2(payload)
            .map_err(|e| XDPoSError::Custom(e))?;

        Ok(ExtraFieldsV2::new(round, quorum_cert))
    }

    /// Encode extra fields to block header extra data
    ///
    /// # Arguments
    /// * `vanity` - 32 bytes of vanity data
    /// * `round` - Consensus round number
    /// * `quorum_cert` - Optional QC (None for switch block)
    /// * `seal` - 65 bytes seal signature
    ///
    /// # Returns
    /// Complete extra data bytes
    pub fn encode_extra_fields(
        &self,
        vanity: &[u8; 32],
        round: Round,
        quorum_cert: Option<&QuorumCert>,
        seal: &[u8; 65],
    ) -> Vec<u8> {
        let fields = encode_extra_fields_v2(round, quorum_cert);
        
        let mut extra = Vec::with_capacity(EXTRA_VANITY + fields.len() + EXTRA_SEAL);
        extra.extend_from_slice(vanity);
        extra.extend_from_slice(&fields); // Already includes version byte
        extra.extend_from_slice(seal);
        
        extra
    }

    /// Extract the seal signature from extra data
    pub fn extract_seal(&self, extra: &[u8]) -> XDPoSResult<[u8; 65]> {
        if extra.len() < EXTRA_SEAL {
            return Err(XDPoSError::MissingSignature);
        }

        let seal_start = extra.len() - EXTRA_SEAL;
        let seal_bytes = &extra[seal_start..];
        
        let mut seal = [0u8; 65];
        seal.copy_from_slice(seal_bytes);
        Ok(seal)
    }

    /// Verify a Quorum Certificate
    ///
    /// # Arguments
    /// * `qc` - The quorum certificate to verify
    /// * `masternodes` - List of valid masternodes for the epoch
    ///
    /// # Returns
    /// Ok if QC is valid, error otherwise
    pub fn verify_qc(&self, qc: &QuorumCert, masternodes: &[Address]) -> XDPoSResult<()> {
        // Use custom threshold if configured
        let threshold = self.config.v2.as_ref()
            .and_then(|v2| {
                if v2.cert_threshold > 0 && v2.cert_threshold <= 100 {
                    Some(v2.cert_threshold as f64 / 100.0)
                } else {
                    None
                }
            });

        verify_qc(qc, masternodes, threshold)
    }

    /// Verify a Timeout Certificate
    pub fn verify_tc(&self, tc: &TimeoutCert, masternodes: &[Address]) -> XDPoSResult<()> {
        let threshold = self.config.v2.as_ref()
            .and_then(|v2| {
                if v2.cert_threshold > 0 && v2.cert_threshold <= 100 {
                    Some(v2.cert_threshold as f64 / 100.0)
                } else {
                    None
                }
            });

        verify_tc(tc, masternodes, threshold)
    }

    /// Verify block proposer is correct for the round
    ///
    /// # Arguments
    /// * `round` - The consensus round
    /// * `proposer` - The address that proposed the block
    /// * `validators` - List of validators for this epoch
    ///
    /// # Returns
    /// Ok if proposer is correct, error otherwise
    pub fn verify_proposer(
        &self,
        round: Round,
        proposer: &Address,
        validators: &[Address],
    ) -> XDPoSResult<()> {
        let expected_proposer = select_proposer(round, validators)?;
        
        if proposer != &expected_proposer {
            return Err(XDPoSError::Unauthorized);
        }

        Ok(())
    }

    /// Verify round monotonicity - rounds must strictly increase
    ///
    /// # Arguments
    /// * `current_round` - The round of the current block
    /// * `parent_round` - The round of the parent block
    ///
    /// # Returns
    /// Ok if round increases correctly, error otherwise
    pub fn verify_round_monotonicity(
        &self,
        current_round: Round,
        parent_round: Round,
    ) -> XDPoSResult<()> {
        if current_round <= parent_round {
            return Err(XDPoSError::Custom(format!(
                "round must increase: current={}, parent={}",
                current_round, parent_round
            )));
        }
        Ok(())
    }

    /// Verify the QC references the correct parent block
    ///
    /// # Arguments
    /// * `qc` - The quorum certificate in the current block
    /// * `parent_hash` - The hash of the parent block
    /// * `parent_number` - The number of the parent block
    /// * `parent_round` - The round of the parent block
    ///
    /// # Returns
    /// Ok if QC matches parent, error otherwise
    pub fn verify_qc_parent(
        &self,
        qc: &QuorumCert,
        parent_hash: &B256,
        parent_number: u64,
        parent_round: Round,
    ) -> XDPoSResult<()> {
        let qc_info = &qc.proposed_block_info;
        
        if qc_info.hash != *parent_hash {
            return Err(XDPoSError::BlockInfoMismatch);
        }
        
        if qc_info.number != parent_number {
            return Err(XDPoSError::BlockInfoMismatch);
        }
        
        if qc_info.round != parent_round {
            return Err(XDPoSError::BlockInfoMismatch);
        }

        Ok(())
    }

    /// Get epoch switch info for a block
    ///
    /// This retrieves the validator set and penalties for an epoch
    ///
    /// # Arguments
    /// * `block_hash` - The hash of the epoch switch block
    ///
    /// # Returns
    /// Epoch switch information
    pub fn get_epoch_switch_info(&self, block_hash: &B256) -> XDPoSResult<EpochSwitchInfo> {
        // Check cache first
        {
            let state = self.state.read();
            if let Some(info) = state.epoch_info_cache.get(block_hash) {
                return Ok(info.clone());
            }
        }

        // In production, this would fetch from the chain
        // For now, return error
        Err(XDPoSError::Custom("Epoch info not in cache - chain lookup needed".into()))
    }

    /// Cache epoch switch info
    pub fn cache_epoch_switch_info(&self, block_hash: B256, info: EpochSwitchInfo) {
        let mut state = self.state.write();
        state.epoch_info_cache.insert(block_hash, info);
    }

    /// Get signature hash for voting
    pub fn vote_sig_hash(vote: &VoteForSign) -> B256 {
        vote_sig_hash(vote)
    }

    /// Get signature hash for timeout
    pub fn timeout_sig_hash(timeout: &TimeoutForSign) -> B256 {
        timeout_sig_hash(timeout)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_config() -> XDPoSConfig {
        XDPoSConfig {
            epoch: 900,
            v2: Some(crate::config::V2Config {
                switch_block: 23556600,
                cert_threshold: 67,
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    #[test]
    fn test_engine_creation() {
        let config = default_config();
        let engine = XDPoSV2Engine::new(config);
        assert_eq!(engine.current_round(), 0);
    }

    #[test]
    fn test_is_v2_block() {
        let engine = XDPoSV2Engine::new(default_config());
        
        assert!(!engine.is_v2_block(1000));
        assert!(engine.is_v2_block(23556600));
        assert!(engine.is_v2_block(23556601));
    }

    #[test]
    fn test_is_epoch_switch() {
        let engine = XDPoSV2Engine::new(default_config());
        
        assert!(engine.is_epoch_switch(0));
        assert!(engine.is_epoch_switch(900));
        assert!(engine.is_epoch_switch(1800));
        assert!(!engine.is_epoch_switch(899));
        assert!(!engine.is_epoch_switch(901));
    }

    #[test]
    fn test_get_epoch() {
        let engine = XDPoSV2Engine::new(default_config());
        
        assert_eq!(engine.get_epoch(0), 0);
        assert_eq!(engine.get_epoch(899), 0);
        assert_eq!(engine.get_epoch(900), 1);
        assert_eq!(engine.get_epoch(1800), 2);
    }

    #[test]
    fn test_round_management() {
        let engine = XDPoSV2Engine::new(default_config());
        
        assert_eq!(engine.current_round(), 0);
        
        engine.set_current_round(100);
        assert_eq!(engine.current_round(), 100);
    }

    #[test]
    fn test_highest_qc_updates() {
        let engine = XDPoSV2Engine::new(default_config());
        
        let block_info1 = BlockInfo::new(B256::with_last_byte(1), 100, 1000);
        let qc1 = QuorumCert::new(block_info1, 500);
        
        engine.set_highest_qc(qc1.clone());
        assert!(engine.highest_qc().is_some());
        
        // Try to set lower round QC - should not update
        let block_info2 = BlockInfo::new(B256::with_last_byte(2), 50, 900);
        let qc2 = QuorumCert::new(block_info2, 500);
        
        engine.set_highest_qc(qc2);
        let highest = engine.highest_qc().unwrap();
        assert_eq!(highest.proposed_block_info.round, 100);
        
        // Set higher round QC - should update
        let block_info3 = BlockInfo::new(B256::with_last_byte(3), 200, 1100);
        let qc3 = QuorumCert::new(block_info3, 500);
        
        engine.set_highest_qc(qc3);
        let highest = engine.highest_qc().unwrap();
        assert_eq!(highest.proposed_block_info.round, 200);
    }

    #[test]
    fn test_decode_extra_fields_too_short() {
        let engine = XDPoSV2Engine::new(default_config());
        
        let short_extra = vec![0u8; 50];
        assert!(engine.decode_extra_fields(&short_extra).is_err());
    }

    #[test]
    fn test_decode_extra_fields_v1_block() {
        let engine = XDPoSV2Engine::new(default_config());
        
        // Create V1 extra data (version byte 1)
        let mut extra = vec![0u8; MIN_EXTRA_LENGTH];
        extra[EXTRA_VANITY] = 1; // V1 version
        
        assert!(engine.decode_extra_fields(&extra).is_err());
    }

    #[test]
    fn test_encode_decode_extra_fields() {
        let engine = XDPoSV2Engine::new(default_config());
        
        let vanity = [0u8; 32];
        let round = 100u64;
        let block_info = BlockInfo::new(B256::with_last_byte(1), 99, 1000);
        let qc = QuorumCert::new(block_info, 500);
        let seal = [0u8; 65];
        
        let extra = engine.encode_extra_fields(&vanity, round, Some(&qc), &seal);
        
        // Verify structure
        assert!(extra.len() >= MIN_EXTRA_LENGTH);
        assert_eq!(&extra[0..32], &vanity);
        assert_eq!(extra[EXTRA_VANITY], 2); // Version byte
        
        // Decode and verify
        let decoded = engine.decode_extra_fields(&extra).unwrap();
        assert_eq!(decoded.round, round);
        assert!(decoded.quorum_cert.is_some());
    }

    #[test]
    fn test_extract_seal() {
        let engine = XDPoSV2Engine::new(default_config());
        
        let mut extra = vec![0u8; MIN_EXTRA_LENGTH];
        // Fill seal with test pattern
        let len = extra.len();
        for i in 0..65 {
            extra[len - 65 + i] = i as u8;
        }
        
        let seal = engine.extract_seal(&extra).unwrap();
        assert_eq!(seal.len(), 65);
        assert_eq!(seal[0], 0);
        assert_eq!(seal[64], 64);
    }

    #[test]
    fn test_verify_proposer() {
        let engine = XDPoSV2Engine::new(default_config());
        
        let validators: Vec<Address> = (0..5)
            .map(|i| Address::with_last_byte(i))
            .collect();
        
        // Round 0 -> validator 0
        assert!(engine.verify_proposer(0, &validators[0], &validators).is_ok());
        assert!(engine.verify_proposer(0, &validators[1], &validators).is_err());
        
        // Round 3 -> validator 3
        assert!(engine.verify_proposer(3, &validators[3], &validators).is_ok());
        assert!(engine.verify_proposer(3, &validators[0], &validators).is_err());
    }

    #[test]
    fn test_verify_round_monotonicity() {
        let engine = XDPoSV2Engine::new(default_config());
        
        // Valid: current > parent
        assert!(engine.verify_round_monotonicity(100, 99).is_ok());
        assert!(engine.verify_round_monotonicity(200, 100).is_ok());
        
        // Invalid: current <= parent
        assert!(engine.verify_round_monotonicity(100, 100).is_err());
        assert!(engine.verify_round_monotonicity(100, 101).is_err());
    }

    #[test]
    fn test_verify_qc_parent() {
        let engine = XDPoSV2Engine::new(default_config());
        
        let parent_hash = B256::with_last_byte(1);
        let parent_number = 1000;
        let parent_round = 99;
        
        let block_info = BlockInfo::new(parent_hash, parent_round, parent_number);
        let qc = QuorumCert::new(block_info, 500);
        
        // Valid: QC matches parent
        assert!(engine.verify_qc_parent(&qc, &parent_hash, parent_number, parent_round).is_ok());
        
        // Invalid: wrong hash
        let wrong_hash = B256::with_last_byte(2);
        assert!(engine.verify_qc_parent(&qc, &wrong_hash, parent_number, parent_round).is_err());
        
        // Invalid: wrong number
        assert!(engine.verify_qc_parent(&qc, &parent_hash, 999, parent_round).is_err());
        
        // Invalid: wrong round
        assert!(engine.verify_qc_parent(&qc, &parent_hash, parent_number, 98).is_err());
    }

    #[test]
    fn test_epoch_switch_info_cache() {
        let engine = XDPoSV2Engine::new(default_config());
        
        let block_hash = B256::with_last_byte(1);
        let block_info = BlockInfo::new(block_hash, 0, 900);
        
        let epoch_info = EpochSwitchInfo {
            masternodes: vec![Address::with_last_byte(1)],
            masternodes_len: 1,
            epoch_switch_block_info: block_info.clone(),
            epoch_switch_parent_block_info: BlockInfo::new(B256::with_last_byte(0), 899, 899),
            penalties: vec![],
            standby_nodes: vec![],
        };
        
        // Cache it
        engine.cache_epoch_switch_info(block_hash, epoch_info.clone());
        
        // Retrieve from cache
        let retrieved = engine.get_epoch_switch_info(&block_hash).unwrap();
        assert_eq!(retrieved.masternodes_len, 1);
    }
}
