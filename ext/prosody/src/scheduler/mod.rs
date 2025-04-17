use crate::RUNTIME;
use crate::bridge::{Bridge, BridgeError};
use crate::result::result_channel;
use crate::scheduler::handle::TaskHandle;
use crate::scheduler::processor::RubyProcessor;
use magnus::{Error, Ruby};
use std::convert::identity;
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
    pub async fn new(bridge: Bridge) -> Result<Self, SchedulerError> {
        let processor = bridge
            .run_sync(|ruby| {
                RubyProcessor::new(ruby)
                    .map_err(|error| SchedulerError::Processor(error.to_string()))
            })
            .await??;

        Ok(Self { bridge, processor })
    }

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

impl Drop for Scheduler {
    fn drop(&mut self) {
        let bridge = self.bridge.clone();
        let processor = self.processor.clone();

        RUNTIME.spawn(async move {
            if let Err(error) = bridge
                .run_sync(move |ruby| {
                    processor
                        .stop(ruby)
                        .map_err(|error| SchedulerError::Shutdown(error.to_string()))
                })
                .await
                .map_err(SchedulerError::from)
                .and_then(identity)
            {
                error!("failed to shutdown processor: {error:#}");
            }
        });
    }
}

#[derive(Debug, Error)]
pub enum SchedulerError {
    #[error("failed to create task processor: {0}")]
    Processor(String),

    #[error("failed to submit task: {0}")]
    Submit(String),

    #[error(transparent)]
    Bridge(#[from] BridgeError),

    #[error("failed to cancel task: {0}")]
    Cancel(String),

    #[error("failed to shutdown processor: {0}")]
    Shutdown(String),
}
