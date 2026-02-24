//! XDC wire protocol message types.

use alloy_primitives::{Address, Bytes, B256, U256};
use alloy_rlp::{Decodable, Encodable, RlpDecodable, RlpEncodable};
use reth_primitives::{BlockBody, Header, TransactionSigned, Withdrawal};
use std::fmt;

/// XDC protocol message IDs
#[repr(u8)]
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum XdcMessageID {
    // eth/62, eth/63, eth/66 messages
    Status = 0x00,
    NewBlockHashes = 0x01,
    Transactions = 0x02,
    GetBlockHeaders = 0x03,
    BlockHeaders = 0x04,
    GetBlockBodies = 0x05,
    BlockBodies = 0x06,
    NewBlock = 0x07,
    // eth/63 state sync (not used in eth/66+)
    GetNodeData = 0x0d,
    NodeData = 0x0e,
    GetReceipts = 0x0f,
    Receipts = 0x10,
    // eth/100 XDPoS2 consensus messages
    Vote = 0xe0,
    Timeout = 0xe1,
    SyncInfo = 0xe2,
}

impl XdcMessageID {
    /// Returns true if this message is a request that expects a response
    pub const fn is_request(&self) -> bool {
        matches!(
            self,
            Self::GetBlockHeaders | Self::GetBlockBodies | Self::GetNodeData | Self::GetReceipts
        )
    }

    /// Returns true if this message is a response to a request
    pub const fn is_response(&self) -> bool {
        matches!(
            self,
            Self::BlockHeaders | Self::BlockBodies | Self::NodeData | Self::Receipts
        )
    }
}

impl From<XdcMessageID> for u8 {
    fn from(id: XdcMessageID) -> Self {
        id as u8
    }
}

impl TryFrom<u8> for XdcMessageID {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x00 => Ok(Self::Status),
            0x01 => Ok(Self::NewBlockHashes),
            0x02 => Ok(Self::Transactions),
            0x03 => Ok(Self::GetBlockHeaders),
            0x04 => Ok(Self::BlockHeaders),
            0x05 => Ok(Self::GetBlockBodies),
            0x06 => Ok(Self::BlockBodies),
            0x07 => Ok(Self::NewBlock),
            0x0d => Ok(Self::GetNodeData),
            0x0e => Ok(Self::NodeData),
            0x0f => Ok(Self::GetReceipts),
            0x10 => Ok(Self::Receipts),
            0xe0 => Ok(Self::Vote),
            0xe1 => Ok(Self::Timeout),
            0xe2 => Ok(Self::SyncInfo),
            _ => Err(()),
        }
    }
}

impl Encodable for XdcMessageID {
    fn encode(&self, out: &mut dyn bytes::BufMut) {
        (*self as u8).encode(out)
    }
    fn length(&self) -> usize {
        (*self as u8).length()
    }
}

impl Decodable for XdcMessageID {
    fn decode(buf: &mut &[u8]) -> alloy_rlp::Result<Self> {
        let id = u8::decode(buf)?;
        Self::try_from(id).map_err(|_| alloy_rlp::Error::Custom("invalid message ID"))
    }
}

/// XDC protocol messages (all versions)
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum XdcMessage {
    // eth/63 messages
    Status(Xdc63Status),
    NewBlockHashes(NewBlockHashes),
    Transactions(Transactions),
    GetBlockHeaders(GetBlockHeaders63),
    BlockHeaders(BlockHeaders63),
    GetBlockBodies(GetBlockBodies63),
    BlockBodies(BlockBodies63),
    NewBlock(Box<NewBlock>),
    GetNodeData(GetNodeData63),
    NodeData(NodeData63),
    GetReceipts(GetReceipts63),
    Receipts(Receipts63),
    // eth/100 consensus messages
    Vote(VoteMessage),
    Timeout(TimeoutMessage),
    SyncInfo(SyncInfoMessage),
}

impl XdcMessage {
    /// Returns the message ID
    pub const fn message_id(&self) -> XdcMessageID {
        match self {
            Self::Status(_) => XdcMessageID::Status,
            Self::NewBlockHashes(_) => XdcMessageID::NewBlockHashes,
            Self::Transactions(_) => XdcMessageID::Transactions,
            Self::GetBlockHeaders(_) => XdcMessageID::GetBlockHeaders,
            Self::BlockHeaders(_) => XdcMessageID::BlockHeaders,
            Self::GetBlockBodies(_) => XdcMessageID::GetBlockBodies,
            Self::BlockBodies(_) => XdcMessageID::BlockBodies,
            Self::NewBlock(_) => XdcMessageID::NewBlock,
            Self::GetNodeData(_) => XdcMessageID::GetNodeData,
            Self::NodeData(_) => XdcMessageID::NodeData,
            Self::GetReceipts(_) => XdcMessageID::GetReceipts,
            Self::Receipts(_) => XdcMessageID::Receipts,
            Self::Vote(_) => XdcMessageID::Vote,
            Self::Timeout(_) => XdcMessageID::Timeout,
            Self::SyncInfo(_) => XdcMessageID::SyncInfo,
        }
    }

    /// Returns true if this is a request message
    pub const fn is_request(&self) -> bool {
        self.message_id().is_request()
    }

    /// Returns true if this is a response message
    pub const fn is_response(&self) -> bool {
        self.message_id().is_response()
    }
}

/// XDC Status message (eth/63 compatible - no ForkID)
#[derive(Clone, Debug, PartialEq, Eq, RlpEncodable, RlpDecodable)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Xdc63Status {
    /// Protocol version (62, 63, or 100)
    pub protocol_version: u32,
    /// Network ID (50 for mainnet, 51 for Apothem)
    pub network_id: u64,
    /// Total difficulty
    pub total_difficulty: U256,
    /// Head block hash
    pub head_hash: B256,
    /// Genesis block hash
    pub genesis_hash: B256,
    // Note: NO ForkID field (XDC compatibility)
}

impl Xdc63Status {
    /// Create a new XDC status message
    pub const fn new(
        protocol_version: u32,
        network_id: u64,
        total_difficulty: U256,
        head_hash: B256,
        genesis_hash: B256,
    ) -> Self {
        Self {
            protocol_version,
            network_id,
            total_difficulty,
            head_hash,
            genesis_hash,
        }
    }
}

/// Hash or block number for GetBlockHeaders origin
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum HashOrNumber {
    /// Block hash
    Hash(B256),
    /// Block number
    Number(u64),
}

impl Encodable for HashOrNumber {
    fn encode(&self, out: &mut dyn bytes::BufMut) {
        match self {
            Self::Hash(hash) => hash.encode(out),
            Self::Number(number) => number.encode(out),
        }
    }

    fn length(&self) -> usize {
        match self {
            Self::Hash(hash) => hash.length(),
            Self::Number(number) => number.length(),
        }
    }
}

impl Decodable for HashOrNumber {
    fn decode(buf: &mut &[u8]) -> alloy_rlp::Result<Self> {
        // Try to decode as number first (more common)
        if let Ok(number) = u64::decode(&mut &buf[..]) {
            *buf = &buf[number.length()..];
            return Ok(Self::Number(number))
        }
        // Otherwise decode as hash
        Ok(Self::Hash(B256::decode(buf)?))
    }
}

/// GetBlockHeaders request (eth/63 - no request ID)
#[derive(Clone, Debug, PartialEq, Eq, RlpEncodable, RlpDecodable)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetBlockHeaders63 {
    /// Block hash or number to start from
    pub origin: HashOrNumber,
    /// Maximum number of headers to retrieve
    pub amount: u64,
    /// Blocks to skip between consecutive headers
    pub skip: u64,
    /// Query direction (false = rising, true = falling)
    pub reverse: bool,
}

/// BlockHeaders response (eth/63 - no request ID)
#[derive(Clone, Debug, PartialEq, Eq, RlpEncodable, RlpDecodable)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BlockHeaders63 {
    /// Block headers
    pub headers: Vec<Header>,
}

/// GetBlockBodies request (eth/63 - no request ID)
#[derive(Clone, Debug, PartialEq, Eq, RlpEncodable, RlpDecodable)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetBlockBodies63 {
    /// Block hashes to retrieve bodies for
    pub hashes: Vec<B256>,
}

/// BlockBodies response (eth/63 - no request ID)
#[derive(Clone, Debug, PartialEq, Eq, RlpEncodable, RlpDecodable)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BlockBodies63 {
    /// Block bodies
    pub bodies: Vec<BlockBody>,
}

/// GetNodeData request (eth/63 only - removed in eth/66)
#[derive(Clone, Debug, PartialEq, Eq, RlpEncodable, RlpDecodable)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetNodeData63 {
    /// State trie node hashes
    pub hashes: Vec<B256>,
}

/// NodeData response (eth/63 only - removed in eth/66)
#[derive(Clone, Debug, PartialEq, Eq, RlpEncodable, RlpDecodable)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NodeData63 {
    /// State trie node data
    pub data: Vec<Bytes>,
}

/// GetReceipts request (eth/63 - no request ID)
#[derive(Clone, Debug, PartialEq, Eq, RlpEncodable, RlpDecodable)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetReceipts63 {
    /// Block hashes to retrieve receipts for
    pub hashes: Vec<B256>,
}

/// Receipts response (eth/63 - no request ID)
#[derive(Clone, Debug, PartialEq, Eq, RlpEncodable, RlpDecodable)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Receipts63 {
    /// Receipts by block
    pub receipts: Vec<Vec<reth_primitives::Receipt>>,
}

/// NewBlockHashes announcement
#[derive(Clone, Debug, PartialEq, Eq, RlpEncodable, RlpDecodable)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NewBlockHashes {
    /// Block hash and number pairs
    pub hashes: Vec<BlockHashNumber>,
}

/// Block hash and number pair
#[derive(Clone, Debug, PartialEq, Eq, RlpEncodable, RlpDecodable)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BlockHashNumber {
    /// Block hash
    pub hash: B256,
    /// Block number
    pub number: u64,
}

/// Transactions broadcast
#[derive(Clone, Debug, PartialEq, Eq, RlpEncodable, RlpDecodable)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Transactions {
    /// Transactions
    pub transactions: Vec<TransactionSigned>,
}

/// NewBlock announcement
#[derive(Clone, Debug, PartialEq, Eq, RlpEncodable, RlpDecodable)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NewBlock {
    /// Block header
    pub header: Header,
    /// Block body
    pub body: BlockBody,
    /// Total difficulty
    pub total_difficulty: U256,
}

/// XDPoS2 Vote message (eth/100)
#[derive(Clone, Debug, PartialEq, Eq, RlpEncodable, RlpDecodable)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct VoteMessage {
    /// Consensus round number
    pub round: u64,
    /// Block hash being voted for
    pub block_hash: B256,
    /// BLS signature (96 bytes)
    pub signature: Bytes,
}

/// XDPoS2 Timeout message (eth/100)
#[derive(Clone, Debug, PartialEq, Eq, RlpEncodable, RlpDecodable)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TimeoutMessage {
    /// Consensus round number
    pub round: u64,
    /// BLS signature (96 bytes)
    pub signature: Bytes,
}

/// XDPoS2 SyncInfo message (eth/100)
#[derive(Clone, Debug, PartialEq, Eq, RlpEncodable, RlpDecodable)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SyncInfoMessage {
    /// Highest quorum certificate
    pub highest_qc: Bytes,
    /// Highest timeout certificate
    pub highest_tc: Bytes,
    /// Latest block number
    pub latest_block_no: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_id_properties() {
        assert!(XdcMessageID::GetBlockHeaders.is_request());
        assert!(XdcMessageID::BlockHeaders.is_response());
        assert!(!XdcMessageID::NewBlock.is_request());
        assert!(!XdcMessageID::NewBlock.is_response());
    }

    #[test]
    fn test_xdc_status_encoding() {
        let status = Xdc63Status::new(
            63,
            50,
            U256::from(1000),
            B256::random(),
            B256::random(),
        );

        let mut encoded = Vec::new();
        status.encode(&mut encoded);

        let decoded = Xdc63Status::decode(&mut &encoded[..]).unwrap();
        assert_eq!(status, decoded);
    }

    #[test]
    fn test_hash_or_number_encoding() {
        // Test number
        let num = HashOrNumber::Number(12345);
        let mut encoded = Vec::new();
        num.encode(&mut encoded);
        let decoded = HashOrNumber::decode(&mut &encoded[..]).unwrap();
        assert_eq!(num, decoded);

        // Test hash
        let hash = HashOrNumber::Hash(B256::random());
        let mut encoded = Vec::new();
        hash.encode(&mut encoded);
        let decoded = HashOrNumber::decode(&mut &encoded[..]).unwrap();
        assert_eq!(hash, decoded);
    }
}
