use std::time::{Duration, Instant};

use hiero_runtime_core::{
    ContractResultView, FinalityPolicy, MirrorAccountView, MirrorTransactionRecord, RetryDecision,
    RetryPolicy, RuntimeError, RuntimeErrorCode, TransactionPage, TransactionRef,
};

use reqwest::{Client, StatusCode};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::json;

use crate::dto::{MirrorAccountDto, MirrorContractResultDto, MirrorTransactionsResponseDto};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionLookup {
    pub requested_transaction_id: String,
    pub primary: MirrorTransactionRecord,
    pub duplicates: Vec<MirrorTransactionRecord>,
    pub entries: Vec<MirrorTransactionRecord>,
}

impl TransactionLookup {
    pub fn has_duplicates(&self) -> bool {
        !self.duplicates.is_empty()
    }
}

#[derive(Clone)]
pub struct MirrorClient {
    http: Client,
    base_url: String,
    retry: RetryPolicy,
}

impl MirrorClient {
    pub fn new(base_url: impl Into<String>, retry: RetryPolicy) -> Result<Self, RuntimeError> {
        let base_url = base_url.into().trim_end_matches('/').to_string();

        if base_url.is_empty() {
            return Err(RuntimeError::invalid_config(
                "mirror base URL must not be empty",
            ));
        }

        let http = Client::builder().build().map_err(|err| {
            RuntimeError::internal(format!("failed to build reqwest client: {err}"))
        })?;

        Ok(Self {
            http,
            base_url,
            retry,
        })
    }

    pub async fn get_transaction(
        &self,
        transaction_id: &str,
    ) -> Result<TransactionLookup, RuntimeError> {
        let tx_ref = TransactionRef::parse(transaction_id)?;
        self.get_transaction_by_ref(&tx_ref).await
    }

    pub async fn wait_for_transaction(
        &self,
        transaction_id: &str,
        policy: &FinalityPolicy,
    ) -> Result<TransactionLookup, RuntimeError> {
        let tx_ref = TransactionRef::parse(transaction_id)?;

        let started = Instant::now();

        loop {
            match self.get_transaction_by_ref(&tx_ref).await {
                Ok(lookup) => return Ok(lookup),
                Err(err) if err.code == RuntimeErrorCode::NotFound => {
                    if started.elapsed() >= Duration::from_millis(policy.mirror_timeout_ms) {
                        return Err(RuntimeError::timeout(format!(
                            "mirror transaction {} not visible within {}ms",
                            tx_ref.to_canonical_string(),
                            policy.mirror_timeout_ms
                        )));
                    }

                    tokio::time::sleep(Duration::from_millis(policy.poll_interval_ms)).await;
                }
                Err(err) if err.is_retryable() => {
                    if started.elapsed() >= Duration::from_millis(policy.mirror_timeout_ms) {
                        return Err(RuntimeError::timeout(format!(
                            "mirror transaction {} not visible within {}ms",
                            tx_ref.to_canonical_string(),
                            policy.mirror_timeout_ms
                        )));
                    }

                    tokio::time::sleep(Duration::from_millis(policy.poll_interval_ms)).await;
                }
                Err(err) => return Err(err),
            }
        }
    }

    pub async fn get_account(&self, id: &str) -> Result<MirrorAccountView, RuntimeError> {
        if id.trim().is_empty() {
            return Err(RuntimeError::invalid_config("account id must not be empty"));
        }

        let path = format!("/api/v1/accounts/{id}");
        let dto: MirrorAccountDto = self.get_json_with_retry(&path).await?;
        dto.try_into()
    }

    pub async fn get_contract_result(
        &self,
        transaction_id_or_hash: &str,
        nonce: Option<i32>,
    ) -> Result<ContractResultView, RuntimeError> {
        if transaction_id_or_hash.trim().is_empty() {
            return Err(RuntimeError::invalid_config(
                "transactionIdOrHash must not be empty",
            ));
        }

        let mut path = format!("/api/v1/contracts/results/{transaction_id_or_hash}");
        if let Some(n) = nonce {
            path.push_str(&format!("?nonce={n}"));
        }

        let dto: MirrorContractResultDto = self.get_json_with_retry(&path).await?;
        dto.try_into()
    }

    /// List transactions for an account, with cursor-based pagination.
    ///
    /// `limit` caps the number of records per page (Mirror Node max is 100).
    /// `cursor` is the opaque `next_cursor` from a previous `TransactionPage`;
    /// omit it to start from the most recent transaction.
    pub async fn list_transactions_for_account(
        &self,
        account_id: &str,
        limit: u32,
        cursor: Option<&str>,
    ) -> Result<TransactionPage, RuntimeError> {
        if account_id.trim().is_empty() {
            return Err(RuntimeError::invalid_config(
                "accountId must not be empty",
            ));
        }

        // When a cursor is provided it is already a full Mirror Node path
        let path = match cursor {
            Some(c) if !c.trim().is_empty() => c.to_string(),
            _ => format!(
                "/api/v1/transactions?account.id={account_id}&limit={limit}&order=desc"
            ),
        };

        let response: MirrorTransactionsResponseDto = self.get_json_with_retry(&path).await?;

        let items: Vec<MirrorTransactionRecord> =
            response.transactions.into_iter().map(Into::into).collect();

        let next_cursor = response.links.and_then(|l| l.next);

        Ok(TransactionPage { items, next_cursor })
    }

    // ── Private helpers ───────────────────────────────────────────────────────

    async fn get_transaction_by_ref(
        &self,
        tx_ref: &TransactionRef,
    ) -> Result<TransactionLookup, RuntimeError> {
        let path = tx_ref.to_mirror_path();
        let response: MirrorTransactionsResponseDto = self.get_json_with_retry(&path).await?;

        if response.transactions.is_empty() {
            return Err(RuntimeError::not_found(format!(
                "mirror transaction lookup returned no records for {}",
                tx_ref.to_canonical_string()
            )));
        }

        Ok(normalize_transaction_lookup(tx_ref, response.transactions))
    }

    async fn get_json_with_retry<T>(&self, path: &str) -> Result<T, RuntimeError>
    where
        T: DeserializeOwned,
    {
        let url = format!("{}{}", self.base_url, path);
        let mut attempts_completed = 0u32;

        loop {
            let response = self.http.get(&url).send().await;

            let result = match response {
                Ok(resp) => {
                    if resp.status().is_success() {
                        resp.json::<T>().await.map_err(|err| {
                            RuntimeError::with_details(
                                RuntimeErrorCode::Serialization,
                                format!("failed to deserialize mirror response: {err}"),
                                json!({
                                    "path": path
                                }),
                            )
                        })
                    } else {
                        Err(map_http_status(resp.status(), path))
                    }
                }
                Err(err) => Err(map_transport_error(err, path)),
            };

            match result {
                Ok(value) => return Ok(value),
                Err(err) => {
                    attempts_completed += 1;
                    let decision = self.retry.classify_retry(attempts_completed, &err);

                    match decision {
                        RetryDecision {
                            should_retry: true,
                            next_delay_ms: Some(delay_ms),
                        } => {
                            tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                        }
                        _ => return Err(err),
                    }
                }
            }
        }
    }
}

// ── Normalisation ─────────────────────────────────────────────────────────────

fn normalize_transaction_lookup(
    tx_ref: &TransactionRef,
    transactions: Vec<crate::dto::MirrorTransactionDto>,
) -> TransactionLookup {
    let entries: Vec<MirrorTransactionRecord> = transactions.into_iter().map(Into::into).collect();


    let primary_idx = entries
        .iter()
        .position(|e| {
            e.scheduled.unwrap_or(false) == tx_ref.scheduled && e.nonce == tx_ref.nonce
        })
        .unwrap_or(0);

    let primary = entries[primary_idx].clone();
    let duplicates: Vec<MirrorTransactionRecord> = entries
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != primary_idx)
        .map(|(_, e)| e.clone())
        .collect();

    TransactionLookup {
        requested_transaction_id: tx_ref.to_canonical_string(),
        primary,
        duplicates,
        entries,
    }
}

// ── HTTP error mapping ────────────────────────────────────────────────────────

fn map_transport_error(err: reqwest::Error, path: &str) -> RuntimeError {
    if err.is_timeout() {
        RuntimeError::with_retryable_and_details(
            RuntimeErrorCode::Timeout,
            format!("mirror request timed out: {err}"),
            true,
            json!({
                "path": path
            }),
        )
    } else {
        RuntimeError::with_retryable_and_details(
            RuntimeErrorCode::Transport,
            format!("mirror transport error: {err}"),
            true,
            json!({
                "path": path
            }),
        )
    }
}

fn map_http_status(status: StatusCode, path: &str) -> RuntimeError {
    let status_u16 = status.as_u16();

    match status_u16 {
        404 => RuntimeError::with_details(
            RuntimeErrorCode::NotFound,
            format!("mirror resource not found at {path}"),
            json!({
                "path": path,
                "status": status_u16
            }),
        ),
        429 => RuntimeError::with_retryable_and_details(
            RuntimeErrorCode::RateLimited,
            format!("mirror rate limited request to {path}"),
            true,
            json!({
                "path": path,
                "status": status_u16
            }),
        ),
        500..=599 => RuntimeError::with_retryable_and_details(
            RuntimeErrorCode::MirrorHttp,
            format!("mirror server error on {path}"),
            true,
            json!({
                "path": path,
                "status": status_u16
            }),
        ),
        _ => RuntimeError::with_details(
            RuntimeErrorCode::MirrorHttp,
            format!("mirror http error on {path}"),
            json!({
                "path": path,
                "status": status_u16
            }),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dto::MirrorTransactionDto;

    #[test]
    fn normalize_transaction_lookup_selects_primary_by_scheduled_flag() {
        let tx_ref = TransactionRef::parse("0.0.1001@1719943901.123456789").unwrap();
        let lookup = normalize_transaction_lookup(
            &tx_ref,
            vec![
                MirrorTransactionDto {
                    transaction_id: "0.0.1001@1719943901.123456789".to_string(),
                    result: "SUCCESS".to_string(),
                    consensus_timestamp: Some("1719943901.111111111".to_string()),
                    name: Some("CRYPTOTRANSFER".to_string()),
                    scheduled: Some(false),
                    nonce: None,
                },
                MirrorTransactionDto {
                    transaction_id: "0.0.1001@1719943901.123456789".to_string(),
                    result: "DUPLICATE_TRANSACTION".to_string(),
                    consensus_timestamp: Some("1719943901.222222222".to_string()),
                    name: Some("CRYPTOTRANSFER".to_string()),
                    scheduled: Some(false),
                    nonce: None,
                },
            ],
        );

        assert_eq!(
            lookup.requested_transaction_id,
            "0.0.1001@1719943901.123456789"
        );
        assert_eq!(lookup.primary.result, "SUCCESS");
        assert_eq!(lookup.duplicates.len(), 1);
        assert_eq!(lookup.entries.len(), 2);
        assert!(lookup.has_duplicates());
    }

    #[test]
    fn normalize_transaction_lookup_selects_scheduled_entry_as_primary() {
        let tx_ref =
            TransactionRef::parse("0.0.1001@1719943901.123456789?scheduled").unwrap();
        let lookup = normalize_transaction_lookup(
            &tx_ref,
            vec![
                MirrorTransactionDto {
                    transaction_id: "0.0.1001@1719943901.123456789".to_string(),
                    result: "FEE_SCHEDULE_CHANGE_NOT_AUTHORIZED".to_string(),
                    consensus_timestamp: Some("1719943901.000000001".to_string()),
                    name: Some("CRYPTOTRANSFER".to_string()),
                    scheduled: Some(false),
                    nonce: None,
                },
                MirrorTransactionDto {
                    transaction_id: "0.0.1001@1719943901.123456789".to_string(),
                    result: "SUCCESS".to_string(),
                    consensus_timestamp: Some("1719943901.111111111".to_string()),
                    name: Some("CRYPTOTRANSFER".to_string()),
                    scheduled: Some(true),
                    nonce: None,
                },
            ],
        );

        assert_eq!(
            lookup.requested_transaction_id,
            "0.0.1001@1719943901.123456789?scheduled"
        );
        // The scheduled entry (index 1) must be selected as primary.
        assert_eq!(lookup.primary.result, "SUCCESS");
        assert_eq!(lookup.primary.scheduled, Some(true));
        assert_eq!(lookup.duplicates.len(), 1);
    }
}
