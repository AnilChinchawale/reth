//! XDPoS2 protocol handler.
//!
//! Provides high-level message handling for the XDPoS2 consensus protocol.

use crate::{
    errors::XdcStreamError,
    types::{SyncInfoMessage, TimeoutMessage, VoteMessage, XdcMessage},
};
use alloy_primitives::{Bytes, B256};

/// XDPoS2 consensus protocol handler
#[derive(Debug, Default)]
pub struct XdposHandler;

impl XdposHandler {
    /// Create a new XDPoS2 handler
    pub fn new() -> Self {
        Self
    }

    /// Create a Vote message
    pub fn create_vote(&self, round: u64, block_hash: B256, signature: Bytes) -> XdcMessage {
        XdcMessage::Vote(VoteMessage {
            round,
            block_hash,
            signature,
        })
    }

    /// Create a Timeout message
    pub fn create_timeout(&self, round: u64, signature: Bytes) -> XdcMessage {
        XdcMessage::Timeout(TimeoutMessage { round, signature })
    }

    /// Create a SyncInfo message
    pub fn create_sync_info(
        &self,
        highest_qc: Bytes,
        highest_tc: Bytes,
        latest_block_no: u64,
    ) -> XdcMessage {
        XdcMessage::SyncInfo(SyncInfoMessage {
            highest_qc,
            highest_tc,
            latest_block_no,
        })
    }

    /// Validate a vote message
    pub fn validate_vote(&self, vote: &VoteMessage, current_round: u64) -> Result<(), XdcStreamError> {
        // Check signature length (BLS signature should be 96 bytes)
        if vote.signature.len() != 96 {
            return Err(XdcStreamError::InvalidMessageForVersion {
                version: crate::version::XdcVersion::Eth100,
                message: format!("Invalid BLS signature length: {}", vote.signature.len()),
            });
        }

        // Check round (should be current or next round)
        if vote.round < current_round || vote.round > current_round + 1 {
            return Err(XdcStreamError::InvalidMessageForVersion {
                version: crate::version::XdcVersion::Eth100,
                message: format!("Invalid vote round: {}", vote.round),
            });
        }

        Ok(())
    }

    /// Validate a timeout message
    pub fn validate_timeout(&self, timeout: &TimeoutMessage, current_round: u64) -> Result<(), XdcStreamError> {
        // Check signature length
        if timeout.signature.len() != 96 {
            return Err(XdcStreamError::InvalidMessageForVersion {
                version: crate::version::XdcVersion::Eth100,
                message: format!("Invalid BLS signature length: {}", timeout.signature.len()),
            });
        }

        // Check round
        if timeout.round < current_round || timeout.round > current_round + 1 {
            return Err(XdcStreamError::InvalidMessageForVersion {
                version: crate::version::XdcVersion::Eth100,
                message: format!("Invalid timeout round: {}", timeout.round),
            });
        }

        Ok(())
    }

    /// Validate a sync info message
    pub fn validate_sync_info(&self, _sync_info: &SyncInfoMessage) -> Result<(), XdcStreamError> {
        // Basic validation - just check that fields exist
        // More detailed validation would require consensus state
        Ok(())
    }

    /// Validate an XDPoS2 message
    pub fn validate_message(&self, msg: &XdcMessage, current_round: u64) -> Result<(), XdcStreamError> {
        match msg {
            XdcMessage::Vote(vote) => self.validate_vote(vote, current_round),
            XdcMessage::Timeout(timeout) => self.validate_timeout(timeout, current_round),
            XdcMessage::SyncInfo(sync_info) => self.validate_sync_info(sync_info),
            // Other messages not supported in XDPoS2 handler
            _ => Err(XdcStreamError::InvalidMessageForVersion {
                version: crate::version::XdcVersion::Eth100,
                message: format!("{:?}", msg.message_id()),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_vote() {
        let handler = XdposHandler::new();
        let vote = handler.create_vote(
            100,
            B256::random(),
            Bytes::from(vec![0u8; 96]),
        );

        assert!(matches!(vote, XdcMessage::Vote(_)));
        if let XdcMessage::Vote(v) = vote {
            assert_eq!(v.round, 100);
            assert_eq!(v.signature.len(), 96);
        }
    }

    #[test]
    fn test_create_timeout() {
        let handler = XdposHandler::new();
        let timeout = handler.create_timeout(
            100,
            Bytes::from(vec![0u8; 96]),
        );

        assert!(matches!(timeout, XdcMessage::Timeout(_)));
        if let XdcMessage::Timeout(t) = timeout {
            assert_eq!(t.round, 100);
            assert_eq!(t.signature.len(), 96);
        }
    }

    #[test]
    fn test_validate_vote() {
        let handler = XdposHandler::new();
        
        let valid_vote = VoteMessage {
            round: 100,
            block_hash: B256::random(),
            signature: Bytes::from(vec![0u8; 96]),
        };

        assert!(handler.validate_vote(&valid_vote, 100).is_ok());
        assert!(handler.validate_vote(&valid_vote, 99).is_ok()); // Can be next round

        // Invalid: wrong signature length
        let invalid_vote = VoteMessage {
            round: 100,
            block_hash: B256::random(),
            signature: Bytes::from(vec![0u8; 32]),
        };
        assert!(handler.validate_vote(&invalid_vote, 100).is_err());

        // Invalid: round too far in future
        let future_vote = VoteMessage {
            round: 102,
            block_hash: B256::random(),
            signature: Bytes::from(vec![0u8; 96]),
        };
        assert!(handler.validate_vote(&future_vote, 100).is_err());
    }
}
