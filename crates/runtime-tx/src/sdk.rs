use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use hiero_runtime_core::{
    NetworkKind, ReceiptResult, RuntimeConfig, RuntimeError, RuntimeErrorCode, SubmittedTransaction,
};
use hiero_sdk::{
    AccountId, Client, Hbar, PrivateKey, Status, TransactionId, TransactionReceiptQuery,
    TransferTransaction,
};
use serde_json::json;

use crate::provider::{HbarTransferSubmitter, ReceiptProvider};
use crate::runtime::HbarTransferRequest;

pub struct HieroSdkTxAdapter {
    client: Arc<Client>,
    operator_account_id: Option<AccountId>,
}

impl std::fmt::Debug for HieroSdkTxAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HieroSdkTxAdapter")
            .field("operator_account_id", &self.operator_account_id)
            .finish_non_exhaustive()
    }
}

impl HieroSdkTxAdapter {
    pub fn from_runtime_config(config: &RuntimeConfig) -> Result<Self, RuntimeError> {
        let client = match config.network.kind {
            NetworkKind::Mainnet | NetworkKind::Testnet | NetworkKind::Previewnet => {
                let name = match config.network.kind {
                    NetworkKind::Mainnet => "mainnet",
                    NetworkKind::Testnet => "testnet",
                    NetworkKind::Previewnet => "previewnet",
                    _ => unreachable!(),
                };
                Client::for_name(name)
                    .map_err(|err| map_sdk_error("failed to construct Hiero SDK client", &err))?
            }
            NetworkKind::Custom => {
                let nodes = config.network.consensus_nodes.as_deref().ok_or_else(|| {
                    RuntimeError::with_details(
                        RuntimeErrorCode::InvalidConfig,
                        "consensusNodes is required for custom network transaction submission",
                        json!({ "networkKind": "custom" }),
                    )
                })?;

                if nodes.is_empty() {
                    return Err(RuntimeError::with_details(
                        RuntimeErrorCode::InvalidConfig,
                        "consensusNodes must not be empty for custom network",
                        json!({ "networkKind": "custom" }),
                    ));
                }

                let mut network_map: HashMap<String, AccountId> = HashMap::new();
                for node in nodes {
                    let account_id: AccountId = node.account_id.parse().map_err(|err| {
                        RuntimeError::with_details(
                            RuntimeErrorCode::InvalidConfig,
                            format!("invalid consensus node accountId: {err}"),
                            json!({ "field": "consensusNodes[].accountId", "value": node.account_id }),
                        )
                    })?;
                    network_map.insert(node.url.clone(), account_id);
                }

                Client::for_network(network_map).map_err(|err| {
                    map_sdk_error("failed to construct Hiero SDK client for custom network", &err)
                })?
            }
        };

        let operator_account_id = if let Some(operator) = &config.operator {
            let account_id: AccountId = operator.account_id.parse().map_err(|err| {
                RuntimeError::with_details(
                    RuntimeErrorCode::InvalidConfig,
                    format!("invalid operator.accountId: {err}"),
                    json!({
                        "field": "operator.accountId"
                    }),
                )
            })?;

            let private_key: PrivateKey = operator.private_key.parse().map_err(|err| {
                RuntimeError::with_details(
                    RuntimeErrorCode::InvalidConfig,
                    format!("invalid operator.privateKey: {err}"),
                    json!({
                        "field": "operator.privateKey"
                    }),
                )
            })?;

            client.set_operator(account_id, private_key);
            Some(account_id)
        } else {
            None
        };

        Ok(Self {
            client: Arc::new(client),
            operator_account_id,
        })
    }
}

#[async_trait]
impl ReceiptProvider for HieroSdkTxAdapter {
    async fn get_receipt(
        &self,
        transaction_id: &str,
    ) -> Result<Option<ReceiptResult>, RuntimeError> {
        let tx_id: TransactionId = transaction_id.parse().map_err(|err| {
            RuntimeError::with_details(
                RuntimeErrorCode::InvalidConfig,
                format!("invalid transactionId: {err}"),
                json!({
                    "field": "transactionId",
                    "transactionId": transaction_id
                }),
            )
        })?;

        let receipt = TransactionReceiptQuery::new()
            .transaction_id(tx_id)
            .execute(&self.client)
            .await;

        match receipt {
            Ok(receipt) => Ok(Some(ReceiptResult {
                transaction_id: receipt
                    .transaction_id
                    .map(|id| id.to_string())
                    .unwrap_or_else(|| transaction_id.to_string()),
                status: sdk_status_to_string(receipt.status),
            })),
            Err(err) => {
                if is_receipt_not_found_error(&err) {
                    Ok(None)
                } else {
                    Err(map_sdk_error("failed to query transaction receipt", &err))
                }
            }
        }
    }
}

#[async_trait]
impl HbarTransferSubmitter for HieroSdkTxAdapter {
    async fn submit_hbar_transfer(
        &self,
        request: &HbarTransferRequest,
    ) -> Result<SubmittedTransaction, RuntimeError> {
        request.validate()?;

        let sender: AccountId = request.from_account_id.parse().map_err(|err| {
            RuntimeError::with_details(
                RuntimeErrorCode::InvalidConfig,
                format!("invalid fromAccountId: {err}"),
                json!({
                    "field": "fromAccountId"
                }),
            )
        })?;

        let receiver: AccountId = request.to_account_id.parse().map_err(|err| {
            RuntimeError::with_details(
                RuntimeErrorCode::InvalidConfig,
                format!("invalid toAccountId: {err}"),
                json!({
                    "field": "toAccountId"
                }),
            )
        })?;

        let operator_account_id = self.operator_account_id.ok_or_else(|| {
            RuntimeError::with_details(
                RuntimeErrorCode::InvalidConfig,
                "operator credentials are required for HBAR transfer submission",
                json!({
                    "field": "operator"
                }),
            )
        })?;

        if sender != operator_account_id {
            return Err(RuntimeError::with_details(
                RuntimeErrorCode::InvalidConfig,
                "fromAccountId must match the configured operator account for HBAR transfer submission",
                json!({
                    "fromAccountId": request.from_account_id,
                    "operatorAccountId": operator_account_id.to_string()
                }),
            ));
        }

        let amount_tinybar_i64 = i64::try_from(request.amount_tinybar).map_err(|_| {
            RuntimeError::with_details(
                RuntimeErrorCode::InvalidConfig,
                "amountTinybar exceeds the supported signed tinybar range",
                json!({
                    "amountTinybar": request.amount_tinybar
                }),
            )
        })?;

        let amount = Hbar::from_tinybars(amount_tinybar_i64);

        let response = TransferTransaction::new()
            .hbar_transfer(sender, -amount)
            .hbar_transfer(receiver, amount)
            .execute(&self.client)
            .await
            .map_err(|err| map_sdk_error("failed to submit HBAR transfer", &err))?;

        Ok(SubmittedTransaction {
            transaction_id: response.transaction_id.to_string(),
        })
    }
}

fn sdk_status_to_string(status: Status) -> String {
    // Convert PascalCase Debug name (e.g. "TransactionExpired") to
    // the canonical Hedera SCREAMING_SNAKE_CASE (e.g. "TRANSACTION_EXPIRED").
    let debug = format!("{status:?}");
    let mut out = String::with_capacity(debug.len() + 8);
    for (i, ch) in debug.chars().enumerate() {
        if ch.is_uppercase() && i > 0 {
            out.push('_');
        }
        out.push(ch.to_ascii_uppercase());
    }
    out
}

fn is_receipt_not_found_error(err: &hiero_sdk::Error) -> bool {
    let text = err.to_string().to_ascii_uppercase();
    text.contains("RECEIPTNOTFOUND") || text.contains("RECEIPT_NOT_FOUND")
}

fn map_sdk_error(context: &str, err: &hiero_sdk::Error) -> RuntimeError {
    let text = err.to_string();
    let upper = text.to_ascii_uppercase();

    if upper.contains("RECEIPTNOTFOUND") || upper.contains("RECEIPT_NOT_FOUND") {
        return RuntimeError::with_details(
            RuntimeErrorCode::NotFound,
            format!("{context}: {text}"),
            json!({
                "sdkError": text
            }),
        );
    }

    if upper.contains("BUSY")
        || upper.contains("PLATFORM_TRANSACTION_NOT_CREATED")
        || upper.contains("TRANSACTION_EXPIRED")
        || upper.contains("TIMED OUT")
        || upper.contains("TIMEOUT")
    {
        return RuntimeError::with_retryable_and_details(
            RuntimeErrorCode::Consensus,
            format!("{context}: {text}"),
            true,
            json!({
                "sdkError": text
            }),
        );
    }

    RuntimeError::with_details(
        RuntimeErrorCode::Consensus,
        format!("{context}: {text}"),
        json!({
            "sdkError": text
        }),
    )
}

#[cfg(test)]
mod tests {
    use hiero_runtime_core::{
        FinalityPolicy, NetworkConfig, NetworkKind, OperatorConfig, RetryPolicy, RuntimeConfig,
    };

    use super::*;

    fn base_config(kind: NetworkKind) -> RuntimeConfig {
        RuntimeConfig {
            network: NetworkConfig {
                kind,
                mirror_base_url: "https://testnet.mirrornode.hedera.com".to_string(),
                consensus_nodes: None,
            },
            operator: None,
            retry: RetryPolicy::default(),
            finality: FinalityPolicy::default(),
        }
    }

    #[test]
    fn custom_network_without_consensus_nodes_returns_invalid_config() {
        // base_config sets consensus_nodes: None — that must fail with INVALID_CONFIG.
        let err = HieroSdkTxAdapter::from_runtime_config(&base_config(NetworkKind::Custom))
            .expect_err("custom network without consensusNodes should fail");

        assert_eq!(err.code, RuntimeErrorCode::InvalidConfig);
    }

    #[test]
    fn custom_network_with_empty_consensus_nodes_returns_invalid_config() {
        use hiero_runtime_core::NetworkConfig;
        let config = RuntimeConfig {
            network: NetworkConfig {
                kind: NetworkKind::Custom,
                mirror_base_url: "https://mirror.example.com".to_string(),
                consensus_nodes: Some(vec![]),
            },
            operator: None,
            retry: RetryPolicy::default(),
            finality: FinalityPolicy::default(),
        };

        let err = HieroSdkTxAdapter::from_runtime_config(&config)
            .expect_err("empty consensusNodes should fail");

        assert_eq!(err.code, RuntimeErrorCode::InvalidConfig);
    }

    #[tokio::test]
    async fn named_network_without_operator_still_builds_receipt_capable_adapter() {
        let adapter = HieroSdkTxAdapter::from_runtime_config(&base_config(NetworkKind::Testnet))
            .expect("named network adapter should build");

        assert!(adapter.operator_account_id.is_none());
    }

    #[tokio::test]
    async fn invalid_operator_values_fail_fast() {
        let mut config = base_config(NetworkKind::Testnet);
        config.operator = Some(OperatorConfig {
            account_id: "not-an-account".to_string(),
            private_key: "also-not-a-key".to_string(),
        });

        let err = HieroSdkTxAdapter::from_runtime_config(&config)
            .expect_err("invalid operator config should fail");

        assert_eq!(err.code, RuntimeErrorCode::InvalidConfig);
    }
}
