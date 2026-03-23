use hiero_runtime_core::{FinalityPolicy, RetryPolicy, RuntimeErrorCode};
use hiero_runtime_mirror::MirrorClient;
use serde_json::json;
use wiremock::{
    matchers::{method, path},
    Mock, MockServer, ResponseTemplate,
};

fn test_retry_policy() -> RetryPolicy {
    RetryPolicy {
        max_attempts: 3,
        initial_delay_ms: 10,
        max_delay_ms: 20,
        jitter: false,
    }
}

fn test_finality_policy() -> FinalityPolicy {
    FinalityPolicy {
        receipt_timeout_ms: 500,
        mirror_timeout_ms: 120,
        poll_interval_ms: 20,
    }
}

const TX_ID: &str = "0.0.1001@1719943901.123456789";
const TX_PATH: &str = "/api/v1/transactions/0.0.1001-1719943901-123456789";

#[tokio::test]
async fn get_transaction_normalizes_primary_and_duplicates() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path(TX_PATH))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "transactions": [
                {
                    "transaction_id": TX_ID,
                    "result": "SUCCESS",
                    "consensus_timestamp": "1719943901.111111111",
                    "name": "CRYPTOTRANSFER"
                },
                {
                    "transaction_id": TX_ID,
                    "result": "DUPLICATE_TRANSACTION",
                    "consensus_timestamp": "1719943901.222222222",
                    "name": "CRYPTOTRANSFER"
                }
            ]
        })))
        .mount(&server)
        .await;

    let client = MirrorClient::new(server.uri(), test_retry_policy()).expect("client must build");
    let lookup = client
        .get_transaction(TX_ID)
        .await
        .expect("transaction lookup should succeed");

    assert_eq!(lookup.requested_transaction_id, TX_ID);
    assert_eq!(lookup.primary.result, "SUCCESS");
    assert_eq!(
        lookup.primary.consensus_timestamp.as_deref(),
        Some("1719943901.111111111")
    );
    assert_eq!(lookup.duplicates.len(), 1);
    assert_eq!(lookup.duplicates[0].result, "DUPLICATE_TRANSACTION");
    assert_eq!(lookup.entries.len(), 2);
}

#[tokio::test]
async fn get_transaction_retries_on_rate_limit_then_succeeds() {
    let server = MockServer::start().await;

    // First request returns 429 (priority 1 = highest, matches once only)
    Mock::given(method("GET"))
        .and(path(TX_PATH))
        .respond_with(ResponseTemplate::new(429).set_body_string("rate limited"))
        .up_to_n_times(1)
        .with_priority(1)
        .mount(&server)
        .await;

    // Subsequent requests return 200 (priority 5 = default, matches always)
    Mock::given(method("GET"))
        .and(path(TX_PATH))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "transactions": [
                {
                    "transaction_id": TX_ID,
                    "result": "SUCCESS",
                    "consensus_timestamp": "1719943901.111111111",
                    "name": "CRYPTOTRANSFER"
                }
            ]
        })))
        .with_priority(5)
        .mount(&server)
        .await;

    let client = MirrorClient::new(server.uri(), test_retry_policy()).expect("client must build");
    let lookup = client
        .get_transaction(TX_ID)
        .await
        .expect("client should retry and succeed");

    assert_eq!(lookup.primary.result, "SUCCESS");
    let requests = server
        .received_requests()
        .await
        .expect("request recording enabled");
    assert_eq!(requests.len(), 2);
}

#[tokio::test]
async fn wait_for_transaction_polls_until_visible_after_not_found() {
    let server = MockServer::start().await;

    // First 2 requests return 404 (priority 1 = highest, matches twice only)
    Mock::given(method("GET"))
        .and(path(TX_PATH))
        .respond_with(ResponseTemplate::new(404).set_body_string("not found"))
        .up_to_n_times(2)
        .with_priority(1)
        .mount(&server)
        .await;

    // Third+ requests return 200 (priority 5 = default, matches always)
    Mock::given(method("GET"))
        .and(path(TX_PATH))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "transactions": [
                {
                    "transaction_id": TX_ID,
                    "result": "SUCCESS",
                    "consensus_timestamp": "1719943901.333333333",
                    "name": "CRYPTOTRANSFER"
                }
            ]
        })))
        .with_priority(5)
        .mount(&server)
        .await;

    let client = MirrorClient::new(server.uri(), test_retry_policy()).expect("client must build");
    let lookup = client
        .wait_for_transaction(TX_ID, &test_finality_policy())
        .await
        .expect("polling should eventually succeed");

    assert_eq!(lookup.primary.result, "SUCCESS");
    let requests = server
        .received_requests()
        .await
        .expect("request recording enabled");
    assert!(requests.len() >= 3);
}

#[tokio::test]
async fn wait_for_transaction_times_out_when_never_visible() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path(TX_PATH))
        .respond_with(ResponseTemplate::new(404).set_body_string("not found"))
        .mount(&server)
        .await;

    let client = MirrorClient::new(server.uri(), test_retry_policy()).expect("client must build");
    let err = client
        .wait_for_transaction(TX_ID, &test_finality_policy())
        .await
        .expect_err("missing transaction should time out");

    assert_eq!(err.code, RuntimeErrorCode::Timeout);
    assert!(err.message.contains("not visible within"));
}
