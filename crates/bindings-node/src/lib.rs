use std::sync::Arc;

use hiero_runtime_core::{
    ContractResultView, MirrorAccountView, RuntimeConfig, RuntimeError, RuntimeErrorCode,
    TransactionPage,
};
use hiero_runtime_mirror::MirrorClient;
use hiero_runtime_schedule::{
    CreateTransferRequest, ScheduleProvider, ScheduleRuntime, SignScheduleRequest,
};
use hiero_runtime_tx::{
    HbarTransferRequest, HbarTransferSubmitter, ReceiptProvider, TxRuntime,
};
use napi::bindgen_prelude::*;
use napi::{Error as NapiError, Status};
use napi_derive::napi;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::json;

// ── Lazy adapters ─────────────────────────────────────────────────────────────
mod lazy {
    use std::sync::Arc;

    use async_trait::async_trait;
    use hiero_runtime_core::{
        CreatedSchedule, ReceiptResult, RuntimeConfig, RuntimeError, ScheduleInfoView,
        SubmittedTransaction,
    };
    use hiero_runtime_schedule::{
        CreateTransferRequest, HieroSdkScheduleProvider, ScheduleProvider, SignScheduleRequest,
    };
    use hiero_runtime_tx::{
        HbarTransferRequest, HbarTransferSubmitter, HieroSdkTxAdapter, ReceiptProvider,
    };
    use tokio::sync::OnceCell;

    // ── LazyTxAdapter ─────────────────────────────────────────────────────────

    /// Deferred receipt provider and transfer submitter.
    ///
    /// `NativeRuntime::create()` does not call into the Hiero SDK at factory
    /// time, keeping the bootstrap cheap. The underlying `HieroSdkTxAdapter`
    /// is constructed on the first receipt query or transfer submission and
    /// reused thereafter.
    pub struct LazyTxAdapter {
        cell: OnceCell<Arc<HieroSdkTxAdapter>>,
        config: RuntimeConfig,
    }

    impl LazyTxAdapter {
        pub fn new(config: RuntimeConfig) -> Self {
            Self {
                cell: OnceCell::new(),
                config,
            }
        }

        async fn adapter(&self) -> Result<Arc<HieroSdkTxAdapter>, RuntimeError> {
            self.cell
                .get_or_try_init(|| async {
                    let adapter = HieroSdkTxAdapter::from_runtime_config(&self.config)?;
                    Ok(Arc::new(adapter))
                })
                .await.cloned()
        }
    }

    // Safety: `OnceCell<Arc<…>>` and `RuntimeConfig` are both `Send + Sync`.
    unsafe impl Send for LazyTxAdapter {}
    unsafe impl Sync for LazyTxAdapter {}

    #[async_trait]
    impl ReceiptProvider for LazyTxAdapter {
        async fn get_receipt(
            &self,
            transaction_id: &str,
        ) -> Result<Option<ReceiptResult>, RuntimeError> {
            let adapter = self.adapter().await?;
            adapter.get_receipt(transaction_id).await
        }
    }

    #[async_trait]
    impl HbarTransferSubmitter for LazyTxAdapter {
        async fn submit_hbar_transfer(
            &self,
            request: &HbarTransferRequest,
        ) -> Result<SubmittedTransaction, RuntimeError> {
            let adapter = self.adapter().await?;
            adapter.submit_hbar_transfer(request).await
        }
    }

    // ── LazyScheduleAdapter ───────────────────────────────────────────────────

    /// Deferred schedule provider.
    ///
    /// Same lazy-init strategy as `LazyTxAdapter`: the Hiero SDK `Client` is
    /// only constructed when a schedule operation is first invoked.
    pub struct LazyScheduleAdapter {
        cell: OnceCell<Arc<HieroSdkScheduleProvider>>,
        config: RuntimeConfig,
    }

    impl LazyScheduleAdapter {
        pub fn new(config: RuntimeConfig) -> Self {
            Self {
                cell: OnceCell::new(),
                config,
            }
        }

        async fn adapter(&self) -> Result<Arc<HieroSdkScheduleProvider>, RuntimeError> {
            self.cell
                .get_or_try_init(|| async {
                    let provider =
                        HieroSdkScheduleProvider::from_runtime_config(&self.config)?;
                    Ok(Arc::new(provider))
                })
                .await.cloned()
        }
    }

    unsafe impl Send for LazyScheduleAdapter {}
    unsafe impl Sync for LazyScheduleAdapter {}

    #[async_trait]
    impl ScheduleProvider for LazyScheduleAdapter {
        async fn create_transfer(
            &self,
            request: &CreateTransferRequest,
        ) -> Result<CreatedSchedule, RuntimeError> {
            let adapter = self.adapter().await?;
            adapter.create_transfer(request).await
        }

        async fn sign(
            &self,
            request: &SignScheduleRequest,
        ) -> Result<ScheduleInfoView, RuntimeError> {
            let adapter = self.adapter().await?;
            adapter.sign(request).await
        }

        async fn get(&self, schedule_id: &str) -> Result<ScheduleInfoView, RuntimeError> {
            let adapter = self.adapter().await?;
            adapter.get(schedule_id).await
        }

        async fn delete(&self, schedule_id: &str) -> Result<(), RuntimeError> {
            let adapter = self.adapter().await?;
            adapter.delete(schedule_id).await
        }
    }
}

use lazy::{LazyScheduleAdapter, LazyTxAdapter};

// ── Metadata ──────────────────────────────────────────────────────────────────

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AddonMetadata {
    package_name: &'static str,
    addon_name: &'static str,
    version: &'static str,
}

// ── Input structs ─────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SubmitHbarTransferInput {
    from_account_id: String,
    to_account_id: String,
    amount_tinybar: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateScheduledTransferInput {
    from_account_id: String,
    to_account_id: String,
    payer_account_id: Option<String>,
    amount_tinybar: String,
    memo: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SignScheduleInput {
    schedule_id: String,
    signer_private_key: String,
}

// ── Exported functions ────────────────────────────────────────────────────────

#[napi]
pub fn get_addon_metadata() -> String {
    let metadata = AddonMetadata {
        package_name: "@hiero-runtime/bindings-node",
        addon_name: "hiero_runtime_bindings_node",
        version: env!("CARGO_PKG_VERSION"),
    };

    serde_json::to_string(&metadata).unwrap_or_else(|_| {
        "{\"packageName\":\"@hiero-runtime/bindings-node\",\"addonName\":\"hiero_runtime_bindings_node\"}".to_string()
    })
}

// ── NativeRuntime ─────────────────────────────────────────────────────────────

#[napi]
pub struct NativeRuntime {
    mirror: MirrorClient,
    tx: TxRuntime,
    schedule: ScheduleRuntime,
}

#[napi]
impl NativeRuntime {
    /// Create a runtime instance.
    #[napi(factory)]
    pub async fn create(config_json: String) -> Result<Self> {
        let config: RuntimeConfig = parse_json_input("config", &config_json)?;

        config
            .validate()
            .map_err(|message| to_napi_error(RuntimeError::invalid_config(message)))?;

        let mirror =
            MirrorClient::new(config.network.mirror_base_url.clone(), config.retry.clone())
                .map_err(to_napi_error)?;

        // Lazy adapters — no Hiero SDK calls here.
        let lazy_tx = Arc::new(LazyTxAdapter::new(config.clone()));
        let lazy_schedule = Arc::new(LazyScheduleAdapter::new(config.clone()));

        let tx = TxRuntime::new_with_submitter(
            mirror.clone(),
            lazy_tx.clone() as Arc<dyn ReceiptProvider>,
            lazy_tx as Arc<dyn HbarTransferSubmitter>,
            config.finality.clone(),
        )
        .map_err(to_napi_error)?;

        let schedule = ScheduleRuntime::new(
            lazy_schedule as Arc<dyn ScheduleProvider>,
            tx.clone(),
            config.finality.clone(),
        )
        .map_err(to_napi_error)?;

        Ok(Self {
            mirror,
            tx,
            schedule,
        })
    }

    #[napi]
    pub async fn get_mirror_transaction(&self, transaction_id: String) -> Result<String> {
        let lookup = self
            .mirror
            .get_transaction(&transaction_id)
            .await
            .map_err(to_napi_error)?;

        serialize_output(&lookup)
    }

    #[napi]
    pub async fn list_transactions_for_account(
        &self,
        account_id: String,
        limit: Option<u32>,
        cursor: Option<String>,
    ) -> Result<String> {
        let page: TransactionPage = self
            .mirror
            .list_transactions_for_account(
                &account_id,
                limit.unwrap_or(25),
                cursor.as_deref(),
            )
            .await
            .map_err(to_napi_error)?;

        serialize_output(&page)
    }

    #[napi]
    pub async fn submit_hbar_transfer(&self, request_json: String) -> Result<String> {
        let input: SubmitHbarTransferInput = parse_json_input("submitHbarTransfer", &request_json)?;

        let amount_tinybar = parse_tinybar_string(&input.amount_tinybar)?;

        let request = HbarTransferRequest {
            from_account_id: input.from_account_id,
            to_account_id: input.to_account_id,
            amount_tinybar,
        };

        let submitted = self
            .tx
            .submit_hbar_transfer(request)
            .await
            .map_err(to_napi_error)?;

        serialize_output(&submitted)
    }

    #[napi]
    pub async fn wait_for_receipt(&self, transaction_id: String) -> Result<String> {
        let receipt = self
            .tx
            .wait_for_receipt(&transaction_id)
            .await
            .map_err(to_napi_error)?;

        serialize_output(&receipt)
    }

    #[napi]
    pub async fn wait_for_finality(&self, transaction_id: String) -> Result<String> {
        let finalized = self
            .tx
            .wait_for_finality(&transaction_id)
            .await
            .map_err(to_napi_error)?;

        serialize_output(&finalized)
    }

    #[napi]
    pub async fn create_scheduled_transfer(&self, request_json: String) -> Result<String> {
        let input: CreateScheduledTransferInput =
            parse_json_input("createScheduledTransfer", &request_json)?;

        let amount_tinybar = parse_tinybar_string(&input.amount_tinybar)?;

        let request = CreateTransferRequest {
            from_account_id: input.from_account_id,
            to_account_id: input.to_account_id,
            payer_account_id: input.payer_account_id,
            amount_tinybar,
            memo: input.memo,
        };

        let created = self
            .schedule
            .create_transfer(request)
            .await
            .map_err(to_napi_error)?;

        serialize_output(&created)
    }

    #[napi]
    pub async fn sign_schedule(&self, request_json: String) -> Result<String> {
        let input: SignScheduleInput = parse_json_input("signSchedule", &request_json)?;

        let request = SignScheduleRequest {
            schedule_id: input.schedule_id,
            signer_private_key: input.signer_private_key,
        };

        let info = self.schedule.sign(request).await.map_err(to_napi_error)?;

        serialize_output(&info)
    }

    #[napi]
    pub async fn get_schedule(&self, schedule_id: String) -> Result<String> {
        let info = self
            .schedule
            .get(&schedule_id)
            .await
            .map_err(to_napi_error)?;

        serialize_output(&info)
    }

    #[napi]
    pub async fn wait_for_schedule_execution(&self, schedule_id: String) -> Result<String> {
        let execution = self
            .schedule
            .wait_for_execution(&schedule_id)
            .await
            .map_err(to_napi_error)?;

        serialize_output(&execution)
    }

    #[napi]
    pub async fn get_mirror_account(&self, id: String) -> Result<String> {
        let account: MirrorAccountView = self
            .mirror
            .get_account(&id)
            .await
            .map_err(to_napi_error)?;

        serialize_output(&account)
    }

    #[napi]
    pub async fn get_contract_result(
        &self,
        transaction_id_or_hash: String,
        nonce: Option<i32>,
    ) -> Result<String> {
        let result: ContractResultView = self
            .mirror
            .get_contract_result(&transaction_id_or_hash, nonce)
            .await
            .map_err(to_napi_error)?;

        serialize_output(&result)
    }

    #[napi]
    pub async fn delete_schedule(&self, schedule_id: String) -> Result<()> {
        self.schedule
            .delete(&schedule_id)
            .await
            .map_err(to_napi_error)
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn parse_tinybar_string(value: &str) -> Result<u64> {
    value.trim().parse::<u64>().map_err(|_| {
        to_napi_error(RuntimeError::with_details(
            RuntimeErrorCode::InvalidConfig,
            format!("amountTinybar must be a valid positive integer string, got: {value}"),
            json!({ "amountTinybar": value }),
        ))
    })
}

fn parse_json_input<T>(input_name: &str, raw: &str) -> Result<T>
where
    T: DeserializeOwned,
{
    serde_json::from_str::<T>(raw).map_err(|err| {
        to_napi_error(RuntimeError::with_details(
            RuntimeErrorCode::InvalidConfig,
            format!("invalid JSON for {input_name}: {err}"),
            json!({
                "input": input_name
            }),
        ))
    })
}

fn serialize_output<T>(value: &T) -> Result<String>
where
    T: Serialize,
{
    serde_json::to_string(value).map_err(|err| {
        to_napi_error(RuntimeError::with_details(
            RuntimeErrorCode::Serialization,
            format!("failed to serialize native output: {err}"),
            json!({}),
        ))
    })
}

fn to_napi_error(err: RuntimeError) -> NapiError {
    let payload = serde_json::to_string(&err).unwrap_or_else(|_| {
        "{\"code\":\"INTERNAL\",\"message\":\"failed to serialize runtime error\",\"retryable\":false}".to_string()
    });

    NapiError::new(Status::GenericFailure, payload)
}
