use async_trait::async_trait;
use hiero_runtime_core::{ReceiptResult, RuntimeError, SubmittedTransaction};

use crate::runtime::HbarTransferRequest;

#[async_trait]
pub trait ReceiptProvider: Send + Sync + 'static {
    async fn get_receipt(
        &self,
        transaction_id: &str,
    ) -> Result<Option<ReceiptResult>, RuntimeError>;
}

#[async_trait]
pub trait HbarTransferSubmitter: Send + Sync + 'static {
    async fn submit_hbar_transfer(
        &self,
        request: &HbarTransferRequest,
    ) -> Result<SubmittedTransaction, RuntimeError>;
}
