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
use crate::id;
use crate::scheduler::result::ProcessingError;
use crate::scheduler::{Scheduler, SchedulerError};
use crate::util::ThreadSafeValue;
use futures::pin_mut;
use magnus::value::ReprValue;
use magnus::{Error, Ruby, Value};
use prosody::consumer::failure::{ClassifyError, ErrorCategory, FallibleHandler};
use prosody::consumer::message::{ConsumerMessage, MessageContext};
use std::sync::Arc;
use thiserror::Error;
use tokio::select;

mod context;
mod message;

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
    scheduler: Scheduler,

    /// Thread-safe reference to the Ruby handler instance
    handler: Arc<ThreadSafeValue>,
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
            scheduler: Scheduler::new(ruby, bridge)?,
            handler: Arc::new(ThreadSafeValue::new(handler_instance)),
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
    async fn on_message(
        &self,
        context: MessageContext,
        message: ConsumerMessage,
    ) -> Result<(), Self::Error> {
        // Capture the tracing span for propagation to Ruby
        let span = message.span().clone();

        // Get a future that completes when the consumer is shutdown
        let shutdown_future = context.on_shutdown();

        // Clone the handler reference for use in the closure
        let handler = self.handler.clone();

        // Create a unique task ID for this message
        let task_id = format!(
            "{}/{}:{}",
            message.topic(),
            message.partition(),
            message.offset()
        );

        // Convert the Kafka message and context to Ruby-compatible types
        let context: Context = context.into();
        let message: Message = message.into();

        // Schedule the task to run in Ruby
        let task_handle = self
            .scheduler
            .schedule(task_id, &span, move |ruby| {
                let _: Value = handler
                    .get(ruby)
                    .funcall(id!("on_message"), (context, message))?;

                Ok(())
            })
            .await?;

        // Get the future that will complete when the task is done
        let result_future = task_handle.result.receive();
        pin_mut!(result_future);

        // Wait for either task completion or shutdown signal
        select! {
            result = &mut result_future => {
                result?;
            }
            () = shutdown_future => {
                // If shutdown requested, cancel the task and wait for it to complete
                task_handle.cancellation_token.cancel(&self.bridge).await?;
                result_future.await?;
            }
        }

        Ok(())
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

    Ok(())
}
