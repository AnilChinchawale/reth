//! eth/100 (XDPoS2) protocol implementation.
//!
//! This module provides support for XDC's custom XDPoS2 consensus protocol.
//!
//! Message types:
//! - Vote (0xe0): Validator votes on blocks
//! - Timeout (0xe1): Timeout messages for consensus
//! - SyncInfo (0xe2): Synchronization info exchange

pub mod messages;
pub mod handler;

pub use messages::*;
pub use handler::XdposHandler;

/// XDPoS2 protocol version
pub const XDPOS2_VERSION: u8 = 100;

/// XDPoS2 message IDs
pub const VOTE_MSG: u8 = 0xe0;
pub const TIMEOUT_MSG: u8 = 0xe1;
pub const SYNCINFO_MSG: u8 = 0xe2;
