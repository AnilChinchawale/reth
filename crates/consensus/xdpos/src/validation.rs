//! XDPoS validation utilities

use crate::{
    config::XDPoSConfig,
    errors::{XDPoSError, XDPoSResult},
};
use alloy_consensus::Header;

/// Validate header extra data
pub fn validate_extra_data(
    header: &Header,
    _config: &XDPoSConfig,
) -> XDPoSResult<()> {
    let extra = &header.extra_data;

    // Check minimum length (vanity + seal)
    if extra.len() < 97 {
        return Err(XDPoSError::ExtraDataTooShort);
    }

    // Check maximum length (prevent spam)
    if extra.len() > 32 * 1024 {
        return Err(XDPoSError::InvalidExtraData);
    }

    Ok(())
}

/// Validate block difficulty
pub fn validate_difficulty(
    header: &Header,
    _config: &XDPoSConfig,
    _is_inturn: bool,
) -> XDPoSResult<()> {
    // XDPoS uses difficulty to indicate turn
    // In-turn: 2, Out-of-turn: 1
    let _expected_difficulty = if _is_inturn { 2 } else { 1 };

    // TODO: Validate difficulty matches expected
    let _difficulty = header.difficulty;

    Ok(())
}
