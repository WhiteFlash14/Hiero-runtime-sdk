use std::sync::Arc;
use std::time::{Duration, Instant};

use hiero_runtime_core::{
    CreatedSchedule, FinalityPolicy, RuntimeError, RuntimeErrorCode, ScheduleExecution,
    ScheduleInfoView, ScheduleState,
};
use hiero_runtime_tx::TxRuntime;

use crate::provider::ScheduleProvider;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateTransferRequest {
    pub from_account_id: String,
    pub to_account_id: String,
    pub payer_account_id: Option<String>,
    pub amount_tinybar: u64,
    pub memo: Option<String>,
}

impl CreateTransferRequest {
    pub fn validate(&self) -> Result<(), RuntimeError> {
        if self.from_account_id.trim().is_empty() {
            return Err(RuntimeError::invalid_config(
                "fromAccountId must not be empty",
            ));
        }

        if self.to_account_id.trim().is_empty() {
            return Err(RuntimeError::invalid_config(
                "toAccountId must not be empty",
            ));
        }

        if let Some(payer) = &self.payer_account_id {
            if payer.trim().is_empty() {
                return Err(RuntimeError::invalid_config(
                    "payerAccountId must not be empty when provided",
                ));
            }
        }

        if self.amount_tinybar == 0 {
            return Err(RuntimeError::invalid_config(
                "amountTinybar must be greater than zero",
            ));
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignScheduleRequest {
    pub schedule_id: String,
    pub signer_private_key: String,
}

impl SignScheduleRequest {
    pub fn validate(&self) -> Result<(), RuntimeError> {
        if self.schedule_id.trim().is_empty() {
            return Err(RuntimeError::invalid_config("scheduleId must not be empty"));
        }

        if self.signer_private_key.trim().is_empty() {
            return Err(RuntimeError::invalid_config(
                "signerPrivateKey must not be empty",
            ));
        }

        Ok(())
    }
}

#[derive(Clone)]
pub struct ScheduleRuntime {
    inner: Arc<ScheduleRuntimeInner>,
}

struct ScheduleRuntimeInner {
    provider: Arc<dyn ScheduleProvider>,
    tx_runtime: TxRuntime,
    wait_policy: FinalityPolicy,
}

impl ScheduleRuntime {
    pub fn new(
        provider: Arc<dyn ScheduleProvider>,
        tx_runtime: TxRuntime,
        wait_policy: FinalityPolicy,
    ) -> Result<Self, RuntimeError> {
        wait_policy
            .validate()
            .map_err(RuntimeError::invalid_config)?;

        Ok(Self {
            inner: Arc::new(ScheduleRuntimeInner {
                provider,
                tx_runtime,
                wait_policy,
            }),
        })
    }

    pub async fn create_transfer(
        &self,
        request: CreateTransferRequest,
    ) -> Result<CreatedSchedule, RuntimeError> {
        request.validate()?;
        self.inner.provider.create_transfer(&request).await
    }

    pub async fn sign(
        &self,
        request: SignScheduleRequest,
    ) -> Result<ScheduleInfoView, RuntimeError> {
        request.validate()?;
        self.inner.provider.sign(&request).await
    }

    pub async fn get(&self, schedule_id: &str) -> Result<ScheduleInfoView, RuntimeError> {
        validate_schedule_id(schedule_id)?;
        self.inner.provider.get(schedule_id).await
    }

    pub async fn delete(&self, schedule_id: &str) -> Result<(), RuntimeError> {
        validate_schedule_id(schedule_id)?;
        self.inner.provider.delete(schedule_id).await
    }

    pub async fn wait_for_execution(
        &self,
        schedule_id: &str,
    ) -> Result<ScheduleExecution, RuntimeError> {
        validate_schedule_id(schedule_id)?;

        let started = Instant::now();

        loop {
            match self.inner.provider.get(schedule_id).await {
                Ok(info) => match info.status {
                    ScheduleState::PendingSignatures => {
                        if started.elapsed()
                            >= Duration::from_millis(self.inner.wait_policy.mirror_timeout_ms)
                        {
                            return Err(RuntimeError::new(
                                RuntimeErrorCode::Timeout,
                                format!(
                                    "schedule {schedule_id} did not execute within {}ms",
                                    self.inner.wait_policy.mirror_timeout_ms
                                ),
                            ));
                        }

                        tokio::time::sleep(Duration::from_millis(
                            self.inner.wait_policy.poll_interval_ms,
                        ))
                        .await;
                    }
                    ScheduleState::Executed => {
                        let scheduled_transaction_id =
                            info.scheduled_transaction_id.clone().ok_or_else(|| {
                                RuntimeError::new(
                                    RuntimeErrorCode::Schedule,
                                    format!(
                                        "schedule {schedule_id} is executed but missing scheduledTransactionId"
                                    ),
                                )
                            })?;

                        let finalized = self
                            .inner
                            .tx_runtime
                            .wait_for_finality(&scheduled_transaction_id)
                            .await?;

                        return Ok(ScheduleExecution {
                            schedule_id: schedule_id.to_string(),
                            scheduled_transaction_id,
                            finalized,
                        });
                    }
                    ScheduleState::Expired => {
                        return Err(RuntimeError::new(
                            RuntimeErrorCode::Schedule,
                            format!("schedule {schedule_id} expired before execution"),
                        ));
                    }
                    ScheduleState::Deleted => {
                        return Err(RuntimeError::new(
                            RuntimeErrorCode::Schedule,
                            format!("schedule {schedule_id} was deleted before execution"),
                        ));
                    }
                },
                Err(err) if err.code == RuntimeErrorCode::NotFound || err.is_retryable() => {
                    if started.elapsed()
                        >= Duration::from_millis(self.inner.wait_policy.mirror_timeout_ms)
                    {
                        return Err(RuntimeError::new(
                            RuntimeErrorCode::Timeout,
                            format!(
                                "schedule {schedule_id} did not become observable within {}ms",
                                self.inner.wait_policy.mirror_timeout_ms
                            ),
                        ));
                    }

                    tokio::time::sleep(Duration::from_millis(
                        self.inner.wait_policy.poll_interval_ms,
                    ))
                    .await;
                }
                Err(err) => return Err(err),
            }
        }
    }
}

fn validate_schedule_id(schedule_id: &str) -> Result<(), RuntimeError> {
    if schedule_id.trim().is_empty() {
        return Err(RuntimeError::invalid_config("scheduleId must not be empty"));
    }

    Ok(())
}
