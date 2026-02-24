//! eth/63 protocol implementation (XDC legacy).
//!
//! This module provides support for the eth/63 protocol, which XDC Network uses
//! for backward compatibility with legacy nodes.
//!
//! Key differences from eth/66+:
//! - No request IDs in messages
//! - GetNodeData/NodeData messages (removed in eth/66+)
//! - Implicit FIFO request-response matching

pub mod messages;
pub mod handler;

pub use messages::*;
pub use handler::Eth63Handler;

/// eth/63 protocol version
pub const ETH63_VERSION: u8 = 63;

/// Number of message types in eth/63
pub const ETH63_MESSAGE_COUNT: u8 = 17;
