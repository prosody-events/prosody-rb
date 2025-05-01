//! # Task Handle
//!
//! This module provides the `TaskHandle` type, which represents a handle to an asynchronous
//! task scheduled for execution in the Ruby runtime.
//!
//! A `TaskHandle` encapsulates:
//! 1. A `ResultReceiver` for awaiting the completion of the task
//! 2. A `CancellationToken` for requesting cancellation of the task

use crate::scheduler::cancellation::CancellationToken;
use crate::scheduler::result::ResultReceiver;

/// A handle to an asynchronous task scheduled for execution in the Ruby runtime.
///
/// This struct combines two key components for managing asynchronous tasks:
/// - A `ResultReceiver` that allows waiting for the task's completion and retrieving its result
/// - A `CancellationToken` that enables requesting cancellation of the in-progress task
///
/// The handle is typically returned from the `Scheduler::schedule` method and
/// should be retained as long as the task is active to maintain the ability
/// to await its result or request cancellation.
#[derive(Debug)]
pub struct TaskHandle {
    /// Receiver for the task's result, used to await task completion
    pub result: ResultReceiver,

    /// Token used to request cancellation of the task
    pub cancellation_token: CancellationToken,
}

impl TaskHandle {
    /// Creates a new `TaskHandle` with the provided result receiver and cancellation token.
    ///
    /// # Arguments
    ///
    /// * `result` - The receiver that will provide the task's result upon completion
    /// * `cancellation_token` - The token that can be used to request cancellation of the task
    pub fn new(result: ResultReceiver, cancellation_token: CancellationToken) -> TaskHandle {
        Self {
            result,
            cancellation_token,
        }
    }
}
