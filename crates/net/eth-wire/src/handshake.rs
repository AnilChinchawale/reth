use crate::{
    errors::{EthHandshakeError, EthStreamError, P2PStreamError},
    ethstream::MAX_MESSAGE_SIZE,
    CanDisconnect,
};
use bytes::{Bytes, BytesMut};
use futures::{Sink, SinkExt, Stream};
use reth_eth_wire_types::{
    DisconnectReason, EthMessage, EthNetworkPrimitives, ProtocolMessage, StatusMessage,
    UnifiedStatus, StatusEth63, EthVersion,
};
use reth_ethereum_forks::ForkFilter;
use reth_primitives_traits::GotExpected;
use alloy_primitives::U256;
use std::{fmt::Debug, future::Future, pin::Pin, time::Duration};
use tokio::time::timeout;
use tokio_stream::StreamExt;
use tracing::{debug, trace, info};

/// A trait that knows how to perform the P2P handshake.
pub trait EthRlpxHandshake: Debug + Send + Sync + 'static {
    /// Perform the P2P handshake for the `eth` protocol.
    fn handshake<'a>(
        &'a self,
        unauth: &'a mut dyn UnauthEth,
        status: UnifiedStatus,
        fork_filter: ForkFilter,
        timeout_limit: Duration,
    ) -> Pin<Box<dyn Future<Output = Result<UnifiedStatus, EthStreamError>> + 'a + Send>>;
}

/// An unauthenticated stream that can send and receive messages.
pub trait UnauthEth:
    Stream<Item = Result<BytesMut, P2PStreamError>>
    + Sink<Bytes, Error = P2PStreamError>
    + CanDisconnect<Bytes>
    + Unpin
    + Send
{
}

impl<T> UnauthEth for T where
    T: Stream<Item = Result<BytesMut, P2PStreamError>>
        + Sink<Bytes, Error = P2PStreamError>
        + CanDisconnect<Bytes>
        + Unpin
        + Send
{
}

/// The Ethereum P2P handshake.
#[derive(Debug, Default, Clone)]
#[non_exhaustive]
pub struct EthHandshake;

impl EthRlpxHandshake for EthHandshake {
    fn handshake<'a>(
        &'a self,
        unauth: &'a mut dyn UnauthEth,
        status: UnifiedStatus,
        fork_filter: ForkFilter,
        timeout_limit: Duration,
    ) -> Pin<Box<dyn Future<Output = Result<UnifiedStatus, EthStreamError>> + 'a + Send>> {
        Box::pin(async move {
            // Check if this is an XDC chain
            let status_msg = status.into_message();
            let is_xdc_chain = matches!(status_msg.chain().id(), 50 | 51);
            
            if is_xdc_chain {
                info!(chain_id = status_msg.chain().id(), "Using XDC handshake (no ForkID)");
                timeout(timeout_limit, XdcEthHandshake(unauth).xdc_handshake(status_msg))
                    .await
                    .map_err(|_| EthStreamError::StreamTimeout)?
            } else {
                timeout(timeout_limit, EthereumEthHandshake(unauth).eth_handshake(status_msg, fork_filter))
                    .await
                    .map_err(|_| EthStreamError::StreamTimeout)?
            }
        })
    }
}

/// A type that performs the ethereum specific `eth` protocol handshake.
#[derive(Debug)]
pub struct EthereumEthHandshake<'a, S: ?Sized>(pub &'a mut S);

impl<S: ?Sized, E> EthereumEthHandshake<'_, S>
where
    S: Stream<Item = Result<BytesMut, E>> + CanDisconnect<Bytes> + Send + Unpin,
    EthStreamError: From<E> + From<<S as Sink<Bytes>>::Error>,
{
    /// Performs the `eth` rlpx protocol handshake using the given input stream.
    pub async fn eth_handshake(
        self,
        status: StatusMessage,
        fork_filter: ForkFilter,
    ) -> Result<UnifiedStatus, EthStreamError> {
        let unauth = self.0;

        // Send our status message
        let status_msg = alloy_rlp::encode(ProtocolMessage::<EthNetworkPrimitives>::from(
            EthMessage::Status(status.clone()),
        ))
        .into();
        unauth.send(status_msg).await.map_err(EthStreamError::from)?;

        // Receive peer's response
        let their_msg_res = unauth.next().await;
        let their_msg = match their_msg_res {
            Some(Ok(msg)) => msg,
            Some(Err(e)) => return Err(EthStreamError::from(e)),
            None => {
                unauth
                    .disconnect(DisconnectReason::DisconnectRequested)
                    .await
                    .map_err(EthStreamError::from)?;
                return Err(EthStreamError::EthHandshakeError(EthHandshakeError::NoResponse));
            }
        };

        if their_msg.len() > MAX_MESSAGE_SIZE {
            unauth
                .disconnect(DisconnectReason::ProtocolBreach)
                .await
                .map_err(EthStreamError::from)?;
            return Err(EthStreamError::MessageTooBig(their_msg.len()));
        }

        let version = status.version();
        let their_status_message = match ProtocolMessage::<EthNetworkPrimitives>::decode_status(
            version,
            &mut their_msg.as_ref(),
        ) {
            Ok(status) => status,
            Err(err) => {
                debug!("decode error in eth handshake: msg={their_msg:x}");
                unauth
                    .disconnect(DisconnectReason::ProtocolBreach)
                    .await
                    .map_err(EthStreamError::from)?;
                return Err(EthStreamError::InvalidMessage(err));
            }
        };

        trace!("Validating incoming ETH status from peer");

        if status.genesis() != their_status_message.genesis() {
            unauth
                .disconnect(DisconnectReason::ProtocolBreach)
                .await
                .map_err(EthStreamError::from)?;
            return Err(EthHandshakeError::MismatchedGenesis(
                GotExpected { expected: status.genesis(), got: their_status_message.genesis() }
                    .into(),
            )
            .into());
        }

        if status.version() != their_status_message.version() {
            unauth
                .disconnect(DisconnectReason::ProtocolBreach)
                .await
                .map_err(EthStreamError::from)?;
            return Err(EthHandshakeError::MismatchedProtocolVersion(GotExpected {
                got: their_status_message.version(),
                expected: status.version(),
            })
            .into());
        }

        if *status.chain() != *their_status_message.chain() {
            unauth
                .disconnect(DisconnectReason::ProtocolBreach)
                .await
                .map_err(EthStreamError::from)?;
            return Err(EthHandshakeError::MismatchedChain(GotExpected {
                got: *their_status_message.chain(),
                expected: *status.chain(),
            })
            .into());
        }

        // Ensure peer's total difficulty is reasonable
        if let StatusMessage::Legacy(s) = &their_status_message &&
            s.total_difficulty.bit_len() > 160
        {
            unauth
                .disconnect(DisconnectReason::ProtocolBreach)
                .await
                .map_err(EthStreamError::from)?;
            return Err(EthHandshakeError::TotalDifficultyBitLenTooLarge {
                got: s.total_difficulty.bit_len(),
                maximum: 160,
            }
            .into());
        }

        // Fork validation for non-XDC chains
        if let Err(err) = fork_filter
            .validate(their_status_message.forkid())
            .map_err(EthHandshakeError::InvalidFork)
        {
            unauth
                .disconnect(DisconnectReason::ProtocolBreach)
                .await
                .map_err(EthStreamError::from)?;
            return Err(err.into());
        }

        if let StatusMessage::Eth69(s) = &their_status_message {
            if s.earliest > s.latest {
                return Err(EthHandshakeError::EarliestBlockGreaterThanLatestBlock {
                    got: s.earliest,
                    latest: s.latest,
                }
                .into());
            }

            if s.blockhash.is_zero() {
                return Err(EthHandshakeError::BlockhashZero.into());
            }
        }

        Ok(UnifiedStatus::from_message(their_status_message))
    }
}

/// XDC-specific handshake handler
#[derive(Debug)]
pub struct XdcEthHandshake<'a, S: ?Sized>(pub &'a mut S);

impl<S: ?Sized, E> XdcEthHandshake<'_, S>
where
    S: Stream<Item = Result<BytesMut, E>> + CanDisconnect<Bytes> + Send + Unpin,
    EthStreamError: From<E> + From<<S as Sink<Bytes>>::Error>,
{
    /// Performs the XDC eth/63 handshake (no ForkID)
    pub async fn xdc_handshake(
        self,
        status: StatusMessage,
    ) -> Result<UnifiedStatus, EthStreamError> {
        let unauth = self.0;
        
        info!("Starting XDC handshake (eth/63 without ForkID)");

        // For XDC, we need to send a simplified eth/63 status without ForkID
        // Convert to StatusEth63 format
        let xdc_status = match status {
            StatusMessage::Eth63(s) => s,
            StatusMessage::Legacy(s) => StatusEth63 {
                version: s.version,
                chain: s.chain,
                total_difficulty: s.total_difficulty,
                blockhash: s.blockhash,
                genesis: s.genesis,
            },
            StatusMessage::Eth69(s) => StatusEth63 {
                version: EthVersion::Eth63, // Force eth/63 for XDC
                chain: s.chain,
                total_difficulty: U256::ZERO, // Eth69 doesn't have total_difficulty
                blockhash: s.blockhash,
                genesis: s.genesis,
            },
        };

        // Send our status message as eth/63
        let status_msg = alloy_rlp::encode(ProtocolMessage::<EthNetworkPrimitives>::from(
            EthMessage::Status(StatusMessage::Eth63(xdc_status.clone())),
        ))
        .into();
        
        info!(version = ?xdc_status.version, chain = %xdc_status.chain, "Sending XDC status");
        unauth.send(status_msg).await.map_err(EthStreamError::from)?;

        // Receive peer's response
        let their_msg_res = unauth.next().await;
        let their_msg = match their_msg_res {
            Some(Ok(msg)) => msg,
            Some(Err(e)) => return Err(EthStreamError::from(e)),
            None => {
                unauth
                    .disconnect(DisconnectReason::DisconnectRequested)
                    .await
                    .map_err(EthStreamError::from)?;
                return Err(EthStreamError::EthHandshakeError(EthHandshakeError::NoResponse));
            }
        };

        if their_msg.len() > MAX_MESSAGE_SIZE {
            unauth
                .disconnect(DisconnectReason::ProtocolBreach)
                .await
                .map_err(EthStreamError::from)?;
            return Err(EthStreamError::MessageTooBig(their_msg.len()));
        }

        // Decode peer's status - accept eth/63 format
        let their_status_message = match ProtocolMessage::<EthNetworkPrimitives>::decode_status(
            EthVersion::Eth63, // Force eth/63 decoding for XDC
            &mut their_msg.as_ref(),
        ) {
            Ok(status) => {
                info!(version = ?status.version(), "Received XDC peer status");
                status
            }
            Err(err) => {
                debug!("decode error in XDC handshake: msg={their_msg:x}");
                unauth
                    .disconnect(DisconnectReason::ProtocolBreach)
                    .await
                    .map_err(EthStreamError::from)?;
                return Err(EthStreamError::InvalidMessage(err));
            }
        };

        trace!("Validating incoming XDC status from peer");

        // Validate genesis
        if xdc_status.genesis != their_status_message.genesis() {
            unauth
                .disconnect(DisconnectReason::ProtocolBreach)
                .await
                .map_err(EthStreamError::from)?;
            return Err(EthHandshakeError::MismatchedGenesis(
                GotExpected { 
                    expected: xdc_status.genesis, 
                    got: their_status_message.genesis() 
                }.into(),
            )
            .into());
        }

        // Validate network ID (chain ID)
        if xdc_status.chain != *their_status_message.chain() {
            unauth
                .disconnect(DisconnectReason::ProtocolBreach)
                .await
                .map_err(EthStreamError::from)?;
            return Err(EthHandshakeError::MismatchedChain(GotExpected {
                got: *their_status_message.chain(),
                expected: xdc_status.chain,
            })
            .into());
        }

        // For XDC, we accept any eth/63 compatible version
        // Don't validate ForkID - XDC doesn't use it
        info!("XDC handshake successful - peer connected");

        Ok(UnifiedStatus::from_message(their_status_message))
    }
}
