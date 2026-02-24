//! XDC Network wire protocol implementation for Reth.
//!
//! This crate provides compatibility with XDC Network's P2P protocols:
//! - **eth/63**: Legacy Ethereum protocol without request IDs
//! - **eth/100**: XDPoS2 consensus protocol
//!
//! ## Protocol Versions
//!
//! XDC Network uses pre-EIP-2464 protocols without request IDs and without
//! ForkID validation in the handshake.
//!
//! ```rust
//! use reth_xdc_wire::{XdcVersion, Xdc63Status};
//!
//! let version = XdcVersion::Eth63;
//! assert!(!version.has_request_ids());
//! assert!(version.is_legacy());
//! ```
//!
//! ## Handshake
//!
//! XDC handshake validates network ID and genesis hash but does NOT check ForkID:
//!
//! ```rust,ignore
//! use reth_xdc_wire::{XdcHandshake, Xdc63Status};
//!
//! let status = Xdc63Status::new(63, 50, td, head, genesis);
//! let (peer_status, version) = XdcHandshake::execute(
//!     &mut stream,
//!     status,
//!     50,  // network_id
//!     genesis,
//! ).await?;
//! ```
//!
//! ## Message Types
//!
//! ### eth/63 (Legacy)
//!
//! ```rust
//! use reth_xdc_wire::{GetBlockHeaders63, HashOrNumber};
//!
//! let request = GetBlockHeaders63 {
//!     origin: HashOrNumber::Number(100),
//!     amount: 10,
//!     skip: 0,
//!     reverse: false,
//! };
//! // No request ID wrapping
//! ```
//!
//! ### eth/100 (XDPoS2)
//!
//! ```rust
//! use reth_xdc_wire::VoteMessage;
//! use alloy_primitives::{B256, Bytes};
//!
//! let vote = VoteMessage {
//!     round: 100,
//!     block_hash: B256::ZERO,
//!     signature: Bytes::new(),
//! };
//! ```

#![doc(
    html_logo_url = "https://raw.githubusercontent.com/paradigmxyz/reth/main/assets/reth-docs.png",
    html_favicon_url = "https://avatars0.githubusercontent.com/u/97369466?s=256",
    issue_tracker_base_url = "https://github.com/XinFinOrg/reth-xdc/issues/"
)]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![cfg_attr(docsrs, feature(doc_cfg))]

pub mod version;
pub use version::{XdcVersion, ProtocolVersion};

pub mod types;
pub use types::{
    HashOrNumber, Xdc63Status, XdcMessage, XdcMessageID,
    // eth/63 types
    GetBlockHeaders63, BlockHeaders63, GetBlockBodies63, BlockBodies63,
    GetNodeData63, NodeData63, GetReceipts63, Receipts63,
    // eth/100 types
    VoteMessage, TimeoutMessage, SyncInfoMessage,
};

pub mod handshake;
pub use handshake::{XdcHandshake, XdcHandshakeError};

pub mod stream;
pub use stream::{UnauthedXdcStream, XdcEthStream, XdcStreamError};

pub mod errors;
pub use errors::XdcWireError;

pub mod capability;
pub use capability::{xdc_capabilities, XDC_MAINNET_NETWORK_ID, XDC_APOTHEM_NETWORK_ID};

pub mod eth63;
pub mod eth100;

/// Maximum message size (10 MB)
pub const MAX_MESSAGE_SIZE: usize = 10 * 1024 * 1024;

/// Handshake timeout duration
pub const HANDSHAKE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

/// Default request timeout for eth/63 FIFO matching
pub const REQUEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

/// Maximum pending requests per peer for eth/63
pub const MAX_PENDING_REQUESTS: usize = 10;
