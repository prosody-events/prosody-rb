//! # Utilities for Ruby/Rust interoperability
//!
//! This module provides utilities for safe and efficient interaction between
//! Rust and Ruby, particularly focusing on thread-safety and efficient symbol
//! handling.

use crate::bridge::Bridge;
use crate::logging::Logger;
use crate::{BRIDGE, BRIDGE_BUFFER_SIZE, RUNTIME, TRACING_INIT};
use magnus::value::BoxValue;
use magnus::{Ruby, Value};
use prosody::tracing::initialize_tracing;
use std::mem::{ManuallyDrop, forget};
use tokio::runtime::{EnterGuard, Handle};
use tracing::warn;

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
///
/// # Drop safety
///
/// [`BoxValue`] must be dropped on a Ruby thread because its `Drop` impl calls
/// `rb_gc_unregister_address`. This type handles that automatically: if dropped
/// on a Ruby thread, the value is cleaned up normally. If dropped on a
/// non-Ruby thread (e.g. a Tokio worker), the value is leaked rather than
/// risking a segfault. The leaked GC root keeps the Ruby object alive for the
/// lifetime of the process. In practice the risk is low — most
/// `ThreadSafeValue` instances are long-lived (`Arc`-wrapped singletons) and
/// only drop during shutdown.
#[derive(Debug)]
pub struct ThreadSafeValue(ManuallyDrop<BoxValue<Value>>);

// SAFETY: The underlying value can only be accessed from a Ruby thread
// (enforced by requiring `&Ruby` in `get`). Drop is safe across threads:
// on a Ruby thread it calls `rb_gc_unregister_address` normally; on any
// other thread it leaks the value instead of calling into Ruby.
unsafe impl Send for ThreadSafeValue {}

// SAFETY: `get` requires `&Ruby`, ensuring the value is only read on a Ruby
// thread. The inner `BoxValue` is never mutated after construction.
unsafe impl Sync for ThreadSafeValue {}

impl ThreadSafeValue {
    /// Creates a new thread-safe wrapper around a Ruby value.
    ///
    /// # Arguments
    ///
    /// * `value` - The Ruby value to wrap
    pub fn new(value: Value) -> Self {
        Self(ManuallyDrop::new(BoxValue::new(value)))
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

impl Drop for ThreadSafeValue {
    fn drop(&mut self) {
        // SAFETY: `drop` is called exactly once per value, so this
        // `ManuallyDrop::take` cannot double-free.
        let inner = unsafe { ManuallyDrop::take(&mut self.0) };

        // On a Ruby thread: safe to drop directly (calls rb_gc_unregister_address).
        if Ruby::get().is_ok() {
            drop(inner);
            return;
        }

        // On a non-Ruby thread: leak rather than segfault. The GC root keeps
        // the Ruby object alive, but this only occurs at shutdown in practice.
        warn!(
            "leaked a Ruby value because it was dropped on a non-Ruby thread; this is safe but \
             indicates a value outlived its expected scope"
        );
        forget(inner);
    }
}

/// Ensures a Tokio runtime context exists, entering one if necessary.
///
/// Only creates a runtime guard when not already in a runtime context, avoiding
/// `EnterGuard` ordering violations that panic.
///
/// Lazily initializes the bridge and tracing subsystems on first call. This
/// deferred initialization is critical for fork safety—each process gets its
/// own bridge channels and tracing state rather than inheriting stale handles
/// from the parent.
///
/// # Returns
///
/// `Some(EnterGuard)` if we entered a new runtime (hold the guard), or `None`
/// if already in a runtime context.
///
/// # Examples
///
/// ```rust
/// let _guard = ensure_runtime_context(ruby);
/// // Safe to perform async operations
/// ```
pub fn ensure_runtime_context(ruby: &Ruby) -> Option<EnterGuard<'static>> {
    let guard = Handle::try_current().is_err().then(|| RUNTIME.enter());

    // Set up the bridge for Ruby-Rust communication
    let bridge = BRIDGE.get_or_init(|| Bridge::new(ruby, BRIDGE_BUFFER_SIZE));

    // Initialize tracing for observability
    #[allow(clippy::print_stderr, reason = "logger has not been initialized yet")]
    TRACING_INIT.get_or_init(|| {
        let maybe_logger = Logger::new(ruby, bridge.clone())
            .inspect_err(|error| eprintln!("failed to create logger: {error:#}"))
            .ok();

        if let Err(error) = initialize_tracing(maybe_logger) {
            eprintln!("failed to initialize tracing: {error:#}");
        }
    });

    guard
}
