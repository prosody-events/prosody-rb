use crate::bridge::{Bridge, BridgeError};
use crate::result::result_channel;
use crate::scheduler::handle::TaskHandle;
use crate::scheduler::processor::RubyProcessor;
use magnus::{Error, Ruby};
use thiserror::Error;
use tracing::error;

mod cancellation;
mod handle;
mod processor;

#[derive(Clone, Debug)]
pub struct Scheduler {
    bridge: Bridge,
    processor: RubyProcessor,
}

impl Scheduler {
    pub async fn schedule<F>(
        &self,
        task_id: String,
        function: F,
    ) -> Result<TaskHandle, SchedulerError>
    where
        F: FnOnce(&Ruby) -> Result<(), Error> + Send + 'static,
    {
        let (result_tx, result_rx) = result_channel();
        let cloned_instance = self.processor.clone();

        let token = self
            .bridge
            .run_sync(move |ruby: &Ruby| {
                cloned_instance
                    .submit(ruby, &task_id, result_tx, function)
                    .map_err(|error| SchedulerError::Submit(error.to_string()))
            })
            .await??;

        Ok(TaskHandle::new(self.bridge.clone(), result_rx, token))
    }
}

#[derive(Debug, Error)]
pub enum SchedulerError {
    #[error("failed to submit task: {0}")]
    Submit(String),

    #[error(transparent)]
    Bridge(#[from] BridgeError),

    #[error("failed to cancel task: {0}")]
    Cancel(String),
}
