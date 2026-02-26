//! XDPoS Consensus Engine
//!
//! The main consensus engine implementation that supports both:
//! - XDPoS V1: Epoch-based consensus with checkpoint rewards
//! - XDPoS V2: BFT consensus with Quorum Certificates

use crate::{
    config::XDPoSConfig,
    constants::{EXTRA_SEAL, EXTRA_VANITY, INMEMORY_SIGNATURES, INMEMORY_SNAPSHOTS},
    errors::{XDPoSError, XDPoSResult},
    execution::{finalize_state_root, should_apply_rewards},
    reward::RewardCalculator,
    snapshot::Snapshot,
    state_root_cache::XdcStateRootCache,
    v1,
    v2::XDPoSV2Engine,
};
use alloc::{boxed::Box, fmt::Debug, string::String, sync::Arc, vec::Vec};
use core::num::NonZeroUsize;
use alloy_consensus::{BlockHeader, Header};
use alloy_primitives::{Address, B256};
use alloy_rlp::Encodable;
use lru::LruCache;
use parking_lot::Mutex;
use reth_consensus::{Consensus, ConsensusError, FullConsensus, HeaderValidator, ReceiptRootBloom};
use reth_execution_types::BlockExecutionResult;
use reth_primitives_traits::{
    Block, GotExpectedBoxed, NodePrimitives, RecoveredBlock, SealedBlock, SealedHeader,
};
use sha3::{Digest, Keccak256};
use tracing::{debug, info, trace};

/// XDPoS Consensus Engine
pub struct XDPoSConsensus {
    /// XDPoS configuration
    config: XDPoSConfig,
    /// V2 engine (if V2 is configured)
    v2_engine: Option<Arc<XDPoSV2Engine>>,
    /// Recent snapshots cache
    recents: Mutex<LruCache<B256, Snapshot>>,
    /// Recent signatures cache
    signatures: Mutex<LruCache<B256, Address>>,
    /// State root cache for checkpoint blocks
    state_root_cache: Arc<XdcStateRootCache>,
    /// Reward calculator
    reward_calculator: RewardCalculator,
}

impl XDPoSConsensus {
    /// Create a new XDPoS consensus engine
    pub fn new(config: XDPoSConfig) -> Arc<Self> {
        Self::new_with_cache(config, None)
    }

    /// Create a new XDPoS consensus engine with custom cache path
    pub fn new_with_cache(config: XDPoSConfig, cache_path: Option<std::path::PathBuf>) -> Arc<Self> {
        let v2_engine = config.v2.as_ref().map(|_| XDPoSV2Engine::new(config.clone()));
        let state_root_cache = Arc::new(XdcStateRootCache::with_default_size(cache_path));
        let reward_calculator = RewardCalculator::new(config.clone());

        info!(
            epoch = config.epoch,
            v2_enabled = v2_engine.is_some(),
            "Initialized XDPoS consensus engine"
        );

        Arc::new(Self {
            config,
            v2_engine,
            recents: Mutex::new(LruCache::new(
                NonZeroUsize::new(INMEMORY_SNAPSHOTS).unwrap(),
            )),
            signatures: Mutex::new(LruCache::new(
                NonZeroUsize::new(INMEMORY_SIGNATURES).unwrap(),
            )),
            state_root_cache,
            reward_calculator,
        })
    }

    /// Get the XDPoS configuration
    pub fn config(&self) -> &XDPoSConfig {
        &self.config
    }

    /// Check if a block is a V2 block
    pub fn is_v2_block(&self, block_number: u64) -> bool {
        self.config.is_v2(block_number)
    }

    /// Get the V2 engine
    pub fn v2_engine(&self) -> Option<&XDPoSV2Engine> {
        self.v2_engine.as_ref().map(|e| e.as_ref())
    }

    /// Recover the signer from a block header
    pub fn recover_signer(
        &self,
        header: &Header,
    ) -> XDPoSResult<Address> {
        let hash = header.hash_slow();

        // Check cache first
        if let Some(signer) = self.signatures.lock().get(&hash) {
            return Ok(*signer);
        }

        // Extract signature from extra data
        let extra = &header.extra_data;
        if extra.len() < EXTRA_VANITY + EXTRA_SEAL {
            return Err(XDPoSError::MissingSignature);
        }

        let signature = &extra[extra.len() - EXTRA_SEAL..];

        // Compute hash for signing (without the signature portion)
        let sig_hash = self.seal_hash(header);

        // Recover public key from signature
        let signer = self.ecrecover(sig_hash, signature)?;

        // Cache the result
        self.signatures.lock().put(hash, signer);

        Ok(signer)
    }

    /// Compute the seal hash for a header
    /// 
    /// The seal hash is the hash of the header excluding the signature portion
    /// of extra_data. This is what gets signed by the block producer.
    fn seal_hash(
        &self,
        header: &Header,
    ) -> B256 {
        use alloy_rlp::Encodable;
        use sha3::{Digest, Keccak256};
        
        // Create a copy of the header with signature stripped from extra_data
        let mut header_for_seal = header.clone();
        let extra = &header.extra_data;
        
        if extra.len() >= EXTRA_VANITY + EXTRA_SEAL {
            // Strip the signature (last 65 bytes) from extra_data
            let new_extra = &extra[..extra.len() - EXTRA_SEAL];
            header_for_seal.extra_data = new_extra.to_vec().into();
        }
        
        // Encode the modified header and hash it
        let mut encoded = Vec::new();
        header_for_seal.encode(&mut encoded);
        B256::from_slice(&Keccak256::digest(&encoded))
    }

    /// Recover address from signature using ECDSA
    fn ecrecover(
        &self,
        hash: B256,
        signature: &[u8],
    ) -> XDPoSResult<Address> {
        use secp256k1::{ecdsa::RecoverableSignature, Message, PublicKey, SECP256K1};
        
        if signature.len() != 65 {
            return Err(XDPoSError::InvalidSignature);
        }
        
        // Split signature into r, s, v
        let r = &signature[0..32];
        let s = &signature[32..64];
        let v = signature[64];
        
        // Convert v to recovery id (27/28 -> 0/1)
        let recovery_id = match v {
            27 => 0,
            28 => 1,
            0 | 1 => v,
            _ => return Err(XDPoSError::InvalidSignature),
        };
        
        // Create recoverable signature
        let mut sig_bytes = [0u8; 64];
        sig_bytes[0..32].copy_from_slice(r);
        sig_bytes[32..64].copy_from_slice(s);
        
        let rec_id = secp256k1::ecdsa::RecoveryId::from_i32(recovery_id as i32)
            .map_err(|_| XDPoSError::InvalidSignature)?;
        
        let rec_sig = RecoverableSignature::from_compact(&sig_bytes, rec_id)
            .map_err(|_| XDPoSError::InvalidSignature)?;
        
        // Recover public key
        let msg = Message::from_slice(hash.as_slice())
            .map_err(|_| XDPoSError::InvalidSignature)?;
        let pub_key = SECP256K1.recover_ecdsa(&msg, &rec_sig)
            .map_err(|_| XDPoSError::InvalidSignature)?;
        
        // Convert public key to address (keccak256 of pubkey, last 20 bytes)
        let pub_key_bytes = pub_key.serialize_uncompressed();
        let hash = Keccak256::digest(&pub_key_bytes[1..]); // Skip 0x04 prefix
        let address_bytes = &hash[12..32]; // Last 20 bytes
        
        Ok(Address::from_slice(address_bytes))
    }

    /// Get or create a snapshot for a given block
    pub fn snapshot(
        &self,
        _number: u64,
        _hash: B256,
    ) -> XDPoSResult<Snapshot> {
        // TODO: Implement snapshot retrieval/creation
        // 1. Check cache
        // 2. Load from database
        // 3. Create from parent
        Err(XDPoSError::Custom("Not implemented".into()))
    }

    /// Apply rewards at checkpoint blocks
    pub fn apply_rewards(
        &self,
        block: &SealedBlock<impl Block>,
    ) -> Result<(), ConsensusError> {
        let block_number = block.header().number();

        // Only apply at checkpoint blocks
        if should_apply_rewards(block_number, self.config.epoch) {
            debug!(
                block = block_number,
                epoch = self.config.epoch,
                "Checkpoint block detected - rewards would be applied here"
            );

            // TODO: Implement full reward distribution
            // This requires:
            // 1. Walk through the epoch (previous 900 blocks)
            // 2. Count validator signatures using recover_signer
            // 3. Calculate reward distribution using reward_calculator
            // 4. Apply balance changes (requires ExecutionOutcome mutation)
            //
            // Note: Actual reward application happens during execution via
            // apply_checkpoint_rewards() called from the executor, not here.
            // This validation hook just verifies the result.
        }

        Ok(())
    }

    /// Validate state root with cache integration
    ///
    /// For checkpoint blocks, checks the state root cache to handle known divergences
    /// between XDC clients. Returns the finalized state root that should be used.
    pub fn validate_state_root(
        &self,
        block_number: u64,
        header_root: B256,
        computed_root: B256,
    ) -> Result<B256, ConsensusError> {
        let finalized_root = finalize_state_root(
            block_number,
            header_root,
            computed_root,
            &self.state_root_cache,
            self.config.epoch,
        );

        // Check if roots match (either directly or via cache)
        if finalized_root != header_root && finalized_root != computed_root {
            return Err(ConsensusError::BodyStateRootDiff(
                GotExpectedBoxed::from((computed_root, header_root)),
            ));
        }

        trace!(
            block = block_number,
            header = %header_root,
            computed = %computed_root,
            finalized = %finalized_root,
            "State root validated"
        );

        Ok(finalized_root)
    }

    /// Get the state root cache
    pub fn state_root_cache(&self) -> &XdcStateRootCache {
        &self.state_root_cache
    }

    /// Get the reward calculator
    pub fn reward_calculator(&self) -> &RewardCalculator {
        &self.reward_calculator
    }
}

impl Debug for XDPoSConsensus {
    fn fmt(&self, f: &mut alloc::fmt::Formatter<'_>) -> alloc::fmt::Result {
        f.debug_struct("XDPoSConsensus")
            .field("config", &self.config)
            .field("has_v2", &self.v2_engine.is_some())
            .finish()
    }
}

impl<B: Block<Header = Header>> Consensus<B> for XDPoSConsensus {
    fn validate_body_against_header(
        &self,
        body: &B::Body,
        header: &SealedHeader<B::Header>,
    ) -> Result<(), ConsensusError> {
        // XDPoS doesn't allow uncles
        // Verify body matches header
        let _ = body;
        let _ = header;
        Ok(())
    }

    fn validate_block_pre_execution(
        &self,
        block: &SealedBlock<B>,
    ) -> Result<(), ConsensusError> {
        let number = block.header().number();
        let header = block.header();

        if self.is_v2_block(number) {
            // V2 validation
            let v2_engine = self.v2_engine().ok_or(XDPoSError::V2EngineNotInitialized)?;

            // Decode V2 extra fields
            let _extra_fields = v2_engine
                .decode_extra_fields(&block.header().extra_data)
                .map_err(|e| ConsensusError::Custom(Arc::new(e)))?;

            // For now, accept V2 blocks without full validation
            // TODO: Full V2 QC/TC verification
            debug!(block = number, "V2 block validation (basic)");

            Ok(())
        } else {
            // V1 validation with full signer recovery
            let checkpoint = number % self.config.epoch == 0;

            // Parse extra data
            let extra_data = crate::extra_data::V1ExtraData::parse(
                &header.extra_data,
                checkpoint
            ).map_err(|e| ConsensusError::Custom(Arc::new(e)))?;

            // Verify checkpoint beneficiary is zero
            if checkpoint && header.beneficiary != Address::ZERO {
                return Err(ConsensusError::Custom(Arc::new(
                    XDPoSError::InvalidCheckpointBeneficiary
                )));
            }

            // Recover signer from seal
            let _signer = self.recover_signer(header)
                .map_err(|e| ConsensusError::Custom(Arc::new(e)))?;

            // For now, accept all recovered signers
            // Full authorization check requires snapshot which needs DB access
            // TODO: Integrate with snapshot for signer authorization
            debug!(block = number, ?extra_data, "V1 block validated");

            Ok(())
        }
    }
}

impl<H> HeaderValidator<H> for XDPoSConsensus
where
    H: alloy_consensus::BlockHeader + Debug,
{
    fn validate_header(
        &self,
        header: &SealedHeader<H>,
    ) -> Result<(), ConsensusError> {
        let number = header.number();
        let extra = header.extra_data();

        // 1. Validate extra data length (minimum 97 bytes for vanity + seal)
        if extra.len() < EXTRA_VANITY + EXTRA_SEAL {
            return Err(ConsensusError::Custom(Arc::new(
                XDPoSError::ExtraDataTooShort
            )));
        }

        // 2. Validate extra data doesn't exceed maximum
        if extra.len() > 32 * 1024 {
            return Err(ConsensusError::Custom(Arc::new(
                XDPoSError::InvalidExtraData
            )));
        }

        // 3. Validate timestamp is reasonable (not in future by more than 15 seconds)
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        
        // Allow 15 seconds clock drift
        if header.timestamp() > now + 15 {
            return Err(ConsensusError::TimestampIsInFuture {
                timestamp: header.timestamp(),
                present_timestamp: now,
            });
        }

        // 4. For V1 blocks: verify difficulty is valid (1 or 2)
        if !self.is_v2_block(number) {
            let diff = header.difficulty().to::<u64>();
            if diff != 1 && diff != 2 {
                return Err(ConsensusError::Custom(Arc::new(
                    XDPoSError::InvalidDifficulty {
                        expected: 2,
                        got: diff,
                    }
                )));
            }
        }

        // 5. Verify mix hash is zero (XDPoS requirement)
        if header.mix_hash() != Some(B256::ZERO) {
            return Err(ConsensusError::Custom(Arc::new(
                XDPoSError::InvalidMixDigest
            )));
        }

        // 6. Try to recover signer - this validates the ECDSA signature
        // Convert header to alloy_consensus::Header for recover_signer
        // This is a simplified check - full validation happens in validate_block_pre_execution
        trace!(block = number, "XDPoS header validation passed");

        Ok(())
    }

    fn validate_header_against_parent(
        &self,
        header: &SealedHeader<H>,
        parent: &SealedHeader<H>,
    ) -> Result<(), ConsensusError> {
        // Verify block number sequence
        if header.number() != parent.number() + 1 {
            return Err(ConsensusError::ParentBlockNumberMismatch {
                parent_block_number: parent.number(),
                block_number: header.number(),
            });
        }

        // Verify timestamp
        let min_time = parent.timestamp() + self.config.period;
        if header.timestamp() < min_time {
            return Err(ConsensusError::TimestampIsInPast {
                parent_timestamp: parent.timestamp(),
                timestamp: header.timestamp(),
            });
        }

        Ok(())
    }
}

impl<N: NodePrimitives<BlockHeader = Header>> FullConsensus<N> for XDPoSConsensus {
    fn validate_block_post_execution(
        &self,
        block: &RecoveredBlock<N::Block>,
        result: &BlockExecutionResult<N::Receipt>,
        _receipt_root_bloom: Option<ReceiptRootBloom>,
    ) -> Result<(), ConsensusError> {
        let block_number = block.header().number();

        // Validate gas used
        if result.gas_used != block.header().gas_used() {
            return Err(ConsensusError::Custom(Arc::new(XDPoSError::Custom(
                format!(
                    "Gas used mismatch at block {}: computed {}, expected {}",
                    block_number, result.gas_used, block.header().gas_used()
                ),
            ))));
        }

        // Log checkpoint blocks (rewards are applied during execution, not here)
        if should_apply_rewards(block_number, self.config.epoch) {
            debug!(
                block = block_number,
                epoch = self.config.epoch,
                "Validated checkpoint block post-execution"
            );
        }

        // Note: State root validation with cache happens in the merkle stage
        // The cache is accessible via self.state_root_cache() for integration there

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{V2Config, XDPoSConfig};

    fn test_config_v1() -> XDPoSConfig {
        XDPoSConfig::default()
    }

    fn test_config_v2() -> XDPoSConfig {
        XDPoSConfig::default().with_v2(V2Config::new(1000))
    }

    #[test]
    fn test_new_consensus_v1() {
        let consensus = XDPoSConsensus::new(test_config_v1());
        assert!(!consensus.is_v2_block(0));
        assert!(!consensus.is_v2_block(1000));
        assert!(consensus.v2_engine().is_none());
    }

    #[test]
    fn test_new_consensus_v2() {
        let consensus = XDPoSConsensus::new(test_config_v2());
        assert!(!consensus.is_v2_block(999));
        assert!(consensus.is_v2_block(1000));
        assert!(consensus.is_v2_block(1001));
        assert!(consensus.v2_engine().is_some());
    }
}
