use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use hiero_runtime_core::{
    CreatedSchedule, FinalityPolicy, ReceiptResult, RetryPolicy, RuntimeError, RuntimeErrorCode,
    ScheduleInfoView, ScheduleState,
};
use hiero_runtime_mirror::MirrorClient;
use hiero_runtime_schedule::{
    CreateTransferRequest, ScheduleProvider, ScheduleRuntime, SignScheduleRequest,
};
use hiero_runtime_tx::{ReceiptProvider, TxRuntime};
use serde_json::json;
use wiremock::{
    matchers::{method, path, query_param},
    Mock, MockServer, ResponseTemplate,
};

#[derive(Clone)]
enum ReceiptStep {
    Ready {
        transaction_id: String,
        status: String,
    },
}

struct ScriptedReceiptProvider {
    steps: Mutex<VecDeque<ReceiptStep>>,
}

impl ScriptedReceiptProvider {
    fn new(steps: Vec<ReceiptStep>) -> Self {
        Self {
            steps: Mutex::new(VecDeque::from(steps)),
        }
    }
}

#[async_trait]
impl ReceiptProvider for ScriptedReceiptProvider {
    async fn get_receipt(
        &self,
        _transaction_id: &str,
    ) -> Result<Option<ReceiptResult>, RuntimeError> {
        let next = {
            let mut guard = self.steps.lock().expect("receipt steps mutex poisoned");
            guard.pop_front()
        };

        match next {
            Some(ReceiptStep::Ready {
                transaction_id,
                status,
            }) => Ok(Some(ReceiptResult {
                transaction_id,
                status,
            })),
            None => Ok(None),
        }
    }
}

#[derive(Clone)]
#[allow(dead_code)]
enum ScheduleStep {
    Info(ScheduleInfoView),
    Error(RuntimeError),
}

struct ScriptedScheduleProvider {
    created: CreatedSchedule,
    sign_result: ScheduleInfoView,
    get_steps: Mutex<VecDeque<ScheduleStep>>,
}

impl ScriptedScheduleProvider {
    fn new(
        created: CreatedSchedule,
        sign_result: ScheduleInfoView,
        get_steps: Vec<ScheduleStep>,
    ) -> Self {
        Self {
            created,
            sign_result,
            get_steps: Mutex::new(VecDeque::from(get_steps)),
        }
    }
}

#[async_trait]
impl ScheduleProvider for ScriptedScheduleProvider {
    async fn create_transfer(
        &self,
        _request: &CreateTransferRequest,
    ) -> Result<CreatedSchedule, RuntimeError> {
        Ok(self.created.clone())
    }

    async fn sign(&self, _request: &SignScheduleRequest) -> Result<ScheduleInfoView, RuntimeError> {
        Ok(self.sign_result.clone())
    }

    async fn get(&self, _schedule_id: &str) -> Result<ScheduleInfoView, RuntimeError> {
        let next = {
            let mut guard = self
                .get_steps
                .lock()
                .expect("schedule steps mutex poisoned");
            guard.pop_front()
        };

        match next.unwrap_or_else(|| ScheduleStep::Info(self.sign_result.clone())) {
            ScheduleStep::Info(info) => Ok(info),
            ScheduleStep::Error(err) => Err(err),
        }
    }

    async fn delete(&self, _schedule_id: &str) -> Result<(), RuntimeError> {
        Ok(())
    }
}

fn retry_policy() -> RetryPolicy {
    RetryPolicy {
        max_attempts: 2,
        initial_delay_ms: 5,
        max_delay_ms: 10,
        jitter: false,
    }
}

fn policy() -> FinalityPolicy {
    FinalityPolicy {
        receipt_timeout_ms: 120,
        mirror_timeout_ms: 160,
        poll_interval_ms: 20,
    }
}

fn schedule_info(
    status: ScheduleState,
    scheduled_transaction_id: Option<&str>,
) -> ScheduleInfoView {
    ScheduleInfoView {
        schedule_id: "0.0.7001".to_string(),
        payer_account_id: Some("0.0.1001".to_string()),
        creator_account_id: Some("0.0.1001".to_string()),
        signatories: vec!["0.0.1001".to_string()],
        scheduled_transaction_id: scheduled_transaction_id.map(str::to_string),
        status,
        expiration_time: Some("1719944000.000000000".to_string()),
        executed_timestamp: match status {
            ScheduleState::Executed => Some("1719943902.000000000".to_string()),
            _ => None,
        },
        deletion_timestamp: None,
    }
}

fn build_tx_runtime(
    mirror_base_url: String,
    receipt_provider: Arc<dyn ReceiptProvider>,
) -> TxRuntime {
    let mirror =
        MirrorClient::new(mirror_base_url, retry_policy()).expect("mirror client should build");

    TxRuntime::new(mirror, receipt_provider, policy()).expect("tx runtime should build")
}

#[tokio::test]
async fn create_transfer_and_sign_pass_through_provider() {
    let created = CreatedSchedule {
        schedule_id: "0.0.7001".to_string(),
        scheduled_transaction_id: "0.0.1001@1719943901.123456789?scheduled".to_string(),
        status: ScheduleState::PendingSignatures,
    };
    let sign_result = schedule_info(
        ScheduleState::PendingSignatures,
        Some("0.0.1001@1719943901.123456789?scheduled"),
    );

    let schedule_provider = Arc::new(ScriptedScheduleProvider::new(
        created.clone(),
        sign_result.clone(),
        vec![ScheduleStep::Info(sign_result.clone())],
    ));

    let receipt_provider = Arc::new(ScriptedReceiptProvider::new(vec![]));
    let tx_runtime = build_tx_runtime("http://127.0.0.1:9".to_string(), receipt_provider);
    let runtime = ScheduleRuntime::new(schedule_provider, tx_runtime, policy())
        .expect("runtime should build");

    let created_out = runtime
        .create_transfer(CreateTransferRequest {
            from_account_id: "0.0.1001".to_string(),
            to_account_id: "0.0.1002".to_string(),
            payer_account_id: Some("0.0.1001".to_string()),
            amount_tinybar: 100,
            memo: Some("ops payout".to_string()),
        })
        .await
        .expect("create_transfer should succeed");

    let sign_out = runtime
        .sign(SignScheduleRequest {
            schedule_id: "0.0.7001".to_string(),
            signer_private_key: "302e...".to_string(),
        })
        .await
        .expect("sign should succeed");

    assert_eq!(created_out, created);
    assert_eq!(sign_out.status, ScheduleState::PendingSignatures);
    assert_eq!(
        sign_out.scheduled_transaction_id.as_deref(),
        Some("0.0.1001@1719943901.123456789?scheduled")
    );
}

#[tokio::test]
async fn get_returns_schedule_info() {
    let created = CreatedSchedule {
        schedule_id: "0.0.7001".to_string(),
        scheduled_transaction_id: "0.0.1001@1719943901.123456789?scheduled".to_string(),
        status: ScheduleState::PendingSignatures,
    };
    let info = schedule_info(
        ScheduleState::PendingSignatures,
        Some("0.0.1001@1719943901.123456789?scheduled"),
    );

    let schedule_provider = Arc::new(ScriptedScheduleProvider::new(
        created,
        info.clone(),
        vec![ScheduleStep::Info(info.clone())],
    ));

    let receipt_provider = Arc::new(ScriptedReceiptProvider::new(vec![]));
    let tx_runtime = build_tx_runtime("http://127.0.0.1:9".to_string(), receipt_provider);
    let runtime = ScheduleRuntime::new(schedule_provider, tx_runtime, policy())
        .expect("runtime should build");

    let out = runtime.get("0.0.7001").await.expect("get should succeed");

    assert_eq!(out.schedule_id, "0.0.7001");
    assert_eq!(out.status, ScheduleState::PendingSignatures);
}

#[tokio::test]
async fn wait_for_execution_polls_schedule_and_then_uses_tx_runtime_finality() {
    let server = MockServer::start().await;
    let scheduled_tx_id = "0.0.1001@1719943901.123456789?scheduled";
    let mirror_base_path = "/api/v1/transactions/0.0.1001-1719943901-123456789";

    Mock::given(method("GET"))
        .and(path(mirror_base_path))
        .and(query_param("scheduled", "true"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "transactions": [
                {
                    "transaction_id": scheduled_tx_id,
                    "result": "SUCCESS",
                    "consensus_timestamp": "1719943902.000000000",
                    "name": "CRYPTOTRANSFER"
                }
            ]
        })))
        .mount(&server)
        .await;

    let created = CreatedSchedule {
        schedule_id: "0.0.7001".to_string(),
        scheduled_transaction_id: scheduled_tx_id.to_string(),
        status: ScheduleState::PendingSignatures,
    };
    let pending = schedule_info(ScheduleState::PendingSignatures, Some(scheduled_tx_id));
    let executed = schedule_info(ScheduleState::Executed, Some(scheduled_tx_id));

    let schedule_provider = Arc::new(ScriptedScheduleProvider::new(
        created,
        pending.clone(),
        vec![ScheduleStep::Info(pending), ScheduleStep::Info(executed)],
    ));

    let receipt_provider = Arc::new(ScriptedReceiptProvider::new(vec![ReceiptStep::Ready {
        transaction_id: scheduled_tx_id.to_string(),
        status: "SUCCESS".to_string(),
    }]));
    let tx_runtime = build_tx_runtime(server.uri(), receipt_provider);
    let runtime = ScheduleRuntime::new(schedule_provider, tx_runtime, policy())
        .expect("runtime should build");

    let execution = runtime
        .wait_for_execution("0.0.7001")
        .await
        .expect("schedule execution should resolve");

    assert_eq!(execution.schedule_id, "0.0.7001");
    assert_eq!(execution.scheduled_transaction_id, scheduled_tx_id);
    assert_eq!(execution.finalized.receipt.status, "SUCCESS");
    assert_eq!(
        execution
            .finalized
            .primary_mirror_entry
            .as_ref()
            .expect("primary mirror entry should exist")
            .result,
        "SUCCESS"
    );
}

#[tokio::test]
async fn wait_for_execution_fails_when_schedule_expires() {
    let created = CreatedSchedule {
        schedule_id: "0.0.7001".to_string(),
        scheduled_transaction_id: "0.0.1001@1719943901.123456789?scheduled".to_string(),
        status: ScheduleState::PendingSignatures,
    };
    let expired = schedule_info(
        ScheduleState::Expired,
        Some("0.0.1001@1719943901.123456789?scheduled"),
    );

    let schedule_provider = Arc::new(ScriptedScheduleProvider::new(
        created,
        expired.clone(),
        vec![ScheduleStep::Info(expired)],
    ));

    let receipt_provider = Arc::new(ScriptedReceiptProvider::new(vec![]));
    let tx_runtime = build_tx_runtime("http://127.0.0.1:9".to_string(), receipt_provider);
    let runtime = ScheduleRuntime::new(schedule_provider, tx_runtime, policy())
        .expect("runtime should build");

    let err = runtime
        .wait_for_execution("0.0.7001")
        .await
        .expect_err("expired schedule should fail");

    assert_eq!(err.code, RuntimeErrorCode::Schedule);
    assert!(err.message.contains("expired before execution"));
}
