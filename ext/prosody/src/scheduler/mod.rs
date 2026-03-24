//! # Scheduler
//!
//! The scheduler module provides an asynchronous task scheduling mechanism for
//! executing Ruby code from Rust. It handles the complexities of bridging
//! between the Rust async world and the Ruby synchronous environment, ensuring
//! proper propagation of tracing context.
//!
//! This module manages task submission, execution, cancellation, and result
//! handling, while preserving proper OpenTelemetry context across language
//! boundaries.

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

/// Manages the scheduling of Rust functions for execution in Ruby, with proper
/// context propagation.
///
/// The `Scheduler` is responsible for:
/// - Submitting tasks to a Ruby processor for execution
/// - Propagating OpenTelemetry context between Rust and Ruby
/// - Managing task lifecycle and result handling
/// - Ensuring proper shutdown when dropped
#[derive(Debug)]
pub struct Scheduler {
    /// Communication bridge to the Ruby runtime
    bridge: Bridge,

    /// Ruby-side task processor
    processor: RubyProcessor,

    /// Propagator for OpenTelemetry context
    propagator: TextMapCompositePropagator,
}

impl Scheduler {
    /// Creates a new `Scheduler` instance.
    ///
    /// # Arguments
    ///
    /// * `ruby` - A reference to the Ruby VM
    /// * `bridge` - A bridge for communication with the Ruby runtime
    ///
    /// # Errors
    ///
    /// Returns a `Magnus::Error` if the Ruby processor cannot be created.
    pub fn new(ruby: &Ruby, bridge: Bridge) -> Result<Self, Error> {
        Ok(Self {
            bridge,
            processor: RubyProcessor::new(ruby)?,
            propagator: new_propagator(),
        })
    }

    /// Schedules a function to be executed in the Ruby runtime.
    ///
    /// This method:
    /// 1. Injects the current tracing context into a carrier
    /// 2. Creates a channel for receiving the task result
    /// 3. Submits the task to the Ruby processor
    /// 4. Returns a handle for tracking the task
    ///
    /// # Arguments
    ///
    /// * `task_id` - A unique identifier for the task
    /// * `span` - The tracing span to propagate to Ruby
    /// * `function` - The function to execute in Ruby
    ///
    /// # Errors
    ///
    /// Returns a `SchedulerError` if:
    /// - The bridge fails to run the submission function
    /// - The processor fails to submit the task
    ///
    /// # Returns
    ///
    /// A `TaskHandle` for tracking the status and result of the scheduled task.
    #[instrument(level = "debug", skip(self, span, event_context, function), err)]
    pub async fn schedule<F>(
        &self,
        task_id: String,
        span: &Span,
        event_context: HashMap<String, String>,
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
            .run(move |ruby: &Ruby| {
                cloned_instance
                    .submit(ruby, &task_id, carrier, event_context, result_tx, function)
                    .map_err(|error| SchedulerError::Submit(error.to_string()))
            })
            .await??;

        Ok(TaskHandle::new(result_rx, token))
    }
}

impl Drop for Scheduler {
    /// Ensures the Ruby processor is properly stopped when the scheduler is
    /// dropped.
    ///
    /// This spawns an async task to gracefully shut down the processor,
    /// logging any errors that occur during shutdown.
    fn drop(&mut self) {
        let bridge = self.bridge.clone();
        let processor = self.processor.clone();

        RUNTIME.spawn(async move {
            if let Err(error) = bridge
                .run(move |ruby| {
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

/// Errors that can occur during scheduler operations.
#[derive(Debug, Error)]
pub enum SchedulerError {
    /// Failed to submit a task to the Ruby processor.
    #[error("failed to submit task: {0}")]
    Submit(String),

    /// An error occurred in the bridge while communicating with Ruby.
    #[error(transparent)]
    Bridge(#[from] BridgeError),

    /// Failed to cancel a running task.
    #[error("failed to cancel task: {0}")]
    Cancel(String),

    /// Failed to properly shut down the Ruby processor.
    #[error("failed to shutdown processor: {0}")]
    Shutdown(String),
}
