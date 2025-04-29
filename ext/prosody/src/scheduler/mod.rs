use crate::RUNTIME;
use crate::bridge::{Bridge, BridgeError};
use crate::scheduler::handle::TaskHandle;
use crate::scheduler::processor::RubyProcessor;
use crate::scheduler::result::result_channel;
use magnus::{Error, Ruby};
use opentelemetry::propagation::{TextMapCompositePropagator, TextMapPropagator};
use prosody::propagator::new_propagator;
use std::collections::HashMap;
use std::convert::identity;
use thiserror::Error;
use tracing::{Span, error, instrument};
use tracing_opentelemetry::OpenTelemetrySpanExt;

mod cancellation;
pub mod handle;
mod processor;
pub mod result;

#[derive(Debug)]
pub struct Scheduler {
    bridge: Bridge,
    processor: RubyProcessor,
    propagator: TextMapCompositePropagator,
}

impl Clone for Scheduler {
    fn clone(&self) -> Self {
        Self {
            bridge: self.bridge.clone(),
            processor: self.processor.clone(),
            propagator: new_propagator(),
        }
    }
}

impl Scheduler {
    pub fn new(ruby: &Ruby, bridge: Bridge) -> Result<Self, Error> {
        Ok(Self {
            bridge,
            processor: RubyProcessor::new(ruby)?,
            propagator: new_propagator(),
        })
    }

    #[instrument(level = "debug", skip(self, span, function), err)]
    pub async fn schedule<F>(
        &self,
        task_id: String,
        span: &Span,
        function: F,
    ) -> Result<TaskHandle, SchedulerError>
    where
        F: FnOnce(&Ruby) -> Result<(), Error> + Send + 'static,
    {
        let mut carrier: HashMap<String, String> = HashMap::with_capacity(2);
        self.propagator
            .inject_context(&span.context(), &mut carrier);

        let (result_tx, result_rx) = result_channel();
        let cloned_instance = self.processor.clone();

        let token = self
            .bridge
            .run_sync(move |ruby: &Ruby| {
                cloned_instance
                    .submit(ruby, &task_id, carrier, result_tx, function)
                    .map_err(|error| SchedulerError::Submit(error.to_string()))
            })
            .await??;

        Ok(TaskHandle::new(result_rx, token))
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
