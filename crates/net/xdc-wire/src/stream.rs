//! XDC stream implementation with protocol version handling.
//!
//! This module provides the [`XdcEthStream`] which handles message encoding/decoding
//! for different XDC protocol versions (eth/63, eth/66, eth/100).

use crate::{
    errors::{XdcHandshakeError, XdcStreamError},
    handshake::XdcHandshake,
    types::{Xdc63Status, XdcMessage},
    version::XdcVersion,
    MAX_MESSAGE_SIZE, MAX_PENDING_REQUESTS, REQUEST_TIMEOUT,
};
use alloy_primitives::B256;
use alloy_rlp::{Decodable, Encodable};
use bytes::{Bytes, BytesMut};
use futures::{ready, Sink, Stream};
use pin_project::pin_project;
use std::{
    collections::VecDeque,
    pin::Pin,
    task::{Context, Poll},
    time::Instant,
};
use tracing::{debug, trace};

/// Pending request for eth/63 FIFO matching
#[derive(Debug)]
struct PendingRequest {
    /// Request type
    request_type: RequestType,
    /// Timestamp when request was sent
    sent_at: Instant,
}

/// Request types for matching responses
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RequestType {
    /// GetBlockHeaders request
    BlockHeaders,
    /// GetBlockBodies request
    BlockBodies,
    /// GetNodeData request (eth/63 only)
    NodeData,
    /// GetReceipts request
    Receipts,
}

impl RequestType {
    /// Returns true if the response matches this request type
    fn matches_response(&self, msg: &XdcMessage) -> bool {
        matches!(
            (self, msg),
            (RequestType::BlockHeaders, XdcMessage::BlockHeaders(_))
                | (RequestType::BlockBodies, XdcMessage::BlockBodies(_))
                | (RequestType::NodeData, XdcMessage::NodeData(_))
                | (RequestType::Receipts, XdcMessage::Receipts(_))
        )
    }
}

/// Unauthenticated XDC stream (before handshake)
#[pin_project]
#[derive(Debug)]
pub struct UnauthedXdcStream<S> {
    #[pin]
    inner: S,
}

impl<S> UnauthedXdcStream<S> {
    /// Create a new unauthenticated stream
    pub const fn new(inner: S) -> Self {
        Self { inner }
    }

    /// Consume and return the inner stream
    pub fn into_inner(self) -> S {
        self.inner
    }
}

impl<S> UnauthedXdcStream<S>
where
    S: Stream<Item = Result<BytesMut, std::io::Error>>
        + Sink<Bytes, Error = std::io::Error>
        + Send
        + Unpin,
{
    /// Perform handshake and upgrade to authenticated stream
    pub async fn handshake(
        mut self,
        status: Xdc63Status,
        network_id: u64,
        genesis_hash: B256,
    ) -> Result<(XdcEthStream<S>, Xdc63Status), XdcHandshakeError> {
        let (peer_status, version) =
            XdcHandshake::execute(&mut self.inner, status, network_id, genesis_hash).await?;

        let stream = XdcEthStream::new(self.inner, version);

        Ok((stream, peer_status))
    }
}

/// Authenticated XDC Ethereum stream
#[pin_project]
#[derive(Debug)]
pub struct XdcEthStream<S> {
    #[pin]
    inner: S,
    /// Negotiated protocol version
    version: XdcVersion,
    /// Pending requests queue (for eth/63 FIFO matching)
    pending_requests: VecDeque<PendingRequest>,
}

impl<S> XdcEthStream<S> {
    /// Create a new XDC stream with negotiated version
    pub fn new(inner: S, version: XdcVersion) -> Self {
        Self {
            inner,
            version,
            pending_requests: VecDeque::new(),
        }
    }

    /// Get the negotiated protocol version
    pub const fn version(&self) -> XdcVersion {
        self.version
    }

    /// Get the number of pending requests
    pub fn pending_count(&self) -> usize {
        self.pending_requests.len()
    }

    /// Check if we can send more requests
    fn can_send_request(&self) -> bool {
        self.pending_requests.len() < MAX_PENDING_REQUESTS
    }

    /// Clean up expired requests
    fn cleanup_expired(&mut self) {
        let now = Instant::now();
        self.pending_requests
            .retain(|req| now.duration_since(req.sent_at) < REQUEST_TIMEOUT);
    }
}

impl<S> XdcEthStream<S>
where
    S: Sink<Bytes, Error = std::io::Error> + Unpin,
{
    /// Send a message
    pub async fn send_message(&mut self, msg: XdcMessage) -> Result<(), XdcStreamError> {
        // Validate message for protocol version
        self.validate_message(&msg)?;

        // Track request if applicable (for eth/63 FIFO matching)
        if msg.is_request() && self.version.is_legacy() {
            if !self.can_send_request() {
                return Err(XdcStreamError::TooManyPendingRequests {
                    count: self.pending_requests.len(),
                    max: MAX_PENDING_REQUESTS,
                });
            }

            let request_type = match &msg {
                XdcMessage::GetBlockHeaders(_) => RequestType::BlockHeaders,
                XdcMessage::GetBlockBodies(_) => RequestType::BlockBodies,
                XdcMessage::GetNodeData(_) => RequestType::NodeData,
                XdcMessage::GetReceipts(_) => RequestType::Receipts,
                _ => unreachable!(),
            };

            self.pending_requests.push_back(PendingRequest {
                request_type,
                sent_at: Instant::now(),
            });
        }

        // Encode and send
        let bytes = self.encode_message(&msg)?;
        
        use futures::SinkExt;
        self.inner.send(bytes).await?;

        trace!(
            ?msg,
            version = ?self.version,
            "Sent XDC message"
        );

        Ok(())
    }

    /// Encode a message to bytes
    fn encode_message(&self, msg: &XdcMessage) -> Result<Bytes, XdcStreamError> {
        let mut buf = Vec::new();

        // Message ID
        buf.push(msg.message_id() as u8);

        // Message payload
        match msg {
            XdcMessage::Status(s) => s.encode(&mut buf),
            XdcMessage::NewBlockHashes(m) => m.encode(&mut buf),
            XdcMessage::Transactions(m) => m.encode(&mut buf),
            XdcMessage::GetBlockHeaders(m) => m.encode(&mut buf),
            XdcMessage::BlockHeaders(m) => m.encode(&mut buf),
            XdcMessage::GetBlockBodies(m) => m.encode(&mut buf),
            XdcMessage::BlockBodies(m) => m.encode(&mut buf),
            XdcMessage::NewBlock(m) => m.encode(&mut buf),
            XdcMessage::GetNodeData(m) => m.encode(&mut buf),
            XdcMessage::NodeData(m) => m.encode(&mut buf),
            XdcMessage::GetReceipts(m) => m.encode(&mut buf),
            XdcMessage::Receipts(m) => m.encode(&mut buf),
            XdcMessage::Vote(m) => m.encode(&mut buf),
            XdcMessage::Timeout(m) => m.encode(&mut buf),
            XdcMessage::SyncInfo(m) => m.encode(&mut buf),
        }

        Ok(Bytes::from(buf))
    }

    /// Validate message for protocol version
    fn validate_message(&self, msg: &XdcMessage) -> Result<(), XdcStreamError> {
        match (self.version, msg) {
            // eth/63: No GetNodeData in eth/66+
            (XdcVersion::Eth66, XdcMessage::GetNodeData(_) | XdcMessage::NodeData(_)) => {
                Err(XdcStreamError::InvalidMessageForVersion {
                    version: self.version,
                    message: format!("{:?}", msg.message_id()),
                })
            }
            // XDPoS2 messages only in eth/100
            (
                XdcVersion::Eth62 | XdcVersion::Eth63 | XdcVersion::Eth66,
                XdcMessage::Vote(_) | XdcMessage::Timeout(_) | XdcMessage::SyncInfo(_),
            ) => Err(XdcStreamError::InvalidMessageForVersion {
                version: self.version,
                message: format!("{:?}", msg.message_id()),
            }),
            _ => Ok(()),
        }
    }
}

impl<S> XdcEthStream<S>
where
    S: Stream<Item = Result<BytesMut, std::io::Error>> + Unpin,
{
    /// Receive a message
    pub async fn receive_message(&mut self) -> Result<XdcMessage, XdcStreamError> {
        use futures::StreamExt;

        let bytes = self
            .inner
            .next()
            .await
            .ok_or(XdcStreamError::ConnectionClosed)??;

        // Check size
        if bytes.len() > MAX_MESSAGE_SIZE {
            return Err(XdcStreamError::MessageTooLarge {
                size: bytes.len(),
                max: MAX_MESSAGE_SIZE,
            });
        }

        // Decode message
        let msg = self.decode_message(&bytes)?;

        // Match response to pending request (for eth/63)
        if msg.is_response() && self.version.is_legacy() {
            self.cleanup_expired();

            let expected = self
                .pending_requests
                .pop_front()
                .ok_or(XdcStreamError::UnexpectedResponse {
                    expected: "no pending request".to_string(),
                    received: format!("{:?}", msg.message_id()),
                })?;

            if !expected.request_type.matches_response(&msg) {
                return Err(XdcStreamError::UnexpectedResponse {
                    expected: format!("{:?}", expected.request_type),
                    received: format!("{:?}", msg.message_id()),
                });
            }
        }

        trace!(
            ?msg,
            version = ?self.version,
            "Received XDC message"
        );

        Ok(msg)
    }

    /// Decode a message from bytes
    fn decode_message(&self, bytes: &[u8]) -> Result<XdcMessage, XdcStreamError> {
        if bytes.is_empty() {
            return Err(XdcStreamError::InvalidMessageId(0));
        }

        let message_id = bytes[0];
        let payload = &bytes[1..];

        let msg = match message_id {
            0x00 => XdcMessage::Status(Decodable::decode(&mut &payload[..])?),
            0x01 => XdcMessage::NewBlockHashes(Decodable::decode(&mut &payload[..])?),
            0x02 => XdcMessage::Transactions(Decodable::decode(&mut &payload[..])?),
            0x03 => XdcMessage::GetBlockHeaders(Decodable::decode(&mut &payload[..])?),
            0x04 => XdcMessage::BlockHeaders(Decodable::decode(&mut &payload[..])?),
            0x05 => XdcMessage::GetBlockBodies(Decodable::decode(&mut &payload[..])?),
            0x06 => XdcMessage::BlockBodies(Decodable::decode(&mut &payload[..])?),
            0x07 => XdcMessage::NewBlock(Box::new(Decodable::decode(&mut &payload[..])?)),
            0x0d => XdcMessage::GetNodeData(Decodable::decode(&mut &payload[..])?),
            0x0e => XdcMessage::NodeData(Decodable::decode(&mut &payload[..])?),
            0x0f => XdcMessage::GetReceipts(Decodable::decode(&mut &payload[..])?),
            0x10 => XdcMessage::Receipts(Decodable::decode(&mut &payload[..])?),
            0xe0 => XdcMessage::Vote(Decodable::decode(&mut &payload[..])?),
            0xe1 => XdcMessage::Timeout(Decodable::decode(&mut &payload[..])?),
            0xe2 => XdcMessage::SyncInfo(Decodable::decode(&mut &payload[..])?),
            id => return Err(XdcStreamError::InvalidMessageId(id)),
        };

        Ok(msg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{GetBlockHeaders63, HashOrNumber};
    use alloy_primitives::U256;
    use tokio_util::codec::{Framed, LengthDelimitedCodec};

    #[tokio::test]
    async fn test_stream_send_receive() {
        let (client, server) = tokio::io::duplex(4096);

        let client_stream = Framed::new(client, LengthDelimitedCodec::new());
        let mut client = XdcEthStream::new(client_stream, XdcVersion::Eth63);

        let server_stream = Framed::new(server, LengthDelimitedCodec::new());
        let mut server = XdcEthStream::new(server_stream, XdcVersion::Eth63);

        // Send message from client
        let msg = XdcMessage::GetBlockHeaders(GetBlockHeaders63 {
            origin: HashOrNumber::Number(100),
            amount: 10,
            skip: 0,
            reverse: false,
        });

        tokio::spawn(async move {
            client.send_message(msg).await.unwrap();
        });

        // Receive message on server
        let received = server.receive_message().await.unwrap();
        assert!(matches!(received, XdcMessage::GetBlockHeaders(_)));
    }

    #[test]
    fn test_request_matching() {
        assert!(RequestType::BlockHeaders.matches_response(&XdcMessage::BlockHeaders(
            crate::types::BlockHeaders63 {
                headers: vec![]
            }
        )));

        assert!(!RequestType::BlockHeaders.matches_response(&XdcMessage::BlockBodies(
            crate::types::BlockBodies63 {
                bodies: vec![]
            }
        )));
    }
}
