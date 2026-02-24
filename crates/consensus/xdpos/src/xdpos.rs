//! XDPoS Consensus Engine
//!
//! The main consensus engine implementation that supports both:
//! - XDPoS V1: Epoch-based consensus with checkpoint rewards
//! - XDPoS V2: BFT consensus with Quorum Certificates

use crate::{
    config::XDPoSConfig,
    constants::{EXTRA_SEAL, EXTRA_VANITY, INMEMORY_SIGNATURES, INMEMORY_SNAPSHOTS},
    errors::{XDPoSError, XDPoSResult},
    snapshot::Snapshot,
    v1,
    v2::XDPoSV2Engine,
};
use alloc::{boxed::Box, fmt::Debug, string::String, sync::Arc, vec::Vec};
use alloy_consensus::Header;
use alloy_primitives::{Address, B256};
use lru::LruCache;
use parking_lot::Mutex;
use reth_consensus::{Consensus, ConsensusError, FullConsensus, HeaderValidator, ReceiptRootBloom};
use reth_execution_types::BlockExecutionResult;
use reth_primitives_traits::{
    Block, NodePrimitives, RecoveredBlock, SealedBlock, SealedHeader,
};

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
}

impl XDPoSConsensus {
    /// Create a new XDPoS consensus engine
    pub fn new(config: XDPoSConfig) -> Arc<Self> {
        let v2_engine = config.v2.as_ref().map(|_| XDPoSV2Engine::new(config.clone()));

        Arc::new(Self {
            config,
            v2_engine,
            recents: Mutex::new(LruCache::new(INMEMORY_SNAPSHOTS)),
            signatures: Mutex::new(LruCache::new(INMEMORY_SIGNATURES)),
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
    fn seal_hash(
        &self,
        header: &Header,
    ) -> B256 {
        // Hash the header excluding the signature portion of extra data
        // TODO: Implement proper seal hash calculation
        header.hash_slow()
    }

    /// Recover address from signature
    fn ecrecover(
        &self,
        _hash: B256,
        _signature: &[u8],
    ) -> XDPoSResult<Address> {
        // TODO: Implement proper ECDSA recovery
        // Use secp256k1 to recover the public key, then derive address
        Ok(Address::ZERO)
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
        _block: &SealedBlock<impl Block>,
    ) -> Result<(), ConsensusError> {
        // TODO: Implement reward distribution
        // Only apply at checkpoint blocks (number % epoch == 0)
        Ok(())
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

impl<B: Block> Consensus<B> for XDPoSConsensus {
    fn validate_body_against_header(
        &self,
        body: &B::Body,
        header: &SealedHeader<B::Header>,
    ) -> Result<(), ConsensusError> {
        // XDPoS doesn't allow uncles
        // TODO: Verify body matches header
        let _ = body;
        let _ = header;
        Ok(())
    }

    fn validate_block_pre_execution(
        &self,
        block: &SealedBlock<B>,
    ) -> Result<(), ConsensusError> {
        let number = block.number();

        if self.is_v2_block(number) {
            // V2 validation
            let v2_engine = self
                .v2_engine()
                .ok_or_else(|| {
                    ConsensusError::Custom(
                        XDPoSError::V2EngineNotInitialized.to_string().into(),
                    )
                })?;

            // Decode V2 extra fields
            let _extra_fields = v2_engine
                .decode_extra_fields(block.extra_data())
                .map_err(|e| ConsensusError::Custom(e.to_string().into()))?;

            // TODO: Full V2 validation

            Ok(())
        } else {
            // V1 validation
            v1::validate_v1_header(
                block.header(),
                &self.config,
                None, // parent
            )
            .map_err(|e| ConsensusError::Custom(e.to_string().into()))
        }
    }
}

impl<H> HeaderValidator<H> for XDPoSConsensus
where
    H: alloy_consensus::BlockHeader,
{
    fn validate_header(
        &self,
        header: &SealedHeader<H>,
    ) -> Result<(), ConsensusError> {
        // Basic header validation
        let _number = header.number();

        // TODO: Implement header validation
        // - Check extra data length
        // - Verify timestamp
        // - Verify difficulty

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

impl<N: NodePrimitives> FullConsensus<N> for XDPoSConsensus {
    fn validate_block_post_execution(
        &self,
        block: &RecoveredBlock<N::Block>,
        _result: &BlockExecutionResult<N::Receipt>,
        _receipt_root_bloom: Option<ReceiptRootBloom>,
    ) -> Result<(), ConsensusError> {
        // Apply rewards at checkpoint blocks
        self.apply_rewards(block.block())
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
