use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use hiero_runtime_core::{
    FinalityPolicy, FinalizedTransaction, ReceiptResult, RuntimeError, RuntimeErrorCode,
    SubmittedTransaction,
};
use hiero_runtime_mirror::MirrorClient;

use crate::provider::{HbarTransferSubmitter, ReceiptProvider};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HbarTransferRequest {
    pub from_account_id: String,
    pub to_account_id: String,
    pub amount_tinybar: u64,
}

impl HbarTransferRequest {
    pub fn validate(&self) -> Result<(), RuntimeError> {
        if self.from_account_id.trim().is_empty() {
            return Err(RuntimeError::invalid_config(
                "fromAccountId must not be empty",
            ));
        }

        if self.to_account_id.trim().is_empty() {
            return Err(RuntimeError::invalid_config(
                "toAccountId must not be empty",
            ));
        }

        if self.amount_tinybar == 0 {
            return Err(RuntimeError::invalid_config(
                "amountTinybar must be greater than zero",
            ));
        }

        Ok(())
    }
}

struct DisabledHbarTransferSubmitter;

#[async_trait]
impl HbarTransferSubmitter for DisabledHbarTransferSubmitter {
    async fn submit_hbar_transfer(
        &self,
        _request: &HbarTransferRequest,
    ) -> Result<SubmittedTransaction, RuntimeError> {
        Err(RuntimeError::with_details(
            RuntimeErrorCode::Unsupported,
            "HBAR transfer submission is not configured yet for this runtime",
            serde_json::json!({
                "reason": "no submitter configured"
            }),
        ))
    }
}

#[derive(Clone)]
pub struct TxRuntime {
    inner: Arc<TxRuntimeInner>,
}

struct TxRuntimeInner {
    mirror: MirrorClient,
    receipt_provider: Arc<dyn ReceiptProvider>,
    submitter: Arc<dyn HbarTransferSubmitter>,
    finality_policy: FinalityPolicy,
}

#[derive(Clone)]
pub struct AttachedTransaction {
    runtime: TxRuntime,
    transaction_id: String,
}

impl TxRuntime {
    pub fn new(
        mirror: MirrorClient,
        receipt_provider: Arc<dyn ReceiptProvider>,
        finality_policy: FinalityPolicy,
    ) -> Result<Self, RuntimeError> {
        Self::new_with_submitter(
            mirror,
            receipt_provider,
            Arc::new(DisabledHbarTransferSubmitter),
            finality_policy,
        )
    }

    pub fn new_with_submitter(
        mirror: MirrorClient,
        receipt_provider: Arc<dyn ReceiptProvider>,
        submitter: Arc<dyn HbarTransferSubmitter>,
        finality_policy: FinalityPolicy,
    ) -> Result<Self, RuntimeError> {
        finality_policy
            .validate()
            .map_err(RuntimeError::invalid_config)?;

        Ok(Self {
            inner: Arc::new(TxRuntimeInner {
                mirror,
                receipt_provider,
                submitter,
                finality_policy,
            }),
        })
    }

    pub fn attach(
        &self,
        transaction_id: impl Into<String>,
    ) -> Result<AttachedTransaction, RuntimeError> {
        let transaction_id = transaction_id.into();

        if transaction_id.trim().is_empty() {
            return Err(RuntimeError::invalid_config(
                "transactionId must not be empty",
            ));
        }

        Ok(AttachedTransaction {
            runtime: self.clone(),
            transaction_id,
        })
    }

    pub async fn submit_hbar_transfer(
        &self,
        request: HbarTransferRequest,
    ) -> Result<SubmittedTransaction, RuntimeError> {
        request.validate()?;
        self.inner.submitter.submit_hbar_transfer(&request).await
    }

    pub async fn wait_for_receipt(
        &self,
        transaction_id: &str,
    ) -> Result<ReceiptResult, RuntimeError> {
        self.wait_for_receipt_with_policy(transaction_id, &self.inner.finality_policy)
            .await
    }

    pub async fn wait_for_finality(
        &self,
        transaction_id: &str,
    ) -> Result<FinalizedTransaction, RuntimeError> {
        self.wait_for_finality_with_policy(transaction_id, &self.inner.finality_policy)
            .await
    }

    pub async fn wait_for_receipt_with_policy(
        &self,
        transaction_id: &str,
        policy: &FinalityPolicy,
    ) -> Result<ReceiptResult, RuntimeError> {
        if transaction_id.trim().is_empty() {
            return Err(RuntimeError::invalid_config(
                "transactionId must not be empty",
            ));
        }

        policy.validate().map_err(RuntimeError::invalid_config)?;

        let started = Instant::now();

        loop {
            match self
                .inner
                .receipt_provider
                .get_receipt(transaction_id)
                .await
            {
                Ok(Some(receipt)) if receipt.status.eq_ignore_ascii_case("UNKNOWN") => {
                    if started.elapsed() >= Duration::from_millis(policy.receipt_timeout_ms) {
                        return Err(RuntimeError::timeout(format!(
                            "receipt for {transaction_id} not available within {}ms",
                            policy.receipt_timeout_ms
                        )));
                    }

                    tokio::time::sleep(Duration::from_millis(policy.poll_interval_ms)).await;
                }
                Ok(Some(receipt)) => return Ok(receipt),
                Ok(None) => {
                    if started.elapsed() >= Duration::from_millis(policy.receipt_timeout_ms) {
                        return Err(RuntimeError::timeout(format!(
                            "receipt for {transaction_id} not available within {}ms",
                            policy.receipt_timeout_ms
                        )));
                    }

                    tokio::time::sleep(Duration::from_millis(policy.poll_interval_ms)).await;
                }
                Err(err) if err.code == RuntimeErrorCode::NotFound || err.is_retryable() => {
                    if started.elapsed() >= Duration::from_millis(policy.receipt_timeout_ms) {
                        return Err(RuntimeError::timeout(format!(
                            "receipt for {transaction_id} not available within {}ms",
                            policy.receipt_timeout_ms
                        )));
                    }

                    tokio::time::sleep(Duration::from_millis(policy.poll_interval_ms)).await;
                }
                Err(err) => return Err(err),
            }
        }
    }

    pub async fn wait_for_finality_with_policy(
        &self,
        transaction_id: &str,
        policy: &FinalityPolicy,
    ) -> Result<FinalizedTransaction, RuntimeError> {
        let receipt = self
            .wait_for_receipt_with_policy(transaction_id, policy)
            .await?;
        let lookup = self
            .inner
            .mirror
            .wait_for_transaction(transaction_id, policy)
            .await?;

        Ok(FinalizedTransaction {
            transaction_id: transaction_id.to_string(),
            receipt,
            primary_mirror_entry: Some(lookup.primary),
            duplicates: lookup.duplicates,
        })
    }
}

impl AttachedTransaction {
    pub fn transaction_id(&self) -> &str {
        &self.transaction_id
    }

    pub async fn wait_for_receipt(&self) -> Result<ReceiptResult, RuntimeError> {
        self.runtime.wait_for_receipt(&self.transaction_id).await
    }

    pub async fn wait_for_finality(&self) -> Result<FinalizedTransaction, RuntimeError> {
        self.runtime.wait_for_finality(&self.transaction_id).await
    }
}
