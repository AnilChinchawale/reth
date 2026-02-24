//! XDC protocol version definitions and traits.

use alloy_rlp::{Decodable, Encodable, Error as RlpError};
use bytes::BufMut;
use core::{fmt, str::FromStr};
use derive_more::Display;

/// Error thrown when failed to parse a valid [`XdcVersion`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("Unknown XDC protocol version: {0}")]
pub struct ParseVersionError(pub(crate) String);

/// The XDC protocol versions.
///
/// XDC Network uses legacy Ethereum protocols (eth/62, eth/63) without request IDs,
/// plus a custom XDPoS2 protocol (version 100).
#[repr(u8)]
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Display)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum XdcVersion {
    /// eth/62: Basic block and transaction sync (XDC legacy)
    Eth62 = 62,
    /// eth/63: Adds state sync messages (XDC legacy)
    Eth63 = 63,
    /// eth/66: Modern Ethereum with request IDs (some modern XDC nodes)
    Eth66 = 66,
    /// eth/100: XDPoS2 consensus protocol (XDC custom)
    Eth100 = 100,
}

impl XdcVersion {
    /// The latest supported eth version (for compatibility with modern Ethereum nodes)
    pub const LATEST_ETH: Self = Self::Eth66;

    /// The default XDC version (legacy compatibility)
    pub const DEFAULT_XDC: Self = Self::Eth63;

    /// All supported versions in preference order
    pub const ALL_VERSIONS: &'static [Self] = &[
        Self::Eth100, // XDPoS2 consensus
        Self::Eth66,  // Modern with request IDs
        Self::Eth63,  // Legacy (most common)
        Self::Eth62,  // Oldest legacy
    ];

    /// Returns true if the version uses request IDs (EIP-2464)
    pub const fn has_request_ids(&self) -> bool {
        matches!(self, Self::Eth66)
    }

    /// Returns true if the version is a legacy protocol (pre-EIP-2464)
    pub const fn is_legacy(&self) -> bool {
        matches!(self, Self::Eth62 | Self::Eth63)
    }

    /// Returns true if the version is the XDPoS2 consensus protocol
    pub const fn is_consensus(&self) -> bool {
        matches!(self, Self::Eth100)
    }

    /// Returns true if the version is eth/63
    pub const fn is_eth63(&self) -> bool {
        matches!(self, Self::Eth63)
    }

    /// Returns true if the version is eth/66
    pub const fn is_eth66(&self) -> bool {
        matches!(self, Self::Eth66)
    }

    /// Returns the protocol name for capability negotiation
    pub const fn protocol_name(&self) -> &'static str {
        match self {
            Self::Eth62 | Self::Eth63 | Self::Eth66 => "eth",
            Self::Eth100 => "xdpos",
        }
    }

    /// Returns the number of message types for this version
    pub const fn message_count(&self) -> u8 {
        match self {
            Self::Eth62 => 8,    // 0x00-0x07
            Self::Eth63 => 17,   // 0x00-0x10
            Self::Eth66 => 17,   // 0x00-0x10 (same message IDs, but wrapped)
            Self::Eth100 => 227, // 0x00-0xe2 (includes eth/63 + consensus messages)
        }
    }
}

/// RLP encodes `XdcVersion` as a single byte.
impl Encodable for XdcVersion {
    fn encode(&self, out: &mut dyn BufMut) {
        (*self as u8).encode(out)
    }

    fn length(&self) -> usize {
        (*self as u8).length()
    }
}

/// RLP decodes a single byte into `XdcVersion`.
impl Decodable for XdcVersion {
    fn decode(buf: &mut &[u8]) -> alloy_rlp::Result<Self> {
        let version = u8::decode(buf)?;
        Self::try_from(version).map_err(|_| RlpError::Custom("invalid XDC protocol version"))
    }
}

impl TryFrom<&str> for XdcVersion {
    type Error = ParseVersionError;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "62" => Ok(Self::Eth62),
            "63" => Ok(Self::Eth63),
            "66" => Ok(Self::Eth66),
            "100" => Ok(Self::Eth100),
            _ => Err(ParseVersionError(s.to_string())),
        }
    }
}

impl TryFrom<u8> for XdcVersion {
    type Error = ParseVersionError;

    fn try_from(u: u8) -> Result<Self, Self::Error> {
        match u {
            62 => Ok(Self::Eth62),
            63 => Ok(Self::Eth63),
            66 => Ok(Self::Eth66),
            100 => Ok(Self::Eth100),
            _ => Err(ParseVersionError(u.to_string())),
        }
    }
}

impl TryFrom<u32> for XdcVersion {
    type Error = ParseVersionError;

    fn try_from(u: u32) -> Result<Self, Self::Error> {
        if u > 255 {
            return Err(ParseVersionError(u.to_string()))
        }
        Self::try_from(u as u8)
    }
}

impl FromStr for XdcVersion {
    type Err = ParseVersionError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::try_from(s)
    }
}

impl From<XdcVersion> for u8 {
    fn from(v: XdcVersion) -> Self {
        v as Self
    }
}

impl From<XdcVersion> for u32 {
    fn from(v: XdcVersion) -> Self {
        v as u8 as Self
    }
}

impl From<XdcVersion> for &'static str {
    fn from(v: XdcVersion) -> &'static str {
        match v {
            XdcVersion::Eth62 => "62",
            XdcVersion::Eth63 => "63",
            XdcVersion::Eth66 => "66",
            XdcVersion::Eth100 => "100",
        }
    }
}

/// `RLPx` `p2p` protocol version
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ProtocolVersion {
    /// `p2p` version 4
    V4 = 4,
    /// `p2p` version 5
    #[default]
    V5 = 5,
}

impl fmt::Display for ProtocolVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "v{}", *self as u8)
    }
}

impl Encodable for ProtocolVersion {
    fn encode(&self, out: &mut dyn BufMut) {
        (*self as u8).encode(out)
    }
    fn length(&self) -> usize {
        (*self as u8).length()
    }
}

impl Decodable for ProtocolVersion {
    fn decode(buf: &mut &[u8]) -> alloy_rlp::Result<Self> {
        let version = u8::decode(buf)?;
        match version {
            4 => Ok(Self::V4),
            5 => Ok(Self::V5),
            _ => Err(RlpError::Custom("unknown p2p protocol version")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_rlp::{Decodable, Encodable};
    use bytes::BytesMut;

    #[test]
    fn test_xdc_version_try_from_str() {
        assert_eq!(XdcVersion::Eth62, XdcVersion::try_from("62").unwrap());
        assert_eq!(XdcVersion::Eth63, XdcVersion::try_from("63").unwrap());
        assert_eq!(XdcVersion::Eth66, XdcVersion::try_from("66").unwrap());
        assert_eq!(XdcVersion::Eth100, XdcVersion::try_from("100").unwrap());
        assert!(XdcVersion::try_from("99").is_err());
    }

    #[test]
    fn test_xdc_version_from_str() {
        assert_eq!(XdcVersion::Eth62, "62".parse().unwrap());
        assert_eq!(XdcVersion::Eth63, "63".parse().unwrap());
        assert_eq!(XdcVersion::Eth66, "66".parse().unwrap());
        assert_eq!(XdcVersion::Eth100, "100".parse().unwrap());
    }

    #[test]
    fn test_xdc_version_rlp_encode() {
        let versions = [
            XdcVersion::Eth62,
            XdcVersion::Eth63,
            XdcVersion::Eth66,
            XdcVersion::Eth100,
        ];

        for version in versions {
            let mut encoded = BytesMut::new();
            version.encode(&mut encoded);

            assert_eq!(encoded.len(), 1);
            assert_eq!(encoded[0], version as u8);
        }
    }

    #[test]
    fn test_xdc_version_rlp_decode() {
        let test_cases = [
            (62_u8, Ok(XdcVersion::Eth62)),
            (63_u8, Ok(XdcVersion::Eth63)),
            (66_u8, Ok(XdcVersion::Eth66)),
            (100_u8, Ok(XdcVersion::Eth100)),
            (99_u8, Err(RlpError::Custom("invalid XDC protocol version"))),
        ];

        for (input, expected) in test_cases {
            let mut encoded = BytesMut::new();
            input.encode(&mut encoded);

            let mut slice = encoded.as_ref();
            let result = XdcVersion::decode(&mut slice);
            assert_eq!(result, expected);
        }
    }

    #[test]
    fn test_version_properties() {
        assert!(!XdcVersion::Eth62.has_request_ids());
        assert!(!XdcVersion::Eth63.has_request_ids());
        assert!(XdcVersion::Eth66.has_request_ids());
        assert!(!XdcVersion::Eth100.has_request_ids());

        assert!(XdcVersion::Eth62.is_legacy());
        assert!(XdcVersion::Eth63.is_legacy());
        assert!(!XdcVersion::Eth66.is_legacy());
        assert!(!XdcVersion::Eth100.is_legacy());

        assert!(!XdcVersion::Eth63.is_consensus());
        assert!(XdcVersion::Eth100.is_consensus());
    }

    #[test]
    fn test_protocol_names() {
        assert_eq!(XdcVersion::Eth62.protocol_name(), "eth");
        assert_eq!(XdcVersion::Eth63.protocol_name(), "eth");
        assert_eq!(XdcVersion::Eth66.protocol_name(), "eth");
        assert_eq!(XdcVersion::Eth100.protocol_name(), "xdpos");
    }

    #[test]
    fn test_message_counts() {
        assert_eq!(XdcVersion::Eth62.message_count(), 8);
        assert_eq!(XdcVersion::Eth63.message_count(), 17);
        assert_eq!(XdcVersion::Eth66.message_count(), 17);
        assert_eq!(XdcVersion::Eth100.message_count(), 227);
    }
}
