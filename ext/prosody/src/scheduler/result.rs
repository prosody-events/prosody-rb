//! # Result Communication
//!
//! Provides asynchronous result communication between Ruby and Rust.
//!
//! This module implements a channel-based system for Ruby task results to be
//! communicated back to Rust. It supports different error categories
//! (transient vs permanent) and ensures one-time result delivery through
//! atomic guarantees.

use crate::id;
use atomic_take::AtomicTake;
use educe::Educe;
use magnus::block::Proc;
use magnus::value::ReprValue;
use magnus::{Error, Ruby, TryConvert, Value, kwargs};
use prosody::consumer::failure::{ClassifyError, ErrorCategory};
use thiserror::Error;
use tokio::sync::oneshot;
use tracing::debug;

/// Creates a paired sender and receiver for task result communication.
///
/// Creates a one-shot channel wrapped with `AtomicTake` to ensure
/// the result is only sent once, preventing duplicate delivery.
///
/// # Returns
///
/// A tuple containing a connected `ResultSender` and `ResultReceiver` pair.
pub fn result_channel() -> (ResultSender, ResultReceiver) {
    let (result_tx, result_rx) = oneshot::channel();
    let result_tx = AtomicTake::new(result_tx);
    (ResultSender { result_tx }, ResultReceiver { result_rx })
}

/// Sends task results from Ruby back to Rust.
///
/// Wraps a one-shot sender in an `AtomicTake` to guarantee that the result
/// can only be sent once, preventing accidental duplicate sends.
#[derive(Educe)]
#[educe(Debug)]
pub struct ResultSender {
    #[educe(Debug(ignore))]
    result_tx: AtomicTake<oneshot::Sender<Result<(), ProcessingError>>>,
}

/// Receives task results in Rust from Ruby.
///
/// Wraps a one-shot receiver that allows asynchronously waiting
/// for a task to complete or fail.
#[derive(Educe)]
#[educe(Debug)]
pub struct ResultReceiver {
    #[educe(Debug(ignore))]
    result_rx: oneshot::Receiver<Result<(), ProcessingError>>,
}

impl ResultSender {
    /// Sends a result from Ruby to Rust.
    ///
    /// Takes a Ruby result value, interprets it as a success or failure,
    /// and sends it through the channel. For failures, it extracts meaningful
    /// error messages from the Ruby exception and categorizes them as
    /// permanent or transient.
    ///
    /// # Arguments
    ///
    /// * `_ruby` - Reference to the Ruby VM (unused but required by the
    ///   signature)
    /// * `is_success` - Boolean indicating if the operation succeeded
    /// * `result` - For successes, the return value; for failures, the Ruby
    ///   exception
    ///
    /// # Returns
    ///
    /// `true` if the result was sent successfully, `false` if the result had
    /// already been sent or the receiver was dropped.
    pub fn send(&self, _ruby: &Ruby, is_success: bool, result: Value) -> bool {
        let Some(result_tx) = self.result_tx.take() else {
            debug!("result was already sent");
            return false;
        };

        if is_success {
            if result_tx.send(Ok(())).is_err() {
                debug!("discarding result; receiver went away");
            }

            return true;
        }

        // For error results, determine if the error is permanent by calling
        // the `permanent?` method on the Ruby exception
        let is_permanent = result.funcall(id!("permanent?"), ()).unwrap_or(false);

        // Extract a detailed error message from the Ruby exception
        let error_string: String = result
            .funcall(id!("full_message"), (kwargs!("highlight" => false),))
            .or_else(|_| result.funcall(id!("inspect"), ()))
            .unwrap_or_else(|_| result.to_string());

        let error = if is_permanent {
            ProcessingError::Permanent(error_string)
        } else {
            ProcessingError::Transient(error_string)
        };

        if result_tx.send(Err(error)).is_err() {
            debug!("discarding result; receiver went away");
            return false;
        }

        true
    }

    /// Converts this sender into a Ruby Proc for callback-style integration.
    ///
    /// Creates a Ruby Proc that, when called with a success status and result
    /// value, will send that result through this sender back to Rust.
    ///
    /// # Arguments
    ///
    /// * `ruby` - Reference to the Ruby VM
    ///
    /// # Returns
    ///
    /// A Ruby `Proc` that can be passed to Ruby code as a callback
    pub fn into_proc(self, ruby: &Ruby) -> Proc {
        ruby.proc_from_fn(move |ruby, args, _block| match args {
            [is_success, result, ..] => {
                let was_sent = self.send(ruby, bool::try_convert(*is_success)?, *result);
                Ok(was_sent)
            }
            _ => Err(Error::new(
                ruby.exception_arg_error(),
                format!("Expected two arguments but received {}", args.len()),
            )),
        })
    }
}

impl ResultReceiver {
    /// Asynchronously waits for a result from Ruby.
    ///
    /// Awaits the task completion in Ruby and returns its result.
    /// This consumes the receiver, ensuring the result can only be awaited
    /// once.
    ///
    /// # Returns
    ///
    /// `Ok(())` if the task completed successfully
    ///
    /// # Errors
    ///
    /// Returns a `ProcessingError` if:
    /// - The task failed (with either a permanent or transient error)
    /// - The channel was closed unexpectedly (e.g., the Ruby VM terminated)
    pub async fn receive(self) -> Result<(), ProcessingError> {
        self.result_rx.await.map_err(|_| ProcessingError::Closed)?
    }
}

/// Errors that can occur during task processing.
///
/// Represents error scenarios when processing tasks between Ruby and Rust,
/// with categorization that informs handling strategies (e.g., retry policies).
#[derive(Clone, Debug, Error)]
pub enum ProcessingError {
    /// A temporary error that may succeed on retry.
    #[error("transient error: {0}")]
    Transient(String),

    /// A permanent error that will not succeed on retry.
    #[error("permanent error: {0}")]
    Permanent(String),

    /// The result channel was closed before receiving a result.
    ///
    /// This typically happens when the Ruby VM terminates while a task is in
    /// progress.
    #[error("result channel has been closed")]
    Closed,
}

impl ClassifyError for ProcessingError {
    /// Classifies errors to determine retry behavior.
    ///
    /// Maps error types to their appropriate retry categories:
    /// - Transient errors are retryable
    /// - Permanent errors should not be retried
    /// - Channel closure is treated as a transient error, allowing for retries
    ///   when communication is restored
    fn classify_error(&self) -> ErrorCategory {
        match self {
            ProcessingError::Transient(_) | ProcessingError::Closed => ErrorCategory::Transient,
            ProcessingError::Permanent(_) => ErrorCategory::Permanent,
        }
    }
}
