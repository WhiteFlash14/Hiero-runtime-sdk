use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use hiero_runtime_core::{
    FinalityPolicy, ReceiptResult, RetryPolicy, RuntimeError, RuntimeErrorCode,
};
use hiero_runtime_mirror::MirrorClient;
use hiero_runtime_tx::{ReceiptProvider, TxRuntime};
use serde_json::json;
use wiremock::{
    matchers::{method, path},
    Mock, MockServer, ResponseTemplate,
};

#[derive(Clone)]
enum ReceiptStep {
    PendingNone,
    PendingUnknown,
    Ready {
        transaction_id: String,
        status: String,
    },
    Error(RuntimeError),
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
        transaction_id: &str,
    ) -> Result<Option<ReceiptResult>, RuntimeError> {
        let next = {
            let mut guard = self.steps.lock().expect("receipt steps mutex poisoned");
            guard.pop_front()
        };

        match next.unwrap_or(ReceiptStep::PendingNone) {
            ReceiptStep::PendingNone => Ok(None),
            ReceiptStep::PendingUnknown => Ok(Some(ReceiptResult {
                transaction_id: transaction_id.to_string(),
                status: "UNKNOWN".to_string(),
            })),
            ReceiptStep::Ready {
                transaction_id,
                status,
            } => Ok(Some(ReceiptResult {
                transaction_id,
                status,
            })),
            ReceiptStep::Error(err) => Err(err),
        }
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

fn finality_policy() -> FinalityPolicy {
    FinalityPolicy {
        receipt_timeout_ms: 120,
        mirror_timeout_ms: 120,
        poll_interval_ms: 20,
    }
}

fn build_runtime(mirror_base_url: String, provider: Arc<dyn ReceiptProvider>) -> TxRuntime {
    let mirror =
        MirrorClient::new(mirror_base_url, retry_policy()).expect("mirror client should build");

    TxRuntime::new(mirror, provider, finality_policy()).expect("tx runtime should build")
}

#[tokio::test]
async fn wait_for_receipt_polls_until_non_unknown_receipt_exists() {
    let provider = Arc::new(ScriptedReceiptProvider::new(vec![
        ReceiptStep::PendingNone,
        ReceiptStep::PendingUnknown,
        ReceiptStep::Ready {
            transaction_id: "0.0.1001@1719943901.123456789".to_string(),
            status: "SUCCESS".to_string(),
        },
    ]));

    let runtime = build_runtime("http://127.0.0.1:9".to_string(), provider);

    let receipt = runtime
        .wait_for_receipt("0.0.1001@1719943901.123456789")
        .await
        .expect("receipt should eventually resolve");

    assert_eq!(receipt.transaction_id, "0.0.1001@1719943901.123456789");
    assert_eq!(receipt.status, "SUCCESS");
}

#[tokio::test]
async fn attach_returns_handle_that_waits_for_finality() {
    let server = MockServer::start().await;
    let tx_id = "0.0.1001@1719943901.123456789";
    let mirror_path = "/api/v1/transactions/0.0.1001-1719943901-123456789";

    Mock::given(method("GET"))
        .and(path(mirror_path))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "transactions": [
                {
                    "transaction_id": tx_id,
                    "result": "SUCCESS",
                    "consensus_timestamp": "1719943901.999999999",
                    "name": "CRYPTOTRANSFER"
                },
                {
                    "transaction_id": tx_id,
                    "result": "DUPLICATE_TRANSACTION",
                    "consensus_timestamp": "1719943902.000000000",
                    "name": "CRYPTOTRANSFER"
                }
            ]
        })))
        .mount(&server)
        .await;

    let provider = Arc::new(ScriptedReceiptProvider::new(vec![ReceiptStep::Ready {
        transaction_id: tx_id.to_string(),
        status: "SUCCESS".to_string(),
    }]));

    let runtime = build_runtime(server.uri(), provider);
    let handle = runtime.attach(tx_id).expect("attach should succeed");

    let finalized = handle
        .wait_for_finality()
        .await
        .expect("finality should resolve");

    assert_eq!(handle.transaction_id(), tx_id);
    assert_eq!(finalized.transaction_id, tx_id);
    assert_eq!(finalized.receipt.status, "SUCCESS");
    assert_eq!(
        finalized
            .primary_mirror_entry
            .as_ref()
            .expect("primary mirror entry should exist")
            .result,
        "SUCCESS"
    );
    assert_eq!(finalized.duplicates.len(), 1);
    assert_eq!(finalized.duplicates[0].result, "DUPLICATE_TRANSACTION");
}

#[tokio::test]
async fn wait_for_receipt_times_out_when_provider_never_resolves() {
    let provider = Arc::new(ScriptedReceiptProvider::new(vec![]));
    let runtime = build_runtime("http://127.0.0.1:9".to_string(), provider);

    let err = runtime
        .wait_for_receipt("0.0.1001@1719943901.123456789")
        .await
        .expect_err("receipt polling should time out");

    assert_eq!(err.code, RuntimeErrorCode::Timeout);
    assert!(err.message.contains("not available within"));
}

#[tokio::test]
async fn wait_for_receipt_surfaces_non_retryable_errors() {
    let provider = Arc::new(ScriptedReceiptProvider::new(vec![ReceiptStep::Error(
        RuntimeError::invalid_config("bad receipt provider input"),
    )]));
    let runtime = build_runtime("http://127.0.0.1:9".to_string(), provider);

    let err = runtime
        .wait_for_receipt("0.0.1001@1719943901.123456789")
        .await
        .expect_err("non-retryable provider errors must surface immediately");

    assert_eq!(err.code, RuntimeErrorCode::InvalidConfig);
    assert_eq!(err.message, "bad receipt provider input");
}
