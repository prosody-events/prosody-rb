//! This module provides functionality to bridge between Rust and Ruby, allowing
//! asynchronous operations in Rust to communicate with the Ruby runtime.
//!
//! It includes mechanisms for executing Ruby code from Rust threads and for
//! managing asynchronous callback communication between the two languages.

use crate::bridge::Bridge;
use crate::id;
use crate::util::ThreadSafeValue;
use magnus::value::ReprValue;
use magnus::{Error, IntoValue, Ruby, Value};

/// Handles asynchronous callbacks from Rust to Ruby using a thread-safe queue.
///
/// This struct provides a way to send values back to Ruby from asynchronous
/// Rust operations, ensuring thread safety by wrapping the Ruby queue in a
/// thread-safe container.
pub struct AsyncCallback {
    /// A thread-safe wrapper around a Ruby Queue object
    queue: ThreadSafeValue,
}

impl AsyncCallback {
    /// Creates a new `AsyncCallback` from a Ruby Queue object.
    ///
    /// # Arguments
    ///
    /// * `queue` - A Ruby Queue object that will receive values from Rust
    /// * `bridge` - The bridge used to defer cleanup of the wrapped queue
    ///   value onto the Ruby thread when the callback is dropped
    pub fn from_queue(queue: Value, bridge: Bridge) -> Self {
        Self {
            queue: ThreadSafeValue::new(queue, bridge),
        }
    }

    /// Completes the callback by pushing a value to the associated Ruby queue.
    ///
    /// This consumes the callback, preventing it from being used multiple
    /// times.
    ///
    /// # Arguments
    ///
    /// * `ruby` - A reference to the Ruby VM
    /// * `value` - The value to push to the queue, which will be converted to a
    ///   Ruby value
    ///
    /// # Errors
    ///
    /// Returns an error if the Ruby function call fails
    pub fn complete<V>(self, ruby: &Ruby, value: V) -> Result<(), Error>
    where
        V: IntoValue,
    {
        self.queue
            .get(ruby)
            .funcall(id!(ruby, "push"), (value.into_value_with(ruby),))
            .map(|_: Value| ())
    }
}
