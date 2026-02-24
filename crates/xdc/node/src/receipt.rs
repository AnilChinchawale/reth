//! XDC Receipt Builder
//!
//! This module provides receipt building for XDC Network

use alloy_consensus::TxType;
use alloy_evm::eth::receipt_builder::{ReceiptBuilder, ReceiptBuilderCtx};
use reth_ethereum_primitives::{Receipt, TransactionSigned};
use reth_evm::Evm;

/// XDC receipt builder
///
/// Builds receipts for XDC transactions (same as Ethereum)
#[derive(Debug, Default, Clone, Copy)]
#[non_exhaustive]
pub struct XdcReceiptBuilder;

impl ReceiptBuilder for XdcReceiptBuilder {
    type Transaction = TransactionSigned;
    type Receipt = Receipt;

    fn build_receipt<E: Evm>(&self, ctx: ReceiptBuilderCtx<'_, TxType, E>) -> Self::Receipt {
        let ReceiptBuilderCtx { tx_type, result, cumulative_gas_used, .. } = ctx;
        Receipt {
            tx_type,
            // Success flag was added in `EIP-658: Embedding transaction status code in
            // receipts`.
            success: result.is_success(),
            cumulative_gas_used,
            logs: result.into_logs(),
        }
    }
}
