//! Unit tests for XDPoS consensus implementation

pub mod helpers;
mod sync_tests;
mod v1_tests;
mod v2_tests;
pub mod vectors;

// Re-export commonly used test utilities
pub use helpers::{
    mock_checkpoint_header,
    mock_qc,
    mock_signing_tx,
    mock_v1_header,
    mock_v2_header,
    mock_validator_set,
    MockTransaction,
};
