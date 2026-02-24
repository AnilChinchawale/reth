//! XDC handshake implementation.
//!
//! XDC handshake differs from standard Ethereum:
//! - No ForkID validation
//! - 5-field status message (no ForkID field)
//! - Validates only network ID and genesis hash

use crate::{
    errors::XdcHandshakeError,
    types::{Xdc63Status, XdcMessage},
    version::XdcVersion,
    HANDSHAKE_TIMEOUT,
};
use alloy_primitives::B256;
use futures::{Sink, SinkExt, Stream, StreamExt};
use std::time::Duration;
use tokio::time::timeout;
use tracing::{debug, trace};

/// XDC handshake handler
pub struct XdcHandshake;

impl XdcHandshake {
    /// Execute XDC handshake
    ///
    /// This performs the eth/63-style status exchange without ForkID validation.
    ///
    /// # Arguments
    ///
    /// * `stream` - The underlying transport stream
    /// * `local_status` - Our status to send to the peer
    /// * `network_id` - Expected network ID (50 for mainnet, 51 for Apothem)
    /// * `genesis_hash` - Expected genesis hash
    ///
    /// # Returns
    ///
    /// Returns the peer's status and negotiated protocol version on success.
    pub async fn execute<S>(
        stream: &mut S,
        local_status: Xdc63Status,
        network_id: u64,
        genesis_hash: B256,
    ) -> Result<(Xdc63Status, XdcVersion), XdcHandshakeError>
    where
        S: Stream<Item = Result<bytes::BytesMut, std::io::Error>>
            + Sink<bytes::Bytes, Error = std::io::Error>
            + Unpin,
    {
        Self::execute_with_timeout(stream, local_status, network_id, genesis_hash, HANDSHAKE_TIMEOUT).await
    }

    /// Execute XDC handshake with custom timeout
    pub async fn execute_with_timeout<S>(
        stream: &mut S,
        local_status: Xdc63Status,
        network_id: u64,
        genesis_hash: B256,
        timeout_duration: Duration,
    ) -> Result<(Xdc63Status, XdcVersion), XdcHandshakeError>
    where
        S: Stream<Item = Result<bytes::BytesMut, std::io::Error>>
            + Sink<bytes::Bytes, Error = std::io::Error>
            + Unpin,
    {
        timeout(
            timeout_duration,
            Self::execute_without_timeout(stream, local_status, network_id, genesis_hash),
        )
        .await
        .map_err(|_| XdcHandshakeError::Timeout)?
    }

    /// Execute handshake without timeout
    async fn execute_without_timeout<S>(
        stream: &mut S,
        local_status: Xdc63Status,
        network_id: u64,
        genesis_hash: B256,
    ) -> Result<(Xdc63Status, XdcVersion), XdcHandshakeError>
    where
        S: Stream<Item = Result<bytes::BytesMut, std::io::Error>>
            + Sink<bytes::Bytes, Error = std::io::Error>
            + Unpin,
    {
        trace!(
            version = local_status.protocol_version,
            network_id = local_status.network_id,
            "Sending XDC status to peer"
        );

        // 1. Send our status
        let status_bytes = Self::encode_status(&local_status)?;
        stream.send(status_bytes).await?;

        // 2. Receive peer status
        let peer_bytes = stream
            .next()
            .await
            .ok_or(XdcHandshakeError::ConnectionClosed)??;

        let peer_status = Self::decode_status(&peer_bytes)?;

        trace!(
            version = peer_status.protocol_version,
            network_id = peer_status.network_id,
            "Received XDC status from peer"
        );

        // 3. Validate status (NO ForkID validation)
        Self::validate_status(&peer_status, network_id, genesis_hash)?;

        // 4. Determine negotiated version
        let version = XdcVersion::try_from(peer_status.protocol_version)
            .map_err(|_| XdcHandshakeError::UnsupportedVersion(peer_status.protocol_version))?;

        debug!(
            ?version,
            peer_network_id = peer_status.network_id,
            "XDC handshake successful"
        );

        Ok((peer_status, version))
    }

    /// Encode status message
    fn encode_status(status: &Xdc63Status) -> Result<bytes::Bytes, XdcHandshakeError> {
        use alloy_rlp::Encodable;

        let mut buf = Vec::new();
        
        // Message ID (0x00 for Status)
        buf.push(0x00);
        
        // Encode status
        status.encode(&mut buf);

        Ok(bytes::Bytes::from(buf))
    }

    /// Decode status message
    fn decode_status(bytes: &[u8]) -> Result<Xdc63Status, XdcHandshakeError> {
        use alloy_rlp::Decodable;

        if bytes.is_empty() {
            return Err(XdcHandshakeError::UnexpectedMessage);
        }

        // Check message ID
        if bytes[0] != 0x00 {
            return Err(XdcHandshakeError::UnexpectedMessage);
        }

        // Decode status
        let status = Xdc63Status::decode(&mut &bytes[1..])?;

        Ok(status)
    }

    /// Validate peer status
    ///
    /// XDC validation rules:
    /// 1. Network ID must match
    /// 2. Genesis hash must match
    /// 3. NO ForkID validation (key difference from standard Ethereum)
    fn validate_status(
        peer_status: &Xdc63Status,
        expected_network_id: u64,
        expected_genesis: B256,
    ) -> Result<(), XdcHandshakeError> {
        // Validate network ID
        if peer_status.network_id != expected_network_id {
            return Err(XdcHandshakeError::NetworkIdMismatch {
                expected: expected_network_id,
                received: peer_status.network_id,
            });
        }

        // Validate genesis hash
        if peer_status.genesis_hash != expected_genesis {
            return Err(XdcHandshakeError::GenesisMismatch {
                expected: expected_genesis,
                received: peer_status.genesis_hash,
            });
        }

        // Note: We do NOT validate ForkID - this is the key XDC compatibility difference

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::U256;
    use tokio_util::codec::{Framed, LengthDelimitedCodec};

    #[tokio::test]
    async fn test_handshake_success() {
        let (client, server) = tokio::io::duplex(1024);

        let genesis = B256::random();
        let client_status = Xdc63Status::new(63, 50, U256::ZERO, B256::random(), genesis);
        let server_status = Xdc63Status::new(63, 50, U256::from(100), B256::random(), genesis);

        let mut client_stream = Framed::new(client, LengthDelimitedCodec::new());
        let mut server_stream = Framed::new(server, LengthDelimitedCodec::new());

        let client_handle = tokio::spawn(async move {
            XdcHandshake::execute(&mut client_stream, client_status, 50, genesis).await
        });

        let server_handle = tokio::spawn(async move {
            XdcHandshake::execute(&mut server_stream, server_status.clone(), 50, genesis)
                .await
                .map(|(status, _)| status)
        });

        let (client_result, server_result) = tokio::join!(client_handle, server_handle);

        let (peer_status, version) = client_result.unwrap().unwrap();
        assert_eq!(peer_status.network_id, 50);
        assert_eq!(version, XdcVersion::Eth63);

        let peer_status = server_result.unwrap().unwrap();
        assert_eq!(peer_status.network_id, 50);
    }

    #[tokio::test]
    async fn test_handshake_network_id_mismatch() {
        let (client, server) = tokio::io::duplex(1024);

        let genesis = B256::random();
        let client_status = Xdc63Status::new(63, 50, U256::ZERO, B256::random(), genesis);
        let server_status = Xdc63Status::new(63, 51, U256::ZERO, B256::random(), genesis);

        let mut client_stream = Framed::new(client, LengthDelimitedCodec::new());
        let mut server_stream = Framed::new(server, LengthDelimitedCodec::new());

        let client_handle = tokio::spawn(async move {
            XdcHandshake::execute(&mut client_stream, client_status, 50, genesis).await
        });

        let server_handle = tokio::spawn(async move {
            XdcHandshake::execute(&mut server_stream, server_status, 51, genesis).await
        });

        let (client_result, server_result) = tokio::join!(client_handle, server_handle);

        assert!(matches!(
            client_result.unwrap().unwrap_err(),
            XdcHandshakeError::NetworkIdMismatch { .. }
        ));
        assert!(matches!(
            server_result.unwrap().unwrap_err(),
            XdcHandshakeError::NetworkIdMismatch { .. }
        ));
    }

    #[tokio::test]
    async fn test_handshake_genesis_mismatch() {
        let (client, server) = tokio::io::duplex(1024);

        let client_genesis = B256::random();
        let server_genesis = B256::random();

        let client_status = Xdc63Status::new(63, 50, U256::ZERO, B256::random(), client_genesis);
        let server_status = Xdc63Status::new(63, 50, U256::ZERO, B256::random(), server_genesis);

        let mut client_stream = Framed::new(client, LengthDelimitedCodec::new());
        let mut server_stream = Framed::new(server, LengthDelimitedCodec::new());

        let client_handle = tokio::spawn(async move {
            XdcHandshake::execute(&mut client_stream, client_status, 50, client_genesis).await
        });

        let server_handle = tokio::spawn(async move {
            XdcHandshake::execute(&mut server_stream, server_status, 50, server_genesis).await
        });

        let (client_result, server_result) = tokio::join!(client_handle, server_handle);

        assert!(matches!(
            client_result.unwrap().unwrap_err(),
            XdcHandshakeError::GenesisMismatch { .. }
        ));
        assert!(matches!(
            server_result.unwrap().unwrap_err(),
            XdcHandshakeError::GenesisMismatch { .. }
        ));
    }
}
