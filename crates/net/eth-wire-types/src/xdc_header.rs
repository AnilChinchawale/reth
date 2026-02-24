//! Minimal XDC block header decoder for P2P message handling.
//!
//! XDC block headers have 3 extra fields after nonce: validators, validator, penalties.
//! This module decodes them and converts to standard Ethereum headers.

use alloy_consensus::Header;
use alloy_primitives::{Address, Bloom, Bytes, B256, B64, U256};
use alloy_rlp::Decodable;

/// Decode XDC block headers from RLP and convert to standard Ethereum headers.
///
/// XDC headers have 18 required fields (15 standard + 3 XDPoS) plus optional post-EIP-1559 fields.
/// This function decodes the full XDC format and strips the 3 extra fields.
pub fn decode_xdc_block_headers(buf: &mut &[u8]) -> alloy_rlp::Result<Vec<Header>> {
    // Decode outer list (list of headers)
    let list_header = alloy_rlp::Header::decode(buf)?;
    if !list_header.list {
        return Err(alloy_rlp::Error::UnexpectedString);
    }

    let started_len = buf.len();
    let mut headers = Vec::new();

    while started_len - buf.len() < list_header.payload_length {
        headers.push(decode_single_xdc_header(buf)?);
    }

    Ok(headers)
}

/// Decode a single XDC block header from RLP.
fn decode_single_xdc_header(buf: &mut &[u8]) -> alloy_rlp::Result<Header> {
    let rlp_head = alloy_rlp::Header::decode(buf)?;
    if !rlp_head.list {
        return Err(alloy_rlp::Error::UnexpectedString);
    }
    let started_len = buf.len();

    // 15 standard Ethereum header fields
    let parent_hash = B256::decode(buf)?;
    let ommers_hash = B256::decode(buf)?;
    let beneficiary = Address::decode(buf)?;
    let state_root = B256::decode(buf)?;
    let transactions_root = B256::decode(buf)?;
    let receipts_root = B256::decode(buf)?;
    let logs_bloom = Bloom::decode(buf)?;
    let difficulty = U256::decode(buf)?;
    let number = u64::decode(buf)?;
    let gas_limit = u64::decode(buf)?;
    let gas_used = u64::decode(buf)?;
    let timestamp = u64::decode(buf)?;
    let extra_data = Bytes::decode(buf)?;
    let mix_hash = B256::decode(buf)?;
    let nonce = B64::decode(buf)?;

    // 3 XDC-specific fields (decode and discard)
    let _validators = Bytes::decode(buf)?;
    let _validator = Bytes::decode(buf)?;
    let _penalties = Bytes::decode(buf)?;

    // Optional post-EIP-1559 fields
    let mut base_fee_per_gas = None;
    let mut withdrawals_root = None;
    let mut blob_gas_used = None;
    let mut excess_blob_gas = None;
    let mut parent_beacon_block_root = None;
    let mut requests_hash = None;

    if started_len - buf.len() < rlp_head.payload_length {
        base_fee_per_gas = Some(u64::decode(buf)?);
    }
    if started_len - buf.len() < rlp_head.payload_length {
        withdrawals_root = Some(B256::decode(buf)?);
    }
    if started_len - buf.len() < rlp_head.payload_length {
        blob_gas_used = Some(u64::decode(buf)?);
    }
    if started_len - buf.len() < rlp_head.payload_length {
        excess_blob_gas = Some(u64::decode(buf)?);
    }
    if started_len - buf.len() < rlp_head.payload_length {
        parent_beacon_block_root = Some(B256::decode(buf)?);
    }
    if started_len - buf.len() < rlp_head.payload_length {
        requests_hash = Some(B256::decode(buf)?);
    }

    // Skip any remaining unknown fields
    let consumed = started_len - buf.len();
    if consumed < rlp_head.payload_length {
        let remaining = rlp_head.payload_length - consumed;
        if buf.len() < remaining {
            return Err(alloy_rlp::Error::InputTooShort);
        }
        *buf = &buf[remaining..];
    }

    Ok(Header {
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
        base_fee_per_gas,
        withdrawals_root,
        blob_gas_used,
        excess_blob_gas,
        parent_beacon_block_root,
        requests_hash,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_rlp::Encodable;

    #[test]
    fn test_decode_xdc_header_roundtrip() {
        // Encode a minimal XDC header (18 fields)
        let mut buf = Vec::new();
        
        // Outer list (1 header)
        let mut header_buf = Vec::new();
        encode_test_xdc_header(&mut header_buf);
        
        let outer = alloy_rlp::Header { list: true, payload_length: header_buf.len() };
        outer.encode(&mut buf);
        buf.extend_from_slice(&header_buf);
        
        let headers = decode_xdc_block_headers(&mut &buf[..]).unwrap();
        assert_eq!(headers.len(), 1);
        assert_eq!(headers[0].number, 42);
    }

    fn encode_test_xdc_header(buf: &mut Vec<u8>) {
        // Build inner fields
        let mut fields = Vec::new();
        B256::ZERO.encode(&mut fields); // parent_hash
        B256::ZERO.encode(&mut fields); // ommers_hash
        Address::ZERO.encode(&mut fields); // beneficiary
        B256::ZERO.encode(&mut fields); // state_root
        B256::ZERO.encode(&mut fields); // transactions_root
        B256::ZERO.encode(&mut fields); // receipts_root
        Bloom::ZERO.encode(&mut fields); // logs_bloom
        U256::ZERO.encode(&mut fields); // difficulty
        42u64.encode(&mut fields); // number
        0u64.encode(&mut fields); // gas_limit
        0u64.encode(&mut fields); // gas_used
        0u64.encode(&mut fields); // timestamp
        Bytes::new().encode(&mut fields); // extra_data
        B256::ZERO.encode(&mut fields); // mix_hash
        B64::ZERO.encode(&mut fields); // nonce
        Bytes::new().encode(&mut fields); // validators (XDC)
        Bytes::new().encode(&mut fields); // validator (XDC)
        Bytes::new().encode(&mut fields); // penalties (XDC)
        
        let header = alloy_rlp::Header { list: true, payload_length: fields.len() };
        header.encode(buf);
        buf.extend_from_slice(&fields);
    }
}
