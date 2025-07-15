//! Provides functionality for canceling scheduled Ruby tasks from Rust.
//!
//! This module defines a token-based cancellation mechanism that allows
//! asynchronous Rust code to safely cancel operations running in the Ruby VM.

use crate::bridge::Bridge;
use crate::id;
use crate::scheduler::SchedulerError;
use crate::util::ThreadSafeValue;
use magnus::value::ReprValue;
use magnus::{Ruby, Value};

/// Token that can be used to cancel a scheduled Ruby task.
///
/// This struct wraps a Ruby cancellation token object and provides a safe
/// interface for canceling the associated task across the Rust/Ruby boundary.
#[derive(Debug)]
pub struct CancellationToken {
    /// The Ruby cancellation token, wrapped in a thread-safe container
    token: ThreadSafeValue,
}

impl CancellationToken {
    /// Creates a new cancellation token from a Ruby value.
    ///
    /// # Arguments
    ///
    /// * `token` - A Ruby value that responds to the `cancel` method
    pub fn new(token: Value) -> Self {
        Self {
            token: ThreadSafeValue::new(token),
        }
    }

    /// Cancels the associated Ruby task.
    ///
    /// This method consumes the token, preventing it from being used to cancel
    /// the task more than once.
    ///
    /// # Arguments
    ///
    /// * `bridge` - The bridge used to execute Ruby code from Rust
    ///
    /// # Returns
    ///
    /// `Ok(())` if the task was successfully canceled.
    ///
    /// # Errors
    ///
    /// Returns a `SchedulerError::Cancel` if:
    /// - The Ruby `cancel` method call fails
    /// - The bridge fails to execute the Ruby code
    pub async fn cancel(self, bridge: &Bridge) -> Result<(), SchedulerError> {
        bridge
            .run(move |ruby: &Ruby| {
                self.token
                    .get(ruby)
                    .funcall::<_, _, Value>(id!(ruby, "cancel"), ())
                    .map_err(|error| SchedulerError::Cancel(error.to_string()))?;

                Ok(())
            })
            .await?
    }
}
