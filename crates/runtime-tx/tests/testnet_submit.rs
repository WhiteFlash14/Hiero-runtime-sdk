use std::env;
use std::sync::Arc;

use hiero_runtime_core::{
    FinalityPolicy, NetworkConfig, NetworkKind, OperatorConfig, RetryPolicy, RuntimeConfig,
};
use hiero_runtime_mirror::MirrorClient;
use hiero_runtime_tx::{
    HbarTransferRequest, HbarTransferSubmitter, HieroSdkTxAdapter, ReceiptProvider, TxRuntime,
};

#[tokio::test]
#[ignore = "requires HIERO_TESTNET_OPERATOR_ID, HIERO_TESTNET_OPERATOR_KEY, and HIERO_TESTNET_RECEIVER_ID against live testnet"]
async fn submit_hbar_transfer_reaches_finality_on_testnet() {
    let operator_id =
        env::var("HIERO_TESTNET_OPERATOR_ID").expect("HIERO_TESTNET_OPERATOR_ID must be set");
    let operator_key =
        env::var("HIERO_TESTNET_OPERATOR_KEY").expect("HIERO_TESTNET_OPERATOR_KEY must be set");
    let receiver_id =
        env::var("HIERO_TESTNET_RECEIVER_ID").expect("HIERO_TESTNET_RECEIVER_ID must be set");

    let config = RuntimeConfig {
        network: NetworkConfig {
            kind: NetworkKind::Testnet,
            mirror_base_url: "https://testnet.mirrornode.hedera.com".to_string(),
            consensus_nodes: None,
        },
        operator: Some(OperatorConfig {
            account_id: operator_id.clone(),
            private_key: operator_key,
        }),
        retry: RetryPolicy {
            max_attempts: 3,
            initial_delay_ms: 250,
            max_delay_ms: 1000,
            jitter: false,
        },
        finality: FinalityPolicy {
            receipt_timeout_ms: 60_000,
            mirror_timeout_ms: 60_000,
            poll_interval_ms: 1_000,
        },
    };

    let mirror = MirrorClient::new(config.network.mirror_base_url.clone(), config.retry.clone())
        .expect("mirror client should build");

    let adapter = Arc::new(
        HieroSdkTxAdapter::from_runtime_config(&config).expect("sdk adapter should build"),
    );

    let receipt_provider: Arc<dyn ReceiptProvider> = adapter.clone();
    let submitter: Arc<dyn HbarTransferSubmitter> = adapter;

    let runtime =
        TxRuntime::new_with_submitter(mirror, receipt_provider, submitter, config.finality.clone())
            .expect("tx runtime should build");

    let submitted = runtime
        .submit_hbar_transfer(HbarTransferRequest {
            from_account_id: operator_id,
            to_account_id: receiver_id,
            amount_tinybar: 1,
        })
        .await
        .expect("submission should succeed");

    let finalized = runtime
        .wait_for_finality(&submitted.transaction_id)
        .await
        .expect("finality should succeed");

    assert_eq!(finalized.receipt.status, "SUCCESS");
    assert!(finalized.primary_mirror_entry.is_some());
}
