use serde::Deserialize;

use hiero_runtime_core::{
    ContractResultView, MirrorAccountView, MirrorTransactionRecord, RuntimeError, RuntimeErrorCode,
};

// ── Transaction DTOs ──────────────────────────────────────────────────────────

/// Cursor links returned by Mirror Node list responses.
#[derive(Debug, Deserialize, Default)]
pub struct MirrorLinksDto {
    #[serde(default)]
    pub next: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct MirrorTransactionsResponseDto {
    pub transactions: Vec<MirrorTransactionDto>,
    #[serde(default)]
    pub links: Option<MirrorLinksDto>,
}

#[derive(Debug, Deserialize)]
pub struct MirrorTransactionDto {
    pub transaction_id: String,
    pub result: String,
    #[serde(default)]
    pub consensus_timestamp: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub scheduled: Option<bool>,
    #[serde(default)]
    pub nonce: Option<i32>,
}

impl From<MirrorTransactionDto> for MirrorTransactionRecord {
    fn from(value: MirrorTransactionDto) -> Self {
        Self {
            transaction_id: value.transaction_id,
            result: value.result,
            consensus_timestamp: value.consensus_timestamp,
            name: value.name,
            scheduled: value.scheduled,
            nonce: value.nonce,
        }
    }
}

// ── Account DTOs ──────────────────────────────────────────────────────────────

/// Raw DTO for `GET /api/v1/accounts/{idOrAliasOrEvmAddress}`.
#[derive(Debug, Deserialize)]
pub struct MirrorAccountDto {
    pub account: String,
    #[serde(default)]
    pub balance: Option<MirrorAccountBalanceDto>,
    #[serde(default)]
    pub evm_address: Option<String>,
    #[serde(default)]
    pub deleted: bool,
    #[serde(default)]
    pub memo: String,
}

#[derive(Debug, Deserialize)]
pub struct MirrorAccountBalanceDto {
    pub balance: i64,
}

impl TryFrom<MirrorAccountDto> for MirrorAccountView {
    type Error = RuntimeError;

    fn try_from(value: MirrorAccountDto) -> Result<Self, RuntimeError> {
        if value.account.is_empty() {
            return Err(RuntimeError::with_details(
                RuntimeErrorCode::Serialization,
                "mirror account response missing required field: account",
                serde_json::json!({}),
            ));
        }

        Ok(Self {
            account: value.account,
            balance: value
                .balance
                .map(|b| b.balance)
                .unwrap_or(0)
                .to_string(),
            evm_address: value.evm_address,
            deleted: value.deleted,
            memo: value.memo,
        })
    }
}

// ── Contract result DTOs ──────────────────────────────────────────────────────

/// Raw DTO for `GET /api/v1/contracts/results/{transactionIdOrHash}`.
#[derive(Debug, Deserialize)]
pub struct MirrorContractResultDto {
    #[serde(default)]
    pub contract_id: Option<String>,
    #[serde(default)]
    pub transaction_id: Option<String>,
    #[serde(default)]
    pub result: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub gas_used: Option<u64>,
    #[serde(default)]
    pub error_message: Option<String>,
    #[serde(default)]
    pub call_result: Option<String>,
    #[serde(default)]
    pub from: Option<String>,
    #[serde(default)]
    pub to: Option<String>,
}

impl TryFrom<MirrorContractResultDto> for ContractResultView {
    type Error = RuntimeError;

    fn try_from(value: MirrorContractResultDto) -> Result<Self, RuntimeError> {
        let transaction_id = value.transaction_id.ok_or_else(|| {
            RuntimeError::with_details(
                RuntimeErrorCode::Serialization,
                "mirror contract result response missing required field: transaction_id",
                serde_json::json!({}),
            )
        })?;

        let result = value.result.ok_or_else(|| {
            RuntimeError::with_details(
                RuntimeErrorCode::Serialization,
                "mirror contract result response missing required field: result",
                serde_json::json!({}),
            )
        })?;

        let status = value.status.ok_or_else(|| {
            RuntimeError::with_details(
                RuntimeErrorCode::Serialization,
                "mirror contract result response missing required field: status",
                serde_json::json!({}),
            )
        })?;

        Ok(Self {
            contract_id: value.contract_id,
            transaction_id,
            result,
            status,
            gas_used: value.gas_used.unwrap_or(0).to_string(),
            error_message: value.error_message,
            call_result: value.call_result,
            from: value.from,
            to: value.to,
        })
    }
}
