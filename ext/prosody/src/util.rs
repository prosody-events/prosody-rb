//! # Utilities for Ruby/Rust interoperability
//!
//! This module provides utilities for safe and efficient interaction between
//! Rust and Ruby, particularly focusing on thread-safety and efficient symbol
//! handling.

use crate::RUNTIME;
use magnus::value::BoxValue;
use magnus::{Ruby, Value};
use tokio::runtime::{EnterGuard, Handle};

/// Creates a static Ruby identifier (symbol) for efficient reuse.
///
/// This macro creates a lazily-initialized static Ruby identifier from a string
/// literal. Using this macro for frequently accessed Ruby method names or
/// symbols avoids repeatedly converting strings to Ruby symbols at runtime.
///
/// This macro requires a `ruby: &Ruby` parameter to enforce that it can only
/// be used within a Ruby thread context, ensuring thread safety.
///
/// # Examples
///
/// ```
/// let method_name = id!(ruby, "to_s");
/// // Use method_name with Ruby function calls
/// ```
#[macro_export]
macro_rules! id {
    ($ruby:expr, $str:expr) => {{
        static VAL: magnus::value::LazyId = magnus::value::LazyId::new($str);
        let _ruby: &magnus::Ruby = $ruby; // Enforce that ruby is &Ruby type
        *VAL
    }};
}

/// A thread-safe wrapper around a Ruby value.
///
/// Provides a way to safely share Ruby values between threads by wrapping
/// them in a type that implements both `Send` and `Sync`. This type enforces
/// that the underlying Ruby value is only accessed within a Ruby thread
/// context.
#[derive(Debug)]
pub struct ThreadSafeValue(BoxValue<Value>);

// SAFETY: The underlying value can only be accessed from a Ruby thread.
unsafe impl Send for ThreadSafeValue {}

// SAFETY: The underlying value can only be accessed from a Ruby thread.
unsafe impl Sync for ThreadSafeValue {}

impl ThreadSafeValue {
    /// Creates a new thread-safe wrapper around a Ruby value.
    ///
    /// # Arguments
    ///
    /// * `value` - The Ruby value to wrap
    pub fn new(value: Value) -> Self {
        Self(BoxValue::new(value))
    }

    /// Gets a reference to the wrapped Ruby value.
    ///
    /// This method ensures that access to the Ruby value only happens
    /// within a Ruby thread context by requiring a `Ruby` reference.
    ///
    /// # Arguments
    ///
    /// * `_ruby` - A reference to the Ruby VM
    pub fn get(&self, _ruby: &Ruby) -> &Value {
        &self.0
    }
}

/// Ensures we have a Tokio runtime context, entering one only if necessary.
///
/// This function prevents `EnterGuard` ordering violations by only creating a
/// new runtime guard when we're not already in a runtime context. This is
/// essential for preventing panics when async operations are called from
/// contexts that may already have an active runtime (such as from Ruby async
/// processor threads).
///
/// # Returns
///
/// An `Option<EnterGuard>` that holds the runtime guard if one was created,
/// or `None` if we were already in a runtime context.
///
/// # Examples
///
/// ```rust
/// let _guard = ensure_runtime_context();
/// // Now safe to perform async operations regardless of calling context
/// ```
pub fn ensure_runtime_context() -> Option<EnterGuard<'static>> {
    Handle::try_current().is_err().then(|| RUNTIME.enter())
}
