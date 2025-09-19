//! Defines the Ruby-facing wrapper for timer triggers.
//!
//! This module implements a Ruby-accessible wrapper around the Prosody timer
//! trigger type, exposing trigger properties as Ruby objects.

use crate::{ROOT_MOD, id};
use educe::Educe;
use magnus::value::ReprValue;
use magnus::{Error, Module, RClass, Ruby, Value, method};
use prosody::timers::Trigger;

/// Ruby-accessible wrapper for timer triggers.
///
/// Provides Ruby bindings to access timer trigger data including key,
/// execution time, and tracing span information.
#[derive(Educe, Clone)]
#[educe(Debug)]
#[magnus::wrap(class = "Prosody::Timer", frozen_shareable)]
pub struct Timer {
    /// The wrapped Prosody timer trigger
    #[educe(Debug(ignore))]
    inner: Trigger,
}

impl Timer {
    /// Returns the entity key identifying what this timer belongs to.
    ///
    /// # Returns
    ///
    /// A string slice containing the entity key.
    fn key(&self) -> &str {
        self.inner.key.as_ref()
    }

    /// Converts the trigger execution time to a Ruby Time object.
    ///
    /// # Arguments
    ///
    /// * `ruby` - Reference to the Ruby VM
    /// * `this` - Reference to the Trigger instance
    ///
    /// # Returns
    ///
    /// A Ruby Time object representing when this timer should execute.
    ///
    /// # Errors
    ///
    /// Returns an error if the Ruby Time class cannot be accessed or if
    /// creating the Time object fails.
    fn time(ruby: &Ruby, this: &Self) -> Result<Value, Error> {
        // Direct access to epoch seconds - CompactDateTime has no sub-second precision
        let epoch_seconds = i64::from(this.inner.time.epoch_seconds());

        // Create Ruby Time with zero nanoseconds (CompactDateTime precision limit)
        ruby.module_kernel()
            .const_get::<_, RClass>(id!(ruby, "Time"))?
            .funcall(id!(ruby, "at"), (epoch_seconds,))
    }
}

impl From<Trigger> for Timer {
    /// Creates a new Timer wrapper from a Prosody `Trigger`.
    ///
    /// # Arguments
    ///
    /// * `value` - The Prosody `Trigger` to wrap
    fn from(value: Trigger) -> Self {
        Self { inner: value }
    }
}

/// Initializes the `Prosody::Timer` Ruby class and defines its methods.
///
/// # Arguments
///
/// * `ruby` - Reference to the Ruby VM
///
/// # Returns
///
/// OK on successful initialization.
///
/// # Errors
///
/// Returns an error if class or method definition fails.
pub fn init(ruby: &Ruby) -> Result<(), Error> {
    let module = ruby.get_inner(&ROOT_MOD);
    let class = module.define_class(id!(ruby, "Timer"), ruby.class_object())?;

    class.define_method(id!(ruby, "key"), method!(Timer::key, 0))?;
    class.define_method(id!(ruby, "time"), method!(Timer::time, 0))?;

    Ok(())
}
