//! XDPoS Consensus Errors

use alloc::string::String;
use reth_consensus::ConsensusError;

/// XDPoS-specific error types
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum XDPoSError {
    /// Unknown block
    #[error("unknown block")]
    UnknownBlock,

    /// Unauthorized signer
    #[error("unauthorized signer")]
    Unauthorized,

    /// Invalid checkpoint beneficiary
    #[error("beneficiary in checkpoint block must be zero")]
    InvalidCheckpointBeneficiary,

    /// Invalid vote nonce
    #[error("invalid vote nonce")]
    InvalidVote,

    /// Invalid checkpoint vote
    #[error("vote nonce in checkpoint block must be zero")]
    InvalidCheckpointVote,

    /// Missing vanity in extra data
    #[error("extra-data 32 byte vanity prefix missing")]
    MissingVanity,

    /// Missing signature in extra data
    #[error("extra-data 65 byte suffix signature missing")]
    MissingSignature,

    /// Invalid checkpoint signers
    #[error("invalid signer list on checkpoint block")]
    InvalidCheckpointSigners,

    /// Non-zero mix digest
    #[error("non-zero mix digest")]
    InvalidMixDigest,

    /// Non-empty uncle hash
    #[error("non empty uncle hash")]
    InvalidUncleHash,

    /// Invalid difficulty
    #[error("invalid difficulty")]
    InvalidDifficulty,

    /// Invalid voting chain
    #[error("invalid voting chain")]
    InvalidVotingChain,

    /// Block in the future
    #[error("block in the future")]
    FutureBlock,

    /// Invalid timestamp
    #[error("invalid timestamp")]
    InvalidTimestamp,

    /// Unknown ancestor
    #[error("unknown ancestor")]
    UnknownAncestor,

    /// V2 consensus errors
    #[error("missing quorum certificate")]
    MissingQC,

    #[error("invalid quorum certificate")]
    InvalidQC,

    #[error("invalid QC signatures: {0}")]
    InvalidQCSignatures(String),

    #[error("missing timeout certificate")]
    MissingTC,

    #[error("invalid timeout certificate")]
    InvalidTC,

    #[error("invalid TC signatures")]
    InvalidTCSignatures,

    #[error("missing block info")]
    MissingBlockInfo,

    #[error("extra data too short")]
    ExtraDataTooShort,

    #[error("invalid extra data format")]
    InvalidExtraData,

    #[error("gap number mismatch")]
    GapNumberMismatch,

    #[error("block info mismatch")]
    BlockInfoMismatch,

    #[error("V2 engine not initialized")]
    V2EngineNotInitialized,

    #[error("signature verification failed")]
    SignatureVerificationFailed,

    #[error("invalid signature format")]
    InvalidSignatureFormat,

    #[error("creator not in masternode list")]
    CreatorNotMasternode,

    #[error("insufficient signatures: have {have}, need {need}")]
    InsufficientSignatures { have: usize, need: usize },

    /// Custom error message
    #[error("{0}")]
    Custom(String),
}

impl From<XDPoSError> for ConsensusError {
    fn from(err: XDPoSError) -> Self {
        ConsensusError::Custom(alloc::sync::Arc::new(err))
    }
}

impl From<XDPoSError> for reth_errors::RethError {
    fn from(err: XDPoSError) -> Self {
        reth_errors::RethError::Consensus(err.into())
    }
}

/// Result type for XDPoS operations
pub type XDPoSResult<T> = Result<T, XDPoSError>;
