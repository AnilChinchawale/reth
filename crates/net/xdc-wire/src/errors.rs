//! XDC wire protocol error types.

use crate::version::ParseVersionError;
use alloy_primitives::B256;
use reth_eth_wire::errors::EthStreamError;

/// Top-level XDC wire protocol error
#[derive(Debug, thiserror::Error)]
pub enum XdcWireError {
    /// Handshake error
    #[error(transparent)]
    Handshake(#[from] XdcHandshakeError),

    /// Stream error
    #[error(transparent)]
    Stream(#[from] XdcStreamError),

    /// Version parsing error
    #[error(transparent)]
    Version(#[from] ParseVersionError),

    /// RLP error
    #[error("RLP error: {0}")]
    Rlp(#[from] alloy_rlp::Error),

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// XDC handshake errors
#[derive(Debug, thiserror::Error)]
pub enum XdcHandshakeError {
    /// Network ID mismatch
    #[error("Network ID mismatch: expected {expected}, received {received}")]
    NetworkIdMismatch {
        /// Expected network ID
        expected: u64,
        /// Received network ID
        received: u64,
    },

    /// Genesis hash mismatch
    #[error("Genesis hash mismatch: expected {expected}, received {received}")]
    GenesisMismatch {
        /// Expected genesis hash
        expected: B256,
        /// Received genesis hash
        received: B256,
    },

    /// Protocol version mismatch
    #[error("Protocol version mismatch: expected one of {expected:?}, received {received}")]
    ProtocolVersionMismatch {
        /// Expected versions
        expected: Vec<u32>,
        /// Received version
        received: u32,
    },

    /// Unsupported protocol version
    #[error("Unsupported protocol version: {0}")]
    UnsupportedVersion(u32),

    /// Unexpected message during handshake
    #[error("Unexpected message during handshake: expected Status")]
    UnexpectedMessage,

    /// Connection closed during handshake
    #[error("Connection closed during handshake")]
    ConnectionClosed,

    /// Handshake timeout
    #[error("Handshake timeout")]
    Timeout,

    /// RLP decoding error
    #[error("RLP error: {0}")]
    Rlp(#[from] alloy_rlp::Error),

    /// Stream error
    #[error("Stream error: {0}")]
    Stream(#[from] XdcStreamError),

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// XDC stream errors
#[derive(Debug, thiserror::Error)]
pub enum XdcStreamError {
    /// Message too large
    #[error("Message too large: {size} bytes (max {max})")]
    MessageTooLarge {
        /// Actual size
        size: usize,
        /// Maximum allowed size
        max: usize,
    },

    /// Invalid message for protocol version
    #[error("Invalid message for protocol version {version}: {message}")]
    InvalidMessageForVersion {
        /// Protocol version
        version: crate::version::XdcVersion,
        /// Message type
        message: String,
    },

    /// Invalid message ID
    #[error("Invalid message ID: {0:#x}")]
    InvalidMessageId(u8),

    /// RLP error
    #[error("RLP error: {0}")]
    Rlp(#[from] alloy_rlp::Error),

    /// Connection closed
    #[error("Connection closed")]
    ConnectionClosed,

    /// Stream timeout
    #[error("Stream timeout")]
    Timeout,

    /// Request timeout (eth/63 FIFO matching)
    #[error("Request timeout: no response received")]
    RequestTimeout,

    /// Unexpected response (eth/63 FIFO matching)
    #[error("Unexpected response: got {received}, expected {expected}")]
    UnexpectedResponse {
        /// Expected response type
        expected: String,
        /// Received response type
        received: String,
    },

    /// Too many pending requests
    #[error("Too many pending requests: {count} (max {max})")]
    TooManyPendingRequests {
        /// Current count
        count: usize,
        /// Maximum allowed
        max: usize,
    },

    /// Underlying eth stream error
    #[error("Eth stream error: {0}")]
    EthStream(#[from] EthStreamError),

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
