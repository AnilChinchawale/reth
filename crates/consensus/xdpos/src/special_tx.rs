//! Special Transaction Handling for XDC Network
//!
//! This module implements XDC-specific transaction behavior:
//! - TIPSigning gas exemption (block 3,000,000+)
//! - EIP-158 disabled for state compatibility
//! - BigBalance handling for Apothem genesis accounts

use alloy_primitives::{address, Address, U256};

/// Block number where TIPSigning (free gas for system transactions) is enabled
pub const TIP_SIGNING_BLOCK: u64 = 3_000_000;

/// BlockSigners contract address (0x89) - receives signing transactions
pub const BLOCK_SIGNERS: Address = address!("0000000000000000000000000000000000000089");

/// Randomize contract address (0x90) - receives randomization transactions
pub const RANDOMIZE: Address = address!("0000000000000000000000000000000000000090");

/// Validator contract address (0x88) - master validator registry
pub const VALIDATOR: Address = address!("0000000000000000000000000000000000000088");

/// Check if a transaction should have zero gas cost (TIPSigning)
///
/// After block 3,000,000, transactions to BlockSigners (0x89) and Randomize (0x90)
/// contracts are exempt from gas costs. This is called "TIPSigning" in XDC.
///
/// # Arguments
/// * `block_number` - The block number where the transaction is included
/// * `to` - The recipient address of the transaction (None for contract creation)
///
/// # Returns
/// `true` if the transaction should have zero gas cost, `false` otherwise
///
/// # Example
/// ```
/// use alloy_primitives::address;
/// use reth_xdpos::special_tx::{is_free_gas_tx, BLOCK_SIGNERS};
///
/// let block_num = 3_000_001;
/// let to = Some(BLOCK_SIGNERS);
/// assert!(is_free_gas_tx(block_num, to));
/// ```
pub fn is_free_gas_tx(block_number: u64, to: Option<Address>) -> bool {
    if block_number < TIP_SIGNING_BLOCK {
        return false;
    }
    match to {
        Some(addr) if addr == BLOCK_SIGNERS || addr == RANDOMIZE => true,
        _ => false,
    }
}

/// System contracts that bypass normal balance validation
///
/// These contracts are part of the XDC consensus system and can execute
/// operations that would normally fail balance checks.
///
/// # Arguments
/// * `addr` - The address to check
///
/// # Returns
/// `true` if the address is a system contract
pub fn is_system_contract(addr: Address) -> bool {
    addr == VALIDATOR || addr == BLOCK_SIGNERS || addr == RANDOMIZE
}

/// XDC chains disable EIP-158 to prevent empty account cleanup
///
/// EIP-158 introduced empty account cleanup to reduce state bloat, but XDC
/// disables this to maintain state root compatibility with the original Go
/// implementation (XDC v2.6.8).
///
/// This is critical for consensus - enabling EIP-158 would cause state root
/// mismatches with other XDC nodes.
///
/// # Arguments
/// * `chain_id` - The chain ID to check
///
/// # Returns
/// `true` if EIP-158 should be disabled (XDC mainnet or Apothem)
pub fn is_eip158_disabled(chain_id: u64) -> bool {
    chain_id == 50 || chain_id == 51 // mainnet or apothem
}

/// Check if a balance exceeds normal range (Apothem genesis accounts)
///
/// Apothem testnet genesis includes accounts with balances set to 2^256 - 1
/// (U256::MAX). While Rust's U256 handles this correctly without overflow,
/// this helper identifies such accounts for special handling during RLP
/// encoding and state root calculation.
///
/// # Arguments
/// * `balance` - The account balance to check
///
/// # Returns
/// `true` if the balance exceeds u128::MAX (indicating a "big balance" account)
pub fn is_big_balance(balance: U256) -> bool {
    balance > U256::from(u128::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::address;

    #[test]
    fn test_is_free_gas_tx_before_fork() {
        // Before block 3,000,000, no transactions are free
        let block_num = 2_999_999;
        assert!(!is_free_gas_tx(block_num, Some(BLOCK_SIGNERS)));
        assert!(!is_free_gas_tx(block_num, Some(RANDOMIZE)));
        assert!(!is_free_gas_tx(block_num, Some(VALIDATOR)));
    }

    #[test]
    fn test_is_free_gas_tx_after_fork() {
        // After block 3,000,000, only BlockSigners and Randomize are free
        let block_num = 3_000_001;
        assert!(is_free_gas_tx(block_num, Some(BLOCK_SIGNERS)));
        assert!(is_free_gas_tx(block_num, Some(RANDOMIZE)));
        assert!(!is_free_gas_tx(block_num, Some(VALIDATOR)));
        assert!(!is_free_gas_tx(block_num, None)); // Contract creation
    }

    #[test]
    fn test_is_free_gas_tx_at_fork_boundary() {
        // Exactly at block 3,000,000, TIPSigning is enabled
        let block_num = TIP_SIGNING_BLOCK;
        assert!(is_free_gas_tx(block_num, Some(BLOCK_SIGNERS)));
        assert!(is_free_gas_tx(block_num, Some(RANDOMIZE)));
    }

    #[test]
    fn test_is_free_gas_tx_other_addresses() {
        let block_num = 3_000_001;
        let random_addr = address!("1234567890123456789012345678901234567890");
        assert!(!is_free_gas_tx(block_num, Some(random_addr)));
    }

    #[test]
    fn test_is_system_contract() {
        assert!(is_system_contract(VALIDATOR));
        assert!(is_system_contract(BLOCK_SIGNERS));
        assert!(is_system_contract(RANDOMIZE));
        
        let random_addr = address!("1234567890123456789012345678901234567890");
        assert!(!is_system_contract(random_addr));
    }

    #[test]
    fn test_is_eip158_disabled() {
        // XDC Mainnet (50) and Apothem (51) disable EIP-158
        assert!(is_eip158_disabled(50));
        assert!(is_eip158_disabled(51));
        
        // Other chains enable EIP-158
        assert!(!is_eip158_disabled(1)); // Ethereum mainnet
        assert!(!is_eip158_disabled(137)); // Polygon
    }

    #[test]
    fn test_is_big_balance() {
        // Normal balances
        assert!(!is_big_balance(U256::from(0)));
        assert!(!is_big_balance(U256::from(1_000_000_000_000_000_000u64))); // 1 ETH
        assert!(!is_big_balance(U256::from(u128::MAX)));
        
        // Big balances (Apothem genesis accounts)
        let big_balance = U256::from(u128::MAX) + U256::from(1);
        assert!(is_big_balance(big_balance));
        assert!(is_big_balance(U256::MAX));
    }
}
