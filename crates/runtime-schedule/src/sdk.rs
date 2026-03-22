use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use hiero_runtime_core::{
    CreatedSchedule, NetworkKind, RuntimeConfig, RuntimeError, RuntimeErrorCode, ScheduleInfoView,
    ScheduleState,
};
use hiero_sdk::{
    AccountId, Client, Hbar, Key, PrivateKey, ScheduleCreateTransaction, ScheduleDeleteTransaction,
    ScheduleId, ScheduleInfoQuery, ScheduleSignTransaction, TransferTransaction,
};
use serde_json::json;
use time::OffsetDateTime;

use crate::provider::ScheduleProvider;
use crate::runtime::{CreateTransferRequest, SignScheduleRequest};

pub struct HieroSdkScheduleProvider {
    client: Arc<Client>,
}

impl HieroSdkScheduleProvider {
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
                    .map_err(|err| map_sdk_error("failed to build Hiero SDK client", &err))?
            }
            NetworkKind::Custom => {
                let nodes = config.network.consensus_nodes.as_deref().ok_or_else(|| {
                    RuntimeError::with_details(
                        RuntimeErrorCode::InvalidConfig,
                        "consensusNodes is required for custom network schedule operations",
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
                    map_sdk_error(
                        "failed to build Hiero SDK client for custom network",
                        &err,
                    )
                })?
            }
        };

        if let Some(operator) = &config.operator {
            let account_id: AccountId = operator.account_id.parse().map_err(|err| {
                RuntimeError::with_details(
                    RuntimeErrorCode::InvalidConfig,
                    format!("invalid operator.accountId: {err}"),
                    json!({ "field": "operator.accountId" }),
                )
            })?;

            let private_key: PrivateKey = operator.private_key.parse().map_err(|err| {
                RuntimeError::with_details(
                    RuntimeErrorCode::InvalidConfig,
                    format!("invalid operator.privateKey: {err}"),
                    json!({ "field": "operator.privateKey" }),
                )
            })?;

            client.set_operator(account_id, private_key);
        }

        Ok(Self {
            client: Arc::new(client),
        })
    }
}

#[async_trait]
impl ScheduleProvider for HieroSdkScheduleProvider {
    async fn create_transfer(
        &self,
        request: &CreateTransferRequest,
    ) -> Result<CreatedSchedule, RuntimeError> {
        let sender: AccountId = request.from_account_id.parse().map_err(|err| {
            RuntimeError::with_details(
                RuntimeErrorCode::InvalidConfig,
                format!("invalid fromAccountId: {err}"),
                json!({ "field": "fromAccountId" }),
            )
        })?;

        let receiver: AccountId = request.to_account_id.parse().map_err(|err| {
            RuntimeError::with_details(
                RuntimeErrorCode::InvalidConfig,
                format!("invalid toAccountId: {err}"),
                json!({ "field": "toAccountId" }),
            )
        })?;

        let amount_tinybar_i64 = i64::try_from(request.amount_tinybar).map_err(|_| {
            RuntimeError::with_details(
                RuntimeErrorCode::InvalidConfig,
                "amountTinybar exceeds the supported signed tinybar range",
                json!({ "amountTinybar": request.amount_tinybar }),
            )
        })?;

        let amount = Hbar::from_tinybars(amount_tinybar_i64);

        let mut inner_transfer = TransferTransaction::new();
        inner_transfer
            .hbar_transfer(sender, -amount)
            .hbar_transfer(receiver, amount);

        let mut schedule_tx = ScheduleCreateTransaction::new();
        schedule_tx.scheduled_transaction(inner_transfer);

        if let Some(payer_str) = &request.payer_account_id {
            let payer: AccountId = payer_str.parse().map_err(|err| {
                RuntimeError::with_details(
                    RuntimeErrorCode::InvalidConfig,
                    format!("invalid payerAccountId: {err}"),
                    json!({ "field": "payerAccountId" }),
                )
            })?;
            schedule_tx.payer_account_id(payer);
        }

        if let Some(memo) = &request.memo {
            schedule_tx.schedule_memo(memo.clone());
        }

        let response = schedule_tx
            .execute(&self.client)
            .await
            .map_err(|err| map_sdk_error("failed to submit ScheduleCreateTransaction", &err))?;

        let receipt = response
            .get_receipt(&self.client)
            .await
            .map_err(|err| {
                map_sdk_error("failed to get receipt for ScheduleCreateTransaction", &err)
            })?;

        let schedule_id = receipt.schedule_id.ok_or_else(|| {
            RuntimeError::new(
                RuntimeErrorCode::Schedule,
                "ScheduleCreateTransaction receipt missing scheduleId",
            )
        })?;

        let scheduled_transaction_id = receipt.scheduled_transaction_id.ok_or_else(|| {
            RuntimeError::new(
                RuntimeErrorCode::Schedule,
                "ScheduleCreateTransaction receipt missing scheduledTransactionId",
            )
        })?;

        let schedule_id_str = schedule_id.to_string();
        let info = self.get(&schedule_id_str).await?;

        Ok(CreatedSchedule {
            schedule_id: schedule_id_str,
            scheduled_transaction_id: scheduled_transaction_id.to_string(),
            status: info.status,
        })
    }

    async fn sign(
        &self,
        request: &SignScheduleRequest,
    ) -> Result<ScheduleInfoView, RuntimeError> {
        let schedule_id: ScheduleId = request.schedule_id.parse().map_err(|err| {
            RuntimeError::with_details(
                RuntimeErrorCode::InvalidConfig,
                format!("invalid scheduleId: {err}"),
                json!({ "field": "scheduleId", "scheduleId": request.schedule_id }),
            )
        })?;

        let signer_key: PrivateKey = request.signer_private_key.parse().map_err(|err| {
            RuntimeError::with_details(
                RuntimeErrorCode::InvalidConfig,
                format!("invalid signerPrivateKey: {err}"),
                json!({ "field": "signerPrivateKey" }),
            )
        })?;

        let mut sign_tx = ScheduleSignTransaction::new();
        sign_tx.schedule_id(schedule_id);
        sign_tx.sign(signer_key);

        let response = sign_tx
            .execute(&self.client)
            .await
            .map_err(|err| map_sdk_error("failed to submit ScheduleSignTransaction", &err))?;

        response
            .get_receipt(&self.client)
            .await
            .map_err(|err| map_sdk_error("failed to get receipt for ScheduleSignTransaction", &err))?;

        self.get(&request.schedule_id).await
    }

    async fn delete(&self, schedule_id: &str) -> Result<(), RuntimeError> {
        let id: ScheduleId = schedule_id.parse().map_err(|err| {
            RuntimeError::with_details(
                RuntimeErrorCode::InvalidConfig,
                format!("invalid scheduleId: {err}"),
                json!({ "field": "scheduleId", "scheduleId": schedule_id }),
            )
        })?;

        let response = ScheduleDeleteTransaction::new()
            .schedule_id(id)
            .execute(&self.client)
            .await
            .map_err(|err| map_sdk_error("failed to submit ScheduleDeleteTransaction", &err))?;

        response
            .get_receipt(&self.client)
            .await
            .map_err(|err| map_sdk_error("failed to get receipt for ScheduleDeleteTransaction", &err))?;

        Ok(())
    }

    async fn get(&self, schedule_id: &str) -> Result<ScheduleInfoView, RuntimeError> {
        let id: ScheduleId = schedule_id.parse().map_err(|err| {
            RuntimeError::with_details(
                RuntimeErrorCode::InvalidConfig,
                format!("invalid scheduleId: {err}"),
                json!({ "field": "scheduleId", "scheduleId": schedule_id }),
            )
        })?;

        let info = ScheduleInfoQuery::new()
            .schedule_id(id)
            .execute(&self.client)
            .await
            .map_err(|err| {
                if is_not_found_error(&err) {
                    RuntimeError::with_details(
                        RuntimeErrorCode::NotFound,
                        format!("schedule {schedule_id} not found"),
                        json!({ "scheduleId": schedule_id }),
                    )
                } else {
                    map_sdk_error("failed to query schedule info", &err)
                }
            })?;

        let status = derive_schedule_state(&info);

        let signatories = info
            .signatories
            .keys
            .into_iter()
            .flat_map(collect_key_strings)
            .collect();

        Ok(ScheduleInfoView {
            schedule_id: info.schedule_id.to_string(),
            payer_account_id: info.payer_account_id.map(|id| id.to_string()),
            creator_account_id: Some(info.creator_account_id.to_string()),
            signatories,
            scheduled_transaction_id: Some(info.scheduled_transaction_id.to_string()),
            status,
            expiration_time: info.expiration_time.map(format_timestamp),
            executed_timestamp: info.executed_at.map(format_timestamp),
            deletion_timestamp: info.deleted_at.map(format_timestamp),
        })
    }
}

/// Extract public key strings from a `Key` node.

fn collect_key_strings(key: Key) -> Vec<String> {
    match key {
        Key::Single(pk) => vec![pk.to_string()],
        Key::KeyList(kl) => kl.keys.into_iter().flat_map(collect_key_strings).collect(),
        _ => vec![],
    }
}

fn derive_schedule_state(info: &hiero_sdk::ScheduleInfo) -> ScheduleState {
    if info.executed_at.is_some() {
        return ScheduleState::Executed;
    }
    if info.deleted_at.is_some() {
        return ScheduleState::Deleted;
    }
    if let Some(expiry) = info.expiration_time {
        if expiry < OffsetDateTime::now_utc() {
            return ScheduleState::Expired;
        }
    }
    ScheduleState::PendingSignatures
}

fn format_timestamp(t: OffsetDateTime) -> String {
    format!("{}.{:09}", t.unix_timestamp(), t.nanosecond())
}

fn is_not_found_error(err: &hiero_sdk::Error) -> bool {
    let upper = err.to_string().to_ascii_uppercase();
    upper.contains("INVALID_SCHEDULE_ID")
        || upper.contains("SCHEDULE_DELETED")
        || upper.contains("NOT_FOUND")
}

fn map_sdk_error(context: &str, err: &hiero_sdk::Error) -> RuntimeError {
    let text = err.to_string();
    let upper = text.to_ascii_uppercase();

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
            json!({ "sdkError": text }),
        );
    }

    RuntimeError::with_details(
        RuntimeErrorCode::Schedule,
        format!("{context}: {text}"),
        json!({ "sdkError": text }),
    )
}
