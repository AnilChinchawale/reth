//! XDPoS V2 Block Proposer Selection
//!
//! This module implements the proposer selection algorithm for V2 BFT consensus.
//! The proposer is determined by: round_number % len(validators)

use super::Round;
use alloy_primitives::Address;
use crate::errors::{XDPoSError, XDPoSResult};

/// Select the block proposer for a given round
///
/// The proposer is selected using simple round-robin based on round number:
/// `proposer_index = round % validator_count`
///
/// # Arguments
/// * `round` - The consensus round number
/// * `validators` - List of validator addresses for this epoch
///
/// # Returns
/// The address of the selected proposer
pub fn select_proposer(round: Round, validators: &[Address]) -> XDPoSResult<Address> {
    if validators.is_empty() {
        return Err(XDPoSError::Custom("empty validator list".to_string()));
    }

    let index = (round as usize) % validators.len();
    Ok(validators[index])
}

/// Get the index of a validator in the validator set
///
/// # Arguments
/// * `validator` - The validator address to find
/// * `validators` - List of validator addresses
///
/// # Returns
/// The index of the validator, or None if not found
pub fn get_validator_index(validator: &Address, validators: &[Address]) -> Option<usize> {
    validators.iter().position(|v| v == validator)
}

/// Check if an address is a valid validator
///
/// # Arguments
/// * `address` - The address to check
/// * `validators` - List of validator addresses
///
/// # Returns
/// true if the address is in the validator set
pub fn is_validator(address: &Address, validators: &[Address]) -> bool {
    validators.contains(address)
}

/// Calculate which round a validator should propose in
///
/// Given a validator's index, calculate the next round they should propose
/// starting from a given round.
///
/// # Arguments
/// * `validator_index` - Index of the validator in the set
/// * `current_round` - Current round number
/// * `validator_count` - Total number of validators
///
/// # Returns
/// The next round number where this validator should propose
pub fn next_proposer_round(
    validator_index: usize,
    current_round: Round,
    validator_count: usize,
) -> XDPoSResult<Round> {
    if validator_count == 0 {
        return Err(XDPoSError::Custom("validator count is zero".to_string()));
    }
    if validator_index >= validator_count {
        return Err(XDPoSError::Custom(format!(
            "validator index {} out of bounds (count: {})",
            validator_index, validator_count
        )));
    }

    let current_proposer_idx = (current_round as usize) % validator_count;
    
    if validator_index >= current_proposer_idx {
        // Next turn in this cycle
        let rounds_until_turn = validator_index - current_proposer_idx;
        Ok(current_round + rounds_until_turn as u64)
    } else {
        // Next turn in next cycle
        let rounds_until_next_cycle = validator_count - current_proposer_idx;
        let rounds_into_next_cycle = validator_index;
        Ok(current_round + (rounds_until_next_cycle + rounds_into_next_cycle) as u64)
    }
}

/// Get the proposer for the parent block given current round
///
/// # Arguments
/// * `current_round` - Current round number
/// * `validators` - List of validator addresses
///
/// # Returns
/// The address of the parent block proposer (previous round)
pub fn parent_proposer(current_round: Round, validators: &[Address]) -> XDPoSResult<Address> {
    if current_round == 0 {
        return Err(XDPoSError::Custom("no parent for round 0".to_string()));
    }
    select_proposer(current_round - 1, validators)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_validators(count: usize) -> Vec<Address> {
        (0..count)
            .map(|i| Address::with_last_byte(i as u8))
            .collect()
    }

    #[test]
    fn test_select_proposer_round_robin() {
        let validators = make_validators(5);
        
        // Round 0 -> validator 0
        assert_eq!(select_proposer(0, &validators).unwrap(), validators[0]);
        
        // Round 1 -> validator 1
        assert_eq!(select_proposer(1, &validators).unwrap(), validators[1]);
        
        // Round 4 -> validator 4
        assert_eq!(select_proposer(4, &validators).unwrap(), validators[4]);
        
        // Round 5 -> wraps to validator 0
        assert_eq!(select_proposer(5, &validators).unwrap(), validators[0]);
        
        // Round 10 -> validator 0 (10 % 5 = 0)
        assert_eq!(select_proposer(10, &validators).unwrap(), validators[0]);
        
        // Round 13 -> validator 3 (13 % 5 = 3)
        assert_eq!(select_proposer(13, &validators).unwrap(), validators[3]);
    }

    #[test]
    fn test_select_proposer_single_validator() {
        let validators = make_validators(1);
        
        // All rounds should select the same validator
        assert_eq!(select_proposer(0, &validators).unwrap(), validators[0]);
        assert_eq!(select_proposer(1, &validators).unwrap(), validators[0]);
        assert_eq!(select_proposer(100, &validators).unwrap(), validators[0]);
    }

    #[test]
    fn test_select_proposer_empty_validators() {
        let validators: Vec<Address> = vec![];
        assert!(select_proposer(0, &validators).is_err());
    }

    #[test]
    fn test_get_validator_index() {
        let validators = make_validators(5);
        
        assert_eq!(get_validator_index(&validators[0], &validators), Some(0));
        assert_eq!(get_validator_index(&validators[3], &validators), Some(3));
        assert_eq!(get_validator_index(&validators[4], &validators), Some(4));
        
        let non_validator = Address::with_last_byte(99);
        assert_eq!(get_validator_index(&non_validator, &validators), None);
    }

    #[test]
    fn test_is_validator() {
        let validators = make_validators(5);
        
        assert!(is_validator(&validators[0], &validators));
        assert!(is_validator(&validators[4], &validators));
        
        let non_validator = Address::with_last_byte(99);
        assert!(!is_validator(&non_validator, &validators));
    }

    #[test]
    fn test_next_proposer_round() {
        let validator_count = 5;
        
        // Validator 0 at round 0 -> next round is 5
        assert_eq!(
            next_proposer_round(0, 0, validator_count).unwrap(),
            5
        );
        
        // Validator 2 at round 0 -> next round is 2
        assert_eq!(
            next_proposer_round(2, 0, validator_count).unwrap(),
            2
        );
        
        // Validator 1 at round 3 -> next round is 6 (3+3)
        // Current: round 3 (proposer idx 3)
        // Validator 1 needs to wait: (5-3) + 1 = 3 rounds
        assert_eq!(
            next_proposer_round(1, 3, validator_count).unwrap(),
            6
        );
        
        // Validator 4 at round 2 -> next round is 4 (2+2)
        assert_eq!(
            next_proposer_round(4, 2, validator_count).unwrap(),
            4
        );
    }

    #[test]
    fn test_next_proposer_round_same_validator() {
        let validator_count = 5;
        
        // Current round 7 (7 % 5 = 2), asking for validator 2
        // Validator 2 is the current proposer, next turn is in 5 rounds
        assert_eq!(
            next_proposer_round(2, 7, validator_count).unwrap(),
            7 // Same round!
        );
    }

    #[test]
    fn test_next_proposer_round_errors() {
        // Zero validator count
        assert!(next_proposer_round(0, 0, 0).is_err());
        
        // Index out of bounds
        assert!(next_proposer_round(5, 0, 3).is_err());
    }

    #[test]
    fn test_parent_proposer() {
        let validators = make_validators(5);
        
        // Round 1 parent (round 0) -> validator 0
        assert_eq!(parent_proposer(1, &validators).unwrap(), validators[0]);
        
        // Round 5 parent (round 4) -> validator 4
        assert_eq!(parent_proposer(5, &validators).unwrap(), validators[4]);
        
        // Round 6 parent (round 5) -> validator 0 (5 % 5 = 0)
        assert_eq!(parent_proposer(6, &validators).unwrap(), validators[0]);
    }

    #[test]
    fn test_parent_proposer_round_zero() {
        let validators = make_validators(5);
        assert!(parent_proposer(0, &validators).is_err());
    }

    #[test]
    fn test_proposer_pattern_18_validators() {
        // Real-world test with 18 validators (common XDC masternode count)
        let validators = make_validators(18);
        
        for round in 0..36 {
            let expected_idx = (round % 18) as usize;
            let proposer = select_proposer(round, &validators).unwrap();
            assert_eq!(proposer, validators[expected_idx]);
        }
    }

    #[test]
    fn test_round_continuity() {
        // Ensure proposer selection is continuous across rounds
        let validators = make_validators(7);
        let mut previous_proposer = None;
        
        for round in 0..21 {
            let proposer = select_proposer(round, &validators).unwrap();
            
            // Check that proposer changes between rounds (unless wrapping)
            if let Some(prev) = previous_proposer {
                if round % 7 != 0 {
                    assert_ne!(proposer, prev);
                }
            }
            
            previous_proposer = Some(proposer);
        }
    }
}
