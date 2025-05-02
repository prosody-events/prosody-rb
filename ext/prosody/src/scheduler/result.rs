//! # Result Module
//!
//! Provides mechanisms for asynchronous result communication between Ruby and
//! Rust.
//!
//! This module implements a channel-based system that allows Ruby task results
//! to be communicated back to Rust, with support for different error categories
//! and one-time result delivery.

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

/// Creates a paired sender and receiver for communicating task results.
///
/// This function creates a one-shot channel wrapped with `AtomicTake` to ensure
/// the result is only sent once.
///
/// # Returns
///
/// A tuple containing a `ResultSender` and `ResultReceiver` that are connected.
pub fn result_channel() -> (ResultSender, ResultReceiver) {
    let (result_tx, result_rx) = oneshot::channel();
    let result_tx = AtomicTake::new(result_tx);
    (ResultSender { result_tx }, ResultReceiver { result_rx })
}

/// Sends task results from Ruby back to Rust.
///
/// This struct wraps a one-shot sender in an `AtomicTake` to ensure the result
/// can only be sent once, preventing accidental duplicate sends.
#[derive(Educe)]
#[educe(Debug)]
pub struct ResultSender {
    #[educe(Debug(ignore))]
    result_tx: AtomicTake<oneshot::Sender<Result<(), ProcessingError>>>,
}

/// Receives task results in Rust from Ruby.
///
/// This struct wraps a one-shot receiver for task results, allowing
/// asynchronous waiting for task completion or failure.
#[derive(Educe)]
#[educe(Debug)]
pub struct ResultReceiver {
    #[educe(Debug(ignore))]
    result_rx: oneshot::Receiver<Result<(), ProcessingError>>,
}

impl ResultSender {
    /// Sends a result from Ruby to Rust.
    ///
    /// This method takes the result from Ruby, interprets it as a success or
    /// failure, and sends it through the channel. For failures, it attempts
    /// to extract meaningful error messages from the Ruby exception.
    ///
    /// # Arguments
    ///
    /// * `is_success` - Boolean indicating if the operation succeeded.
    /// * `result` - For failures, this contains the Ruby exception.
    ///
    /// # Returns
    ///
    /// `Ok(())` if the result was sent successfully.
    ///
    /// # Errors
    ///
    /// Returns a `magnus::Error` if the result has already been sent.
    pub fn send(&self, is_success: bool, result: Value) {
        let Some(result_tx) = self.result_tx.take() else {
            debug!("result was already sent");
            return;
        };

        if is_success {
            if result_tx.send(Ok(())).is_err() {
                debug!("discarding result; receiver went away");
            }

            return;
        }

        let is_permanent = result.funcall(id!("permanent?"), ()).unwrap_or(false);

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
        }
    }

    /// Converts this sender into a Ruby Proc that can be called from Ruby.
    ///
    /// This method creates a Ruby Proc that, when called with success status
    /// and result, will send the result through this sender.
    ///
    /// # Arguments
    ///
    /// * `ruby` - Reference to the Ruby VM.
    ///
    /// # Returns
    ///
    /// A Ruby `Proc` that can be passed to Ruby code.
    pub fn into_proc(self, ruby: &Ruby) -> Proc {
        ruby.proc_from_fn(move |ruby, args, _block| match args {
            [is_success, result, ..] => {
                self.send(bool::try_convert(*is_success)?, *result);
                Ok(())
            }
            _ => Err(Error::new(
                ruby.exception_arg_error(),
                format!("Expected two arguments but received {}", args.len()),
            )),
        })
    }
}

impl ResultReceiver {
    /// Asynchronously waits for the result from Ruby.
    ///
    /// This method waits for the Ruby task to complete and returns its result.
    ///
    /// # Returns
    ///
    /// `Ok(())` if the task completed successfully, or an error if the task
    /// failed.
    ///
    /// # Errors
    ///
    /// Returns a `ProcessingError` if the task failed or the channel was
    /// closed.
    pub async fn receive(self) -> Result<(), ProcessingError> {
        self.result_rx.await.map_err(|_| ProcessingError::Closed)?
    }
}

/// Errors that can occur during task processing.
///
/// This enum represents various error scenarios that can occur when processing
/// tasks between Ruby and Rust, with specific categorization for handling
/// strategies.
#[derive(Clone, Debug, Error)]
pub enum ProcessingError {
    /// A temporary error that may succeed on retry.
    #[error("transient error: {0}")]
    Transient(String),

    /// A permanent error that will not succeed on retry.
    #[error("permanent error: {0}")]
    Permanent(String),

    /// Error indicating the result channel was closed before receiving a
    /// result.
    #[error("result channel has been closed")]
    Closed,
}

impl ClassifyError for ProcessingError {
    /// Classifies the error as transient or permanent for retry decisions.
    ///
    /// This method determines whether a task should be retried based on the
    /// error type.
    fn classify_error(&self) -> ErrorCategory {
        match self {
            ProcessingError::Transient(_) | ProcessingError::Closed => ErrorCategory::Transient,
            ProcessingError::Permanent(_) => ErrorCategory::Permanent,
        }
    }
}
