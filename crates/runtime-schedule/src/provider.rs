use async_trait::async_trait;
use hiero_runtime_core::{CreatedSchedule, RuntimeError, ScheduleInfoView};

use crate::runtime::{CreateTransferRequest, SignScheduleRequest};

#[async_trait]
pub trait ScheduleProvider: Send + Sync + 'static {
    async fn create_transfer(
        &self,
        request: &CreateTransferRequest,
    ) -> Result<CreatedSchedule, RuntimeError>;

    async fn sign(&self, request: &SignScheduleRequest) -> Result<ScheduleInfoView, RuntimeError>;

    async fn get(&self, schedule_id: &str) -> Result<ScheduleInfoView, RuntimeError>;

    async fn delete(&self, schedule_id: &str) -> Result<(), RuntimeError>;
}
