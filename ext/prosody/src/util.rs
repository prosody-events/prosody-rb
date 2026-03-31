//! # Utilities for Ruby/Rust interoperability
//!
//! This module provides utilities for safe and efficient interaction between
//! Rust and Ruby, particularly focusing on thread-safety and efficient symbol
//! handling.

use crate::bridge::Bridge;
use crate::logging::Logger;
use crate::{BRIDGE, RUNTIME, TRACING_INIT};
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
/// non-Ruby thread, the value is sent through the bridge for deferred cleanup
/// on the Ruby thread. If the bridge is unavailable (shutdown), the value is
/// leaked with a warning rather than risking a segfault.
#[derive(Debug)]
pub struct ThreadSafeValue {
    value: ManuallyDrop<RubyDrop>,
    bridge: Bridge,
}

// SAFETY: The underlying value can only be accessed from a Ruby thread
// (enforced by requiring `&Ruby` in `get`). Drop is safe across threads: it
// sends cleanup through the bridge when not on a Ruby thread, falling back
// to a leak if the bridge is unavailable.
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
    /// * `bridge` - The bridge used to defer cleanup onto the Ruby thread
    pub fn new(value: Value, bridge: Bridge) -> Self {
        Self {
            value: ManuallyDrop::new(RubyDrop::new(value)),
            bridge,
        }
    }

    /// Gets a reference to the wrapped Ruby value.
    ///
    /// This method ensures that access to the Ruby value only happens
    /// within a Ruby thread context by requiring a `Ruby` reference.
    ///
    /// # Arguments
    ///
    /// * `ruby` - A reference to the Ruby VM
    pub fn get(&self, ruby: &Ruby) -> &Value {
        self.value.get(ruby)
    }
}

impl Drop for ThreadSafeValue {
    fn drop(&mut self) {
        // SAFETY: `drop` is called exactly once per value, so this
        // `ManuallyDrop::take` cannot double-free.
        let inner = unsafe { ManuallyDrop::take(&mut self.value) };

        // On a Ruby thread: safe to drop directly.
        if RubyDrop::can_drop() {
            drop(inner);
            return;
        }

        // On a non-Ruby thread: send to the bridge for cleanup on the Ruby
        // thread. When the closure runs, `inner` is dropped on the Ruby thread
        // and `RubyDrop::Drop` takes the normal cleanup path.
        //
        // If the send fails (bridge shut down), the `SendError` owns the
        // closure which owns `inner`. When `SendError` drops on this
        // non-Ruby thread, `RubyDrop::Drop` takes the leak path — warn +
        // forget.
        if self
            .bridge
            .send(Box::new(move |_ruby| drop(inner)))
            .is_err()
        {
            warn!(
                "could not send Ruby value to bridge for cleanup because the bridge has \
                 shut down"
            );
        }
    }
}

/// Wraps a [`BoxValue`] and ensures it is dropped on a Ruby thread.
///
/// `BoxValue::drop` calls `rb_gc_unregister_address`, which must only run on
/// a Ruby thread. `RubyDrop` handles this safely: on a Ruby thread it drops
/// normally; on any other thread it leaks with a warning rather than
/// segfaulting. It has no knowledge of channels or bridges, so there is no
/// risk of infinite recursion in `Drop`.
#[derive(Debug)]
struct RubyDrop(ManuallyDrop<BoxValue<Value>>);

// SAFETY: Either dropped on a Ruby thread (normal cleanup) or leaked on a
// non-Ruby thread (forget + warn). Never calls into Ruby from the wrong thread.
unsafe impl Send for RubyDrop {}

impl RubyDrop {
    fn new(value: Value) -> Self {
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
    fn get(&self, _ruby: &Ruby) -> &Value {
        &self.0
    }

    /// Returns `true` if the current thread is a Ruby thread and it is safe
    /// to call `rb_gc_unregister_address` (i.e. drop the inner [`BoxValue`]).
    fn can_drop() -> bool {
        Ruby::get().is_ok()
    }
}

impl Drop for RubyDrop {
    fn drop(&mut self) {
        // SAFETY: `drop` is called exactly once per value.
        let inner = unsafe { ManuallyDrop::take(&mut self.0) };

        if Ruby::get().is_ok() {
            drop(inner);
            return;
        }

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
    let bridge = BRIDGE.get_or_init(|| Bridge::new(ruby));

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
