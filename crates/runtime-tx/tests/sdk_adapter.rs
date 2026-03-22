use std::sync::Arc;

use hiero_runtime_core::{
    FinalityPolicy, NetworkConfig, NetworkKind, RetryPolicy, RuntimeConfig, RuntimeErrorCode,
};
use hiero_runtime_mirror::MirrorClient;
use hiero_runtime_tx::{
    HbarTransferRequest, HbarTransferSubmitter, HieroSdkTxAdapter, ReceiptProvider, TxRuntime,
};

fn config_without_operator() -> RuntimeConfig {
    RuntimeConfig {
        network: NetworkConfig {
            kind: NetworkKind::Testnet,
            mirror_base_url: "https://testnet.mirrornode.hedera.com".to_string(),
            consensus_nodes: None,
        },
        operator: None,
        retry: RetryPolicy {
            max_attempts: 1,
            initial_delay_ms: 1,
            max_delay_ms: 1,
            jitter: false,
        },
        finality: FinalityPolicy {
            receipt_timeout_ms: 10,
            mirror_timeout_ms: 10,
            poll_interval_ms: 1,
        },
    }
}

#[tokio::test]
async fn sdk_submitter_requires_operator_for_hbar_transfer_submission() {
    let adapter = HieroSdkTxAdapter::from_runtime_config(&config_without_operator())
        .expect("adapter should build without operator for receipt-only use");

    let err = adapter
        .submit_hbar_transfer(&HbarTransferRequest {
            from_account_id: "0.0.1001".to_string(),
            to_account_id: "0.0.1002".to_string(),
            amount_tinybar: 1,
        })
        .await
        .expect_err("submission without operator must fail");

    assert_eq!(err.code, RuntimeErrorCode::InvalidConfig);
    assert!(err.message.contains("operator credentials"));
}

#[tokio::test]
async fn tx_runtime_submit_surfaces_real_submitter_error() {
    let config = config_without_operator();
    let mirror = MirrorClient::new(config.network.mirror_base_url.clone(), config.retry.clone())
        .expect("mirror client should build");

    let adapter =
        Arc::new(HieroSdkTxAdapter::from_runtime_config(&config).expect("adapter should build"));

    let receipt_provider: Arc<dyn ReceiptProvider> = adapter.clone();
    let submitter: Arc<dyn HbarTransferSubmitter> = adapter;

    let runtime =
        TxRuntime::new_with_submitter(mirror, receipt_provider, submitter, config.finality.clone())
            .expect("runtime should build");

    let err = runtime
        .submit_hbar_transfer(HbarTransferRequest {
            from_account_id: "0.0.1001".to_string(),
            to_account_id: "0.0.1002".to_string(),
            amount_tinybar: 1,
        })
        .await
        .expect_err("runtime submission should fail without operator");

    assert_eq!(err.code, RuntimeErrorCode::InvalidConfig);
}
