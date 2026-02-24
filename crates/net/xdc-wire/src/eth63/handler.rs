//! eth/63 protocol handler.
//!
//! Provides high-level message handling for the eth/63 protocol.

use crate::{
    errors::XdcStreamError,
    types::{GetBlockHeaders63, HashOrNumber, XdcMessage},
};
use alloy_primitives::B256;

/// eth/63 protocol handler
#[derive(Debug, Default)]
pub struct Eth63Handler;

impl Eth63Handler {
    /// Create a new eth/63 handler
    pub fn new() -> Self {
        Self
    }

    /// Create a GetBlockHeaders request
    pub fn get_block_headers(
        &self,
        start: u64,
        amount: u64,
        skip: u64,
        reverse: bool,
    ) -> XdcMessage {
        XdcMessage::GetBlockHeaders(GetBlockHeaders63 {
            origin: HashOrNumber::Number(start),
            amount,
            skip,
            reverse,
        })
    }

    /// Create a GetBlockHeaders request by hash
    pub fn get_block_headers_by_hash(
        &self,
        hash: B256,
        amount: u64,
        skip: u64,
        reverse: bool,
    ) -> XdcMessage {
        XdcMessage::GetBlockHeaders(GetBlockHeaders63 {
            origin: HashOrNumber::Hash(hash),
            amount,
            skip,
            reverse,
        })
    }

    /// Create a GetBlockBodies request
    pub fn get_block_bodies(&self, hashes: Vec<B256>) -> XdcMessage {
        XdcMessage::GetBlockBodies(crate::types::GetBlockBodies63 { hashes })
    }

    /// Create a GetNodeData request (eth/63 only)
    pub fn get_node_data(&self, hashes: Vec<B256>) -> XdcMessage {
        XdcMessage::GetNodeData(crate::types::GetNodeData63 { hashes })
    }

    /// Create a GetReceipts request
    pub fn get_receipts(&self, hashes: Vec<B256>) -> XdcMessage {
        XdcMessage::GetReceipts(crate::types::GetReceipts63 { hashes })
    }

    /// Validate an eth/63 message
    pub fn validate_message(&self, msg: &XdcMessage) -> Result<(), XdcStreamError> {
        // eth/63 doesn't support consensus messages
        match msg {
            XdcMessage::Vote(_) | XdcMessage::Timeout(_) | XdcMessage::SyncInfo(_) => {
                Err(XdcStreamError::InvalidMessageForVersion {
                    version: crate::version::XdcVersion::Eth63,
                    message: format!("{:?}", msg.message_id()),
                })
            }
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_get_block_headers() {
        let handler = Eth63Handler::new();
        let msg = handler.get_block_headers(100, 10, 0, false);

        assert!(matches!(msg, XdcMessage::GetBlockHeaders(_)));
        if let XdcMessage::GetBlockHeaders(req) = msg {
            assert_eq!(req.amount, 10);
            assert!(!req.reverse);
        }
    }

    #[test]
    fn test_create_get_block_bodies() {
        let handler = Eth63Handler::new();
        let hashes = vec![B256::random(), B256::random()];
        let msg = handler.get_block_bodies(hashes.clone());

        assert!(matches!(msg, XdcMessage::GetBlockBodies(_)));
        if let XdcMessage::GetBlockBodies(req) = msg {
            assert_eq!(req.hashes.len(), 2);
        }
    }

    #[test]
    fn test_validate_message() {
        let handler = Eth63Handler::new();

        // Valid eth/63 message
        let msg = handler.get_block_headers(100, 10, 0, false);
        assert!(handler.validate_message(&msg).is_ok());

        // Invalid: consensus message in eth/63
        let vote = XdcMessage::Vote(crate::types::VoteMessage {
            round: 1,
            block_hash: B256::ZERO,
            signature: Default::default(),
        });
        assert!(handler.validate_message(&vote).is_err());
    }
}
