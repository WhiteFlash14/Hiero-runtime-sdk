use serde::{Deserialize, Serialize};

use crate::error::{RuntimeError, RuntimeErrorCode};

/// Parsed representation of a Hedera transaction reference.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransactionRef {
    pub base_id: String,
    pub scheduled: bool,
    pub nonce: Option<i32>,
}

impl TransactionRef {
    pub fn parse(raw: &str) -> Result<Self, RuntimeError> {
        if raw.trim().is_empty() {
            return Err(RuntimeError::new(
                RuntimeErrorCode::InvalidConfig,
                "transactionId must not be empty",
            ));
        }

        let mut scheduled = false;
        let mut nonce: Option<i32> = None;
        let base_id: String;

        if let Some(q) = raw.find('?') {
            let query = &raw[q + 1..];
            base_id = raw[..q].trim().to_string();

            for part in query.split('&') {
                let trimmed = part.trim();
                if trimmed == "scheduled" || trimmed == "scheduled=" {
                    scheduled = true;
                } else if let Some(n_str) = trimmed.strip_prefix("nonce=") {
                    if let Ok(n) = n_str.parse::<i32>() {
                        nonce = Some(n);
                    }
                }
            }
        } else {
            base_id = raw.trim().to_string();
        }

        if base_id.is_empty() {
            return Err(RuntimeError::new(
                RuntimeErrorCode::InvalidConfig,
                "transactionId base must not be empty",
            ));
        }

        Ok(Self {
            base_id,
            scheduled,
            nonce,
        })
    }

    /// Returns the canonical string representation
    pub fn to_canonical_string(&self) -> String {
        let mut s = self.base_id.clone();
        if self.scheduled {
            s.push_str("?scheduled");
        } else if let Some(n) = self.nonce {
            s.push_str(&format!("?nonce={n}"));
        }
        s
    }

    /// Returns the Mirror Node API path for this transaction reference
    pub fn to_mirror_path(&self) -> String {
        let normalized = normalize_tx_id_for_mirror(&self.base_id);
        let mut path = format!("/api/v1/transactions/{normalized}");
        if self.scheduled {
            path.push_str("?scheduled=true");
        } else if let Some(n) = self.nonce {
            path.push_str(&format!("?nonce={n}"));
        }
        path
    }
}

fn normalize_tx_id_for_mirror(base_id: &str) -> String {
    if let Some(at_pos) = base_id.find('@') {
        let account = &base_id[..at_pos];
        let timestamp = &base_id[at_pos + 1..];
        if let Some(dot_pos) = timestamp.find('.') {
            let seconds = &timestamp[..dot_pos];
            let nanos = &timestamp[dot_pos + 1..];
            return format!("{account}-{seconds}-{nanos:0>9}");
        }
    }
    base_id.to_string()
}

// ── Domain models ─────────────────────────────────────────────────────────────


#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmittedTransaction {
    pub transaction_id: String,
}

/// Receipt oriented result returned once consensus receipt is available.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReceiptResult {
    pub transaction_id: String,
    pub status: String,
}

/// Normalized mirror transaction entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MirrorTransactionRecord {
    pub transaction_id: String,
    pub result: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub consensus_timestamp: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scheduled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nonce: Option<i32>,
}

/// Normalized mirror account view returned by `GET /api/v1/accounts/{id}`
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MirrorAccountView {
    pub account: String,
    pub balance: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evm_address: Option<String>,
    pub deleted: bool,
    pub memo: String,
}

/// Normalized contract execution result returned by
/// `GET /api/v1/contracts/results/{transactionIdOrHash}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContractResultView {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contract_id: Option<String>,
    pub transaction_id: String,
    pub result: String,
    pub status: String,
    pub gas_used: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub call_result: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
}

/// A single page of transaction records returned by list queries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionPage {
    pub items: Vec<MirrorTransactionRecord>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

/// Finalized tx view combining receipt level state and Mirror visibility.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinalizedTransaction {
    pub transaction_id: String,
    pub receipt: ReceiptResult,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub primary_mirror_entry: Option<MirrorTransactionRecord>,
    pub duplicates: Vec<MirrorTransactionRecord>,
}

/// Stable schedule lifecycle states used by the runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ScheduleState {
    PendingSignatures,
    Executed,
    Expired,
    Deleted,
}

/// Result returned when a schedule has been created successfully.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatedSchedule {
    pub schedule_id: String,
    pub scheduled_transaction_id: String,
    pub status: ScheduleState,
}

/// Schedule summary used across schedule orchestration APIs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduleInfoView {
    pub schedule_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payer_account_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creator_account_id: Option<String>,
    pub signatories: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scheduled_transaction_id: Option<String>,
    pub status: ScheduleState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expiration_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub executed_timestamp: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deletion_timestamp: Option<String>,
}

/// Final schedule execution result, normalized through the tx runtime.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduleExecution {
    pub schedule_id: String,
    pub scheduled_transaction_id: String,
    pub finalized: FinalizedTransaction,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finalized_transaction_serializes_with_expected_shape() {
        let finalized = FinalizedTransaction {
            transaction_id: "0.0.1001@1719943901.123456789".to_string(),
            receipt: ReceiptResult {
                transaction_id: "0.0.1001@1719943901.123456789".to_string(),
                status: "SUCCESS".to_string(),
            },
            primary_mirror_entry: Some(MirrorTransactionRecord {
                transaction_id: "0.0.1001@1719943901.123456789".to_string(),
                result: "SUCCESS".to_string(),
                consensus_timestamp: Some("1719943901.999999999".to_string()),
                name: Some("CRYPTOTRANSFER".to_string()),
                scheduled: None,
                nonce: None,
            }),
            duplicates: vec![],
        };

        let json = serde_json::to_value(&finalized).expect("finalized tx should serialize");

        assert_eq!(json["transactionId"], "0.0.1001@1719943901.123456789");
        assert_eq!(json["receipt"]["status"], "SUCCESS");
        assert_eq!(json["primaryMirrorEntry"]["result"], "SUCCESS");
        assert_eq!(json["primaryMirrorEntry"]["name"], "CRYPTOTRANSFER");
        assert_eq!(
            json["primaryMirrorEntry"]["consensusTimestamp"],
            "1719943901.999999999"
        );
        assert_eq!(
            json["duplicates"]
                .as_array()
                .expect("duplicates should be an array")
                .len(),
            0
        );
    }

    #[test]
    fn created_schedule_serializes_with_status() {
        let created = CreatedSchedule {
            schedule_id: "0.0.7001".to_string(),
            scheduled_transaction_id: "0.0.1001@1719943901.123456789?scheduled".to_string(),
            status: ScheduleState::PendingSignatures,
        };

        let json = serde_json::to_value(&created).expect("created schedule should serialize");

        assert_eq!(json["scheduleId"], "0.0.7001");
        assert_eq!(
            json["scheduledTransactionId"],
            "0.0.1001@1719943901.123456789?scheduled"
        );
        assert_eq!(json["status"], "pendingSignatures");
    }

    #[test]
    fn schedule_info_omits_none_fields() {
        let info = ScheduleInfoView {
            schedule_id: "0.0.7001".to_string(),
            payer_account_id: None,
            creator_account_id: Some("0.0.1001".to_string()),
            signatories: vec!["0.0.1001".to_string()],
            scheduled_transaction_id: None,
            status: ScheduleState::PendingSignatures,
            expiration_time: None,
            executed_timestamp: None,
            deletion_timestamp: None,
        };

        let json = serde_json::to_string(&info).expect("schedule info should serialize");

        assert!(json.contains("\"scheduleId\":\"0.0.7001\""));
        assert!(json.contains("\"creatorAccountId\":\"0.0.1001\""));
        assert!(json.contains("\"signatories\":[\"0.0.1001\"]"));
        assert!(json.contains("\"status\":\"pendingSignatures\""));
        assert!(!json.contains("payerAccountId"));
        assert!(!json.contains("scheduledTransactionId"));
        assert!(!json.contains("expirationTime"));
        assert!(!json.contains("executedTimestamp"));
        assert!(!json.contains("deletionTimestamp"));
    }
}
