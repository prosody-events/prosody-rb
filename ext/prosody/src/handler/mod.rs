//! # Message Handler Module
//!
//! Provides functionality to handle Kafka messages in Ruby by bridging between
//! the Rust-based Prosody Kafka client and Ruby application code.
//!
//! This module implements a handler for Kafka consumer messages that:
//! 1. Receives messages from Kafka via the Prosody consumer
//! 2. Converts them to Ruby objects
//! 3. Schedules execution in the Ruby runtime
//! 4. Properly handles error cases and shutdown scenarios

use crate::bridge::{Bridge, BridgeError};
use crate::handler::context::Context;
use crate::handler::message::Message;
use crate::handler::trigger::Timer;
use crate::id;
use crate::scheduler::result::ProcessingError;
use crate::scheduler::{Scheduler, SchedulerError};
use crate::util::ThreadSafeValue;
use futures::pin_mut;
use magnus::value::ReprValue;
use magnus::{Error, Ruby, Value};
use opentelemetry::propagation::TextMapCompositePropagator;
use opentelemetry::trace::Status;
use prosody::consumer::event_context::EventContext;
use prosody::consumer::message::ConsumerMessage;
use prosody::consumer::middleware::FallibleHandler;
use prosody::consumer::{DemandType, Keyed};
use prosody::error::{ClassifyError, ErrorCategory};
use prosody::propagator::new_propagator;
use prosody::timers::{TimerType, Trigger as ProsodyTrigger};
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;
use tokio::select;
use tracing::{Instrument, info_span};
use tracing_opentelemetry::OpenTelemetrySpanExt;

mod context;
mod message;
mod trigger;

/// A handler that bridges between Kafka messages and Ruby message processing
/// code.
///
/// This struct manages the execution of Ruby message handling code when Kafka
/// messages are received. It uses a scheduler to run the Ruby handler code
/// in the Ruby runtime environment, ensuring proper thread safety and error
/// handling.
#[derive(Clone, Debug)]
pub struct RubyHandler {
    /// Bridge for communicating between Rust and Ruby
    bridge: Bridge,

    /// Scheduler for running Ruby code in the Ruby runtime
    scheduler: Arc<Scheduler>,

    /// Thread-safe reference to the Ruby handler instance
    handler: Arc<ThreadSafeValue>,

    /// OpenTelemetry propagator shared across contexts
    propagator: Arc<TextMapCompositePropagator>,
}

impl RubyHandler {
    /// Creates a new handler that will dispatch Kafka messages to a Ruby
    /// handler instance.
    ///
    /// # Arguments
    ///
    /// * `bridge` - Bridge for communicating between Rust and Ruby threads
    /// * `ruby` - Reference to the Ruby VM
    /// * `handler_instance` - Ruby object that will handle Kafka messages
    ///
    /// # Errors
    ///
    /// Returns a `Magnus::Error` if creating the scheduler fails
    pub fn new(bridge: Bridge, ruby: &Ruby, handler_instance: Value) -> Result<Self, Error> {
        Ok(Self {
            bridge: bridge.clone(),
            handler: Arc::new(ThreadSafeValue::new(handler_instance, bridge.clone())),
            scheduler: Arc::new(Scheduler::new(ruby, bridge)?),
            propagator: Arc::new(new_propagator()),
        })
    }
}

impl FallibleHandler for RubyHandler {
    type Error = RubyHandlerError;

    /// Processes a Kafka message by dispatching it to the Ruby handler.
    ///
    /// This method:
    /// 1. Creates a unique task ID from the message metadata
    /// 2. Schedules the handler execution in the Ruby runtime
    /// 3. Waits for the result or responds to shutdown signals
    /// 4. Handles cancellation if a shutdown is requested
    ///
    /// # Arguments
    ///
    /// * `context` - Kafka message context containing metadata and control
    ///   signals
    /// * `message` - The Kafka message to be processed
    ///
    /// # Errors
    ///
    /// Returns a `RubyHandlerError` if any of the following fail:
    /// - Scheduling the task
    /// - Communication with the Ruby runtime
    /// - The Ruby handler throws an exception
    async fn on_message<C>(
        &self,
        context: C,
        message: ConsumerMessage,
        _demand_type: DemandType,
    ) -> Result<(), Self::Error>
    where
        C: EventContext,
    {
        // Create a new span for the on_message operation as a child of the message's
        // span
        let span = info_span!(
            parent: message.span(),
            "on_message",
            topic = %message.topic(),
            partition = message.partition(),
            offset = message.offset(),
            key = %message.key()
        );

        // Get a future that completes when cancellation is signaled
        let cloned_context = context.clone();
        let cancel_future = cloned_context.on_cancel();

        // Clone the handler reference for use in the closure
        let handler = self.handler.clone();

        // Create a unique task ID for this message
        let task_id = format!(
            "{}/{}:{}",
            message.topic(),
            message.partition(),
            message.offset()
        );

        let event_context = HashMap::from([
            ("event_type".into(), "message".into()),
            ("topic".into(), message.topic().to_string()),
            ("partition".into(), message.partition().to_string()),
            ("key".into(), message.key().to_string()),
            ("offset".into(), message.offset().to_string()),
        ]);

        // Convert the Kafka message and context to Ruby-compatible types
        let context = Context::new(
            context.boxed(),
            self.bridge.clone(),
            self.propagator.clone(),
        );
        let message: Message = message.into();

        // Execute the entire message handling operation within the span
        let cloned_span = span.clone();
        async move {
            // Schedule the task to run in Ruby
            let task_handle = self
                .scheduler
                .schedule(task_id, &cloned_span, event_context, move |ruby| {
                    let _: Value = handler
                        .get(ruby)
                        .funcall(id!(ruby, "on_message"), (context, message))?;

                    Ok(())
                })
                .await?;

            // Get the future that will complete when the task is done
            let result_future = task_handle.result.receive();
            pin_mut!(result_future);

            // Wait for either task completion or shutdown signal
            select! {
                result = &mut result_future => {
                    result.inspect_err(|e| cloned_span.set_status(Status::error(first_line(e))))?;
                }
                () = cancel_future => {
                    // A cancel() failure is a genuine bridge error; mark the span.
                    // The subsequent result_future error is expected cancellation, not a handler bug.
                    task_handle.cancellation_token.cancel(&self.bridge).await
                        .inspect_err(|e| cloned_span.set_status(Status::error(first_line(e))))?;
                    result_future.await?;
                }
            }

            Ok(())
        }
        .instrument(span)
        .await
    }

    async fn on_timer<C>(
        &self,
        context: C,
        trigger: ProsodyTrigger,
        _demand_type: DemandType,
    ) -> Result<(), Self::Error>
    where
        C: EventContext,
    {
        // Only process application timers; internal timers are handled by middleware
        if trigger.timer_type != TimerType::Application {
            return Ok(());
        }

        // Create a new span for the on_timer operation as a child of the trigger's span
        let span = info_span!(
            parent: trigger.span(),
            "on_timer",
            key = %trigger.key,
            time = %trigger.time
        );

        // Get a future that completes when cancellation is signaled
        let cloned_context = context.clone();
        let cancel_future = cloned_context.on_cancel();

        // Clone the handler reference for use in the closure
        let handler = self.handler.clone();

        // Create a unique task ID for this timer
        let task_id = format!("timer/{}/{}", trigger.key, trigger.time);

        let event_context = HashMap::from([
            ("event_type".into(), "timer".into()),
            ("key".into(), trigger.key.to_string()),
            ("time".into(), trigger.time.to_string()),
        ]);

        // Convert the timer trigger and context to Ruby-compatible types
        let context = Context::new(
            context.boxed(),
            self.bridge.clone(),
            self.propagator.clone(),
        );
        let timer: Timer = trigger.into();

        // Execute the entire timer handling operation within the span
        let cloned_span = span.clone();
        async move {
            // Schedule the task to run in Ruby
            let task_handle = self
                .scheduler
                .schedule(task_id, &cloned_span, event_context, move |ruby| {
                    let _: Value = handler
                        .get(ruby)
                        .funcall(id!(ruby, "on_timer"), (context, timer))?;

                    Ok(())
                })
                .await?;

            // Get the future that will complete when the task is done
            let result_future = task_handle.result.receive();
            pin_mut!(result_future);

            // Wait for either task completion or shutdown signal
            select! {
                result = &mut result_future => {
                    result.inspect_err(|e| cloned_span.set_status(Status::error(first_line(e))))?;
                }
                () = cancel_future => {
                    // A cancel() failure is a genuine bridge error; mark the span.
                    // The subsequent result_future error is expected cancellation, not a handler bug.
                    task_handle.cancellation_token.cancel(&self.bridge).await
                        .inspect_err(|e| cloned_span.set_status(Status::error(first_line(e))))?;
                    result_future.await?;
                }
            }

            Ok(())
        }
        .instrument(span)
        .await
    }

    /// Shuts down the handler.
    ///
    /// This is a no-op for the Ruby handler since resources are managed
    /// by the Ruby runtime through garbage collection.
    async fn shutdown(self) {
        // No cleanup required - Ruby handles resource cleanup via GC
    }
}

impl ClassifyError for RubyHandlerError {
    /// Categorizes errors for proper retry handling in the Kafka consumer.
    ///
    /// Maps error types to their appropriate categories:
    /// - Scheduler and Bridge errors are considered transient (retryable)
    /// - Processing errors are categorized according to their own
    ///   classification
    fn classify_error(&self) -> ErrorCategory {
        match self {
            RubyHandlerError::Scheduler(_) | RubyHandlerError::Bridge(_) => {
                ErrorCategory::Transient
            }
            RubyHandlerError::Processing(error) => error.classify_error(),
        }
    }
}

fn first_line(e: &impl std::fmt::Display) -> String {
    let s = e.to_string();
    s.lines().next().unwrap_or_default().to_owned()
}

/// Errors that can occur when handling Kafka messages in Ruby.
#[derive(Debug, Error)]
pub enum RubyHandlerError {
    /// Error from the task scheduler
    #[error(transparent)]
    Scheduler(#[from] SchedulerError),

    /// Error communicating with the Ruby runtime
    #[error(transparent)]
    Bridge(#[from] BridgeError),

    /// Error from the Ruby handler code
    #[error(transparent)]
    Processing(#[from] ProcessingError),
}

/// Initializes the handler module by registering Ruby classes for message
/// context and content.
///
/// # Arguments
///
/// * `ruby` - Reference to the Ruby VM
///
/// # Errors
///
/// Returns a `Magnus::Error` if class initialization fails
pub fn init(ruby: &Ruby) -> Result<(), Error> {
    context::init(ruby)?;
    message::init(ruby)?;
    trigger::init(ruby)?;

    Ok(())
}
