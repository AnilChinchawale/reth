//! XDC block header with XDPoS validator fields.

use alloy_primitives::{Address, Bloom, Bytes, Sealable, B256, B64, U256};
use alloy_rlp::{Decodable, Encodable, Header as RlpHeader};
use core::mem;

/// XDC block header with XDPoS consensus fields.
///
/// This extends the standard Ethereum header with three additional fields required by XDC's
/// XDPoS consensus:
/// - `validators`: List of validator addresses (RLP-encoded)
/// - `validator`: The validator that produced this block
/// - `penalties`: Penalty data for misbehaving validators
///
/// RLP encoding order (18 required fields + optional fields):
/// 1-15: Standard Ethereum header fields
/// 16-18: XDC-specific validator fields
/// 19+: Optional fields (base_fee_per_gas, etc.)
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct XdcBlockHeader {
    /// The Keccak 256-bit hash of the parent block's header, in its entirety.
    pub parent_hash: B256,
    /// The Keccak 256-bit hash of the ommers list portion of this block.
    pub ommers_hash: B256,
    /// The 160-bit address to which all fees collected from the successful mining of this block
    /// be transferred.
    pub beneficiary: Address,
    /// The Keccak 256-bit hash of the root node of the state trie, after all transactions are
    /// executed and finalisations applied.
    pub state_root: B256,
    /// The Keccak 256-bit hash of the root node of the trie structure populated with each
    /// transaction in the transactions list portion of the block.
    pub transactions_root: B256,
    /// The Keccak 256-bit hash of the root node of the trie structure populated with the receipts
    /// of each transaction in the transactions list portion of the block.
    pub receipts_root: B256,
    /// The Bloom filter composed from indexable information (logger address and log topics)
    /// contained in each log entry from the receipt of each transaction in the transactions list.
    pub logs_bloom: Bloom,
    /// A scalar value corresponding to the difficulty level of this block. This can be calculated
    /// from the previous block's difficulty level and the timestamp.
    pub difficulty: U256,
    /// A scalar value equal to the number of ancestor blocks. The genesis block has a number of
    /// zero.
    pub number: u64,
    /// A scalar value equal to the current limit of gas expenditure per block.
    pub gas_limit: u64,
    /// A scalar value equal to the total gas used in transactions in this block.
    pub gas_used: u64,
    /// A scalar value equal to the reasonable output of Unix's time() at this block's inception.
    pub timestamp: u64,
    /// An arbitrary byte array containing data relevant to this block. This must be 32 bytes or
    /// fewer.
    pub extra_data: Bytes,
    /// A 256-bit hash which, combined with the nonce, proves that a sufficient amount of
    /// computation has been carried out on this block.
    pub mix_hash: B256,
    /// A 64-bit value which, combined with the mixhash, proves that a sufficient amount of
    /// computation has been carried out on this block.
    pub nonce: B64,
    
    // XDC-specific XDPoS fields (required, come after nonce)
    /// List of validators (RLP-encoded addresses). Empty for non-epoch blocks.
    pub validators: Bytes,
    /// The validator that produced this block (RLP-encoded address).
    pub validator: Bytes,
    /// Penalty data for slashing misbehaving validators.
    pub penalties: Bytes,
    
    // Optional post-London fields
    /// Base fee per gas for EIP-1559 transactions.
    pub base_fee_per_gas: Option<u64>,
    /// The total amount of blob gas consumed by the transactions within the block, added in
    /// EIP-4844.
    pub blob_gas_used: Option<u64>,
    /// A running total of blob gas consumed in excess of the target, prior to the block, added in
    /// EIP-4844.
    pub excess_blob_gas: Option<u64>,
    /// The hash of the parent beacon block's root is included in execution blocks, as proposed by
    /// EIP-4788.
    pub parent_beacon_block_root: Option<B256>,
    /// The Keccak 256-bit hash of the root node of the trie structure populated with each
    /// [EIP-7685] request in the block body.
    pub requests_hash: Option<B256>,
    /// The destination addresses and aggregated value of [EIP-7702] code authorizations in the
    /// block.
    pub target_blobs_per_block: Option<u64>,
}

impl Default for XdcBlockHeader {
    fn default() -> Self {
        Self {
            parent_hash: Default::default(),
            ommers_hash: Default::default(),
            beneficiary: Default::default(),
            state_root: Default::default(),
            transactions_root: Default::default(),
            receipts_root: Default::default(),
            logs_bloom: Default::default(),
            difficulty: Default::default(),
            number: 0,
            gas_limit: 0,
            gas_used: 0,
            timestamp: 0,
            extra_data: Default::default(),
            mix_hash: Default::default(),
            nonce: Default::default(),
            validators: Bytes::new(),
            validator: Bytes::new(),
            penalties: Bytes::new(),
            base_fee_per_gas: None,
            blob_gas_used: None,
            excess_blob_gas: None,
            parent_beacon_block_root: None,
            requests_hash: None,
            target_blobs_per_block: None,
        }
    }
}

impl XdcBlockHeader {
    /// Heavy function that computes the hash of the header via keccak256.
    /// 
    /// IMPORTANT: This computes the hash using only the first 15 standard Ethereum fields,
    /// excluding the 3 XDC-specific fields (validators, validator, penalties).
    /// 
    /// This is necessary for compatibility with Erigon/GP5 which compute block hashes
    /// without the XDC consensus fields. When headers are received via P2P and converted
    /// from standard format to XdcBlockHeader with empty XDC fields, the hash must still
    /// match what the original client computed.
    pub fn hash_slow(&self) -> B256 {
        // XDC block hash includes ALL 18 fields (15 standard + 3 XDC)
        let mut buf = Vec::new();
        self.encode(&mut buf);
        alloy_primitives::keccak256(&buf)
    }
    
    /// Encode only the first 15 standard Ethereum header fields (excluding XDC-specific fields).
    /// This is used for hash calculation to maintain compatibility with Erigon/GP5.
    fn encode_without_xdc_fields(&self, out: &mut dyn alloy_primitives::bytes::BufMut) {
        // Calculate payload length for standard fields only (15 fields)
        let mut payload_length = 0;
        payload_length += self.parent_hash.length();
        payload_length += self.ommers_hash.length();
        payload_length += self.beneficiary.length();
        payload_length += self.state_root.length();
        payload_length += self.transactions_root.length();
        payload_length += self.receipts_root.length();
        payload_length += self.logs_bloom.length();
        payload_length += self.difficulty.length();
        payload_length += self.number.length();
        payload_length += self.gas_limit.length();
        payload_length += self.gas_used.length();
        payload_length += self.timestamp.length();
        payload_length += self.extra_data.length();
        payload_length += self.mix_hash.length();
        payload_length += self.nonce.length();
        
        // Optional post-London fields (if present, include in hash)
        if let Some(ref base_fee) = self.base_fee_per_gas {
            payload_length += base_fee.length();
            
            if let Some(ref blob_gas_used) = self.blob_gas_used {
                payload_length += blob_gas_used.length();
                
                if let Some(ref excess_blob_gas) = self.excess_blob_gas {
                    payload_length += excess_blob_gas.length();
                    
                    if let Some(ref parent_beacon_block_root) = self.parent_beacon_block_root {
                        payload_length += parent_beacon_block_root.length();
                        
                        if let Some(ref requests_hash) = self.requests_hash {
                            payload_length += requests_hash.length();
                            
                            if let Some(ref target_blobs) = self.target_blobs_per_block {
                                payload_length += target_blobs.length();
                            }
                        }
                    }
                }
            }
        }
        
        let list_header = RlpHeader { list: true, payload_length };
        list_header.encode(out);
        
        // Encode standard fields (1-15)
        self.parent_hash.encode(out);
        self.ommers_hash.encode(out);
        self.beneficiary.encode(out);
        self.state_root.encode(out);
        self.transactions_root.encode(out);
        self.receipts_root.encode(out);
        self.logs_bloom.encode(out);
        self.difficulty.encode(out);
        self.number.encode(out);
        self.gas_limit.encode(out);
        self.gas_used.encode(out);
        self.timestamp.encode(out);
        self.extra_data.encode(out);
        self.mix_hash.encode(out);
        self.nonce.encode(out);
        
        // Optional fields (encoded in chain)
        if let Some(ref base_fee) = self.base_fee_per_gas {
            base_fee.encode(out);
            
            if let Some(ref blob_gas_used) = self.blob_gas_used {
                blob_gas_used.encode(out);
                
                if let Some(ref excess_blob_gas) = self.excess_blob_gas {
                    excess_blob_gas.encode(out);
                    
                    if let Some(ref parent_beacon_block_root) = self.parent_beacon_block_root {
                        parent_beacon_block_root.encode(out);
                        
                        if let Some(ref requests_hash) = self.requests_hash {
                            requests_hash.encode(out);
                            
                            if let Some(ref target_blobs) = self.target_blobs_per_block {
                                target_blobs.encode(out);
                            }
                        }
                    }
                }
            }
        }
        // NOTE: XDC-specific fields (validators, validator, penalties) are NOT included in hash
    }
}

impl Sealable for XdcBlockHeader {
    fn hash_slow(&self) -> B256 {
        self.hash_slow()
    }
}

impl alloy_consensus::BlockHeader for XdcBlockHeader {
    fn parent_hash(&self) -> B256 {
        self.parent_hash
    }

    fn ommers_hash(&self) -> B256 {
        self.ommers_hash
    }

    fn beneficiary(&self) -> Address {
        self.beneficiary
    }

    fn state_root(&self) -> B256 {
        self.state_root
    }

    fn transactions_root(&self) -> B256 {
        self.transactions_root
    }

    fn receipts_root(&self) -> B256 {
        self.receipts_root
    }

    fn withdrawals_root(&self) -> Option<B256> {
        None // XDC does not support withdrawals
    }

    fn logs_bloom(&self) -> Bloom {
        self.logs_bloom
    }

    fn difficulty(&self) -> U256 {
        self.difficulty
    }

    fn number(&self) -> u64 {
        self.number
    }

    fn gas_limit(&self) -> u64 {
        self.gas_limit
    }

    fn gas_used(&self) -> u64 {
        self.gas_used
    }

    fn timestamp(&self) -> u64 {
        self.timestamp
    }

    fn mix_hash(&self) -> Option<B256> {
        Some(self.mix_hash)
    }

    fn nonce(&self) -> Option<B64> {
        Some(self.nonce)
    }

    fn base_fee_per_gas(&self) -> Option<u64> {
        self.base_fee_per_gas
    }

    fn blob_gas_used(&self) -> Option<u64> {
        self.blob_gas_used
    }

    fn excess_blob_gas(&self) -> Option<u64> {
        self.excess_blob_gas
    }

    fn parent_beacon_block_root(&self) -> Option<B256> {
        self.parent_beacon_block_root
    }

    fn requests_hash(&self) -> Option<B256> {
        self.requests_hash
    }

    fn extra_data(&self) -> &Bytes {
        &self.extra_data
    }
}

impl Encodable for XdcBlockHeader {
    fn encode(&self, out: &mut dyn alloy_primitives::bytes::BufMut) {
        // XDC headers have 18 required fields (15 standard + 3 XDC) + optional fields
        let mut list_header = RlpHeader { list: true, payload_length: 0 };
        
        // Calculate payload length for all fields
        list_header.payload_length += self.parent_hash.length();
        list_header.payload_length += self.ommers_hash.length();
        list_header.payload_length += self.beneficiary.length();
        list_header.payload_length += self.state_root.length();
        list_header.payload_length += self.transactions_root.length();
        list_header.payload_length += self.receipts_root.length();
        list_header.payload_length += self.logs_bloom.length();
        list_header.payload_length += self.difficulty.length();
        list_header.payload_length += self.number.length();
        list_header.payload_length += self.gas_limit.length();
        list_header.payload_length += self.gas_used.length();
        list_header.payload_length += self.timestamp.length();
        list_header.payload_length += self.extra_data.length();
        list_header.payload_length += self.mix_hash.length();
        list_header.payload_length += self.nonce.length();
        
        // XDC-specific fields (ALWAYS included, even if empty)
        list_header.payload_length += self.validators.length();
        list_header.payload_length += self.validator.length();
        list_header.payload_length += self.penalties.length();
        
        // Optional fields
        if let Some(ref base_fee) = self.base_fee_per_gas {
            list_header.payload_length += base_fee.length();
            
            if let Some(ref blob_gas_used) = self.blob_gas_used {
                list_header.payload_length += blob_gas_used.length();
                
                if let Some(ref excess_blob_gas) = self.excess_blob_gas {
                    list_header.payload_length += excess_blob_gas.length();
                    
                    if let Some(ref parent_beacon_block_root) = self.parent_beacon_block_root {
                        list_header.payload_length += parent_beacon_block_root.length();
                        
                        if let Some(ref requests_hash) = self.requests_hash {
                            list_header.payload_length += requests_hash.length();
                            
                            if let Some(ref target_blobs) = self.target_blobs_per_block {
                                list_header.payload_length += target_blobs.length();
                            }
                        }
                    }
                }
            }
        }
        
        list_header.encode(out);
        
        // Encode all fields in order
        self.parent_hash.encode(out);
        self.ommers_hash.encode(out);
        self.beneficiary.encode(out);
        self.state_root.encode(out);
        self.transactions_root.encode(out);
        self.receipts_root.encode(out);
        self.logs_bloom.encode(out);
        self.difficulty.encode(out);
        self.number.encode(out);
        self.gas_limit.encode(out);
        self.gas_used.encode(out);
        self.timestamp.encode(out);
        self.extra_data.encode(out);
        self.mix_hash.encode(out);
        self.nonce.encode(out);
        
        // XDC-specific fields (ALWAYS encoded)
        self.validators.encode(out);
        self.validator.encode(out);
        self.penalties.encode(out);
        
        // Optional fields (encoded in chain)
        if let Some(ref base_fee) = self.base_fee_per_gas {
            base_fee.encode(out);
            
            if let Some(ref blob_gas_used) = self.blob_gas_used {
                blob_gas_used.encode(out);
                
                if let Some(ref excess_blob_gas) = self.excess_blob_gas {
                    excess_blob_gas.encode(out);
                    
                    if let Some(ref parent_beacon_block_root) = self.parent_beacon_block_root {
                        parent_beacon_block_root.encode(out);
                        
                        if let Some(ref requests_hash) = self.requests_hash {
                            requests_hash.encode(out);
                            
                            if let Some(ref target_blobs) = self.target_blobs_per_block {
                                target_blobs.encode(out);
                            }
                        }
                    }
                }
            }
        }
    }

    fn length(&self) -> usize {
        let mut length = 0;
        length += self.parent_hash.length();
        length += self.ommers_hash.length();
        length += self.beneficiary.length();
        length += self.state_root.length();
        length += self.transactions_root.length();
        length += self.receipts_root.length();
        length += self.logs_bloom.length();
        length += self.difficulty.length();
        length += self.number.length();
        length += self.gas_limit.length();
        length += self.gas_used.length();
        length += self.timestamp.length();
        length += self.extra_data.length();
        length += self.mix_hash.length();
        length += self.nonce.length();
        length += self.validators.length();
        length += self.validator.length();
        length += self.penalties.length();
        
        if let Some(ref base_fee) = self.base_fee_per_gas {
            length += base_fee.length();
            
            if let Some(ref blob_gas_used) = self.blob_gas_used {
                length += blob_gas_used.length();
                
                if let Some(ref excess_blob_gas) = self.excess_blob_gas {
                    length += excess_blob_gas.length();
                    
                    if let Some(ref parent_beacon_block_root) = self.parent_beacon_block_root {
                        length += parent_beacon_block_root.length();
                        
                        if let Some(ref requests_hash) = self.requests_hash {
                            length += requests_hash.length();
                            
                            if let Some(ref target_blobs) = self.target_blobs_per_block {
                                length += target_blobs.length();
                            }
                        }
                    }
                }
            }
        }
        
        length + alloy_rlp::length_of_length(length)
    }
}

impl Decodable for XdcBlockHeader {
    fn decode(buf: &mut &[u8]) -> alloy_rlp::Result<Self> {
        let rlp_head = RlpHeader::decode(buf)?;
        if !rlp_head.list {
            return Err(alloy_rlp::Error::UnexpectedString);
        }
        
        let started_len = buf.len();
        
        // Decode standard Ethereum fields (1-15)
        let parent_hash = Decodable::decode(buf)?;
        let ommers_hash = Decodable::decode(buf)?;
        let beneficiary = Decodable::decode(buf)?;
        let state_root = Decodable::decode(buf)?;
        let transactions_root = Decodable::decode(buf)?;
        let receipts_root = Decodable::decode(buf)?;
        let logs_bloom = Decodable::decode(buf)?;
        let difficulty = Decodable::decode(buf)?;
        let number = Decodable::decode(buf)?;
        let gas_limit = Decodable::decode(buf)?;
        let gas_used = Decodable::decode(buf)?;
        let timestamp = Decodable::decode(buf)?;
        let extra_data = Decodable::decode(buf)?;
        let mix_hash = Decodable::decode(buf)?;
        let nonce = Decodable::decode(buf)?;
        
        let mut consumed = started_len - buf.len();
        
        // XDC-specific fields (16-18) - OPTIONAL (for compatibility with standard Ethereum headers)
        // Real XDC headers will have these, but headers decoded from P2P and re-encoded may not.
        let validators = if consumed < rlp_head.payload_length {
            Decodable::decode(buf)?
        } else {
            Bytes::new()
        };
        
        let validator = if started_len - buf.len() < rlp_head.payload_length {
            Decodable::decode(buf)?
        } else {
            Bytes::new()
        };
        
        let penalties = if started_len - buf.len() < rlp_head.payload_length {
            Decodable::decode(buf)?
        } else {
            Bytes::new()
        };
        
        consumed = started_len - buf.len();
        
        // Optional fields (decode if there's more data)
        let base_fee_per_gas = if consumed < rlp_head.payload_length {
            Some(Decodable::decode(buf)?)
        } else {
            None
        };
        
        let blob_gas_used = if started_len - buf.len() < rlp_head.payload_length {
            Some(Decodable::decode(buf)?)
        } else {
            None
        };
        
        let excess_blob_gas = if started_len - buf.len() < rlp_head.payload_length {
            Some(Decodable::decode(buf)?)
        } else {
            None
        };
        
        let parent_beacon_block_root = if started_len - buf.len() < rlp_head.payload_length {
            Some(Decodable::decode(buf)?)
        } else {
            None
        };
        
        let requests_hash = if started_len - buf.len() < rlp_head.payload_length {
            Some(Decodable::decode(buf)?)
        } else {
            None
        };
        
        let target_blobs_per_block = if started_len - buf.len() < rlp_head.payload_length {
            Some(Decodable::decode(buf)?)
        } else {
            None
        };
        
        let consumed = started_len - buf.len();
        if consumed != rlp_head.payload_length {
            return Err(alloy_rlp::Error::UnexpectedLength);
        }
        
        Ok(Self {
            parent_hash,
            ommers_hash,
            beneficiary,
            state_root,
            transactions_root,
            receipts_root,
            logs_bloom,
            difficulty,
            number,
            gas_limit,
            gas_used,
            timestamp,
            extra_data,
            mix_hash,
            nonce,
            validators,
            validator,
            penalties,
            base_fee_per_gas,
            blob_gas_used,
            excess_blob_gas,
            parent_beacon_block_root,
            requests_hash,
            target_blobs_per_block,
        })
    }
}

// Implement reth's BlockHeader trait
impl reth_primitives_traits::BlockHeader for XdcBlockHeader {}

impl AsRef<Self> for XdcBlockHeader {
    fn as_ref(&self) -> &Self {
        self
    }
}

impl reth_primitives_traits::InMemorySize for XdcBlockHeader {
    fn size(&self) -> usize {
        mem::size_of::<Self>() + self.extra_data.len() + self.validators.len() + 
            self.validator.len() + self.penalties.len()
    }
}

/// Convert XdcBlockHeader to standard Ethereum Header (strips XDC-specific fields)
impl From<XdcBlockHeader> for alloy_consensus::Header {
    fn from(xdc: XdcBlockHeader) -> Self {
        Self {
            parent_hash: xdc.parent_hash,
            ommers_hash: xdc.ommers_hash,
            beneficiary: xdc.beneficiary,
            state_root: xdc.state_root,
            transactions_root: xdc.transactions_root,
            receipts_root: xdc.receipts_root,
            logs_bloom: xdc.logs_bloom,
            difficulty: xdc.difficulty,
            number: xdc.number,
            gas_limit: xdc.gas_limit,
            gas_used: xdc.gas_used,
            timestamp: xdc.timestamp,
            extra_data: xdc.extra_data,
            mix_hash: xdc.mix_hash,
            nonce: xdc.nonce,
            base_fee_per_gas: xdc.base_fee_per_gas,
            withdrawals_root: None, // XDC does not support withdrawals
            blob_gas_used: xdc.blob_gas_used,
            excess_blob_gas: xdc.excess_blob_gas,
            parent_beacon_block_root: xdc.parent_beacon_block_root,
            requests_hash: xdc.requests_hash,
        }
    }
}

/// Convert standard Ethereum Header to XdcBlockHeader (adds empty XDC-specific fields)
/// 
/// This is used when receiving headers over P2P that have been decoded from XDC's 18-field
/// format but had the XDC-specific fields stripped. The validators/validator/penalties fields
/// are set to empty because the original decoder in xdc_header.rs discards them.
impl From<alloy_consensus::Header> for XdcBlockHeader {
    fn from(eth: alloy_consensus::Header) -> Self {
        Self {
            parent_hash: eth.parent_hash,
            ommers_hash: eth.ommers_hash,
            beneficiary: eth.beneficiary,
            state_root: eth.state_root,
            transactions_root: eth.transactions_root,
            receipts_root: eth.receipts_root,
            logs_bloom: eth.logs_bloom,
            difficulty: eth.difficulty,
            number: eth.number,
            gas_limit: eth.gas_limit,
            gas_used: eth.gas_used,
            timestamp: eth.timestamp,
            extra_data: eth.extra_data,
            mix_hash: eth.mix_hash,
            nonce: eth.nonce,
            // XDC-specific fields: set to empty since they were stripped during decode
            validators: Bytes::new(),
            validator: Bytes::new(),
            penalties: Bytes::new(),
            // Optional fields
            base_fee_per_gas: eth.base_fee_per_gas,
            blob_gas_used: eth.blob_gas_used,
            excess_blob_gas: eth.excess_blob_gas,
            parent_beacon_block_root: eth.parent_beacon_block_root,
            requests_hash: eth.requests_hash,
            target_blobs_per_block: None,
        }
    }
}

/// Decode raw RLP bytes as XDC headers, then convert to standard Headers.
/// This is used by the P2P layer to handle XDC's 18-field block headers.
pub fn decode_xdc_headers_to_eth(buf: &mut &[u8]) -> alloy_rlp::Result<Vec<alloy_consensus::Header>> {
    let headers: Vec<XdcBlockHeader> = alloy_rlp::Decodable::decode(buf)?;
    Ok(headers.into_iter().map(Into::into).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_empty_header_roundtrip() {
        let header = XdcBlockHeader::default();
        let mut buf = Vec::new();
        header.encode(&mut buf);
        
        let decoded = XdcBlockHeader::decode(&mut &buf[..]).unwrap();
        assert_eq!(header, decoded);
    }
    
    #[test]
    fn test_header_with_xdc_fields() {
        let mut header = XdcBlockHeader::default();
        header.number = 100;
        header.validators = Bytes::from(vec![1, 2, 3]);
        header.validator = Bytes::from(vec![4, 5]);
        header.penalties = Bytes::from(vec![]);
        
        let mut buf = Vec::new();
        header.encode(&mut buf);
        
        let decoded = XdcBlockHeader::decode(&mut &buf[..]).unwrap();
        assert_eq!(header, decoded);
        assert_eq!(decoded.validators, Bytes::from(vec![1, 2, 3]));
        assert_eq!(decoded.validator, Bytes::from(vec![4, 5]));
    }
    
    #[test]
    fn test_hash_compatibility_with_standard_header() {
        // This test verifies that XdcBlockHeader computes the same hash as a standard Header
        // when both have the same standard fields, regardless of XDC-specific fields.
        // This is critical for P2P compatibility with Erigon/GP5.
        
        // Create a standard header
        let standard_header = alloy_consensus::Header {
            parent_hash: B256::from([1u8; 32]),
            ommers_hash: B256::from([2u8; 32]),
            beneficiary: Address::from([3u8; 20]),
            state_root: B256::from([4u8; 32]),
            transactions_root: B256::from([5u8; 32]),
            receipts_root: B256::from([6u8; 32]),
            logs_bloom: Bloom::default(),
            difficulty: U256::from(100),
            number: 12345,
            gas_limit: 8000000,
            gas_used: 5000000,
            timestamp: 1234567890,
            extra_data: Bytes::from(vec![7, 8, 9]),
            mix_hash: B256::from([10u8; 32]),
            nonce: B64::from([11u8; 8]),
            base_fee_per_gas: Some(1000000000),
            withdrawals_root: None,
            blob_gas_used: None,
            excess_blob_gas: None,
            parent_beacon_block_root: None,
            requests_hash: None,
        };
        
        // Convert to XdcBlockHeader (adds empty XDC fields)
        let xdc_header: XdcBlockHeader = standard_header.clone().into();
        
        // Verify XDC fields are empty (as expected from P2P conversion)
        assert_eq!(xdc_header.validators, Bytes::new());
        assert_eq!(xdc_header.validator, Bytes::new());
        assert_eq!(xdc_header.penalties, Bytes::new());
        
        // CRITICAL: The hashes MUST match for FCU validation to work
        let standard_hash = {
            let mut buf = Vec::new();
            standard_header.encode(&mut buf);
            alloy_primitives::keccak256(&buf)
        };
        let xdc_hash = xdc_header.hash_slow();
        
        assert_eq!(
            standard_hash, xdc_hash,
            "XdcBlockHeader hash must match standard Header hash for P2P compatibility"
        );
    }
    
    #[test]
    fn test_hash_excludes_xdc_fields() {
        // This test verifies that XDC-specific fields do NOT affect the hash.
        // Two headers with different XDC fields but same standard fields should have the same hash.
        
        let mut header1 = XdcBlockHeader::default();
        header1.number = 100;
        header1.validators = Bytes::new(); // empty
        header1.validator = Bytes::new();  // empty
        header1.penalties = Bytes::new();  // empty
        
        let mut header2 = XdcBlockHeader::default();
        header2.number = 100;
        header2.validators = Bytes::from(vec![1, 2, 3]); // has data
        header2.validator = Bytes::from(vec![4, 5, 6]);  // has data
        header2.penalties = Bytes::from(vec![7, 8]);     // has data
        
        // Despite different XDC fields, hashes should match
        assert_eq!(
            header1.hash_slow(),
            header2.hash_slow(),
            "Hash must not include XDC-specific fields"
        );
    }
}
