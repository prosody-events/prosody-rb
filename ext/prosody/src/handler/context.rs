//! Ruby wrapper for the Prosody message context.
//!
//! This module provides a Ruby-compatible wrapper around the Prosody library's
//! `MessageContext`, allowing Ruby code to interact with message context
//! information from Kafka messages and schedule timer events.

use crate::bridge::Bridge;
use crate::tracing_util::extract_opentelemetry_context;
use crate::{ROOT_MOD, id};
use educe::Educe;
use futures::TryStreamExt;
use magnus::exception::{arg_error, runtime_error};
use magnus::value::ReprValue;
use magnus::{Error, Module, RClass, Ruby, Value, method};
use opentelemetry::propagation::TextMapCompositePropagator;
use prosody::consumer::event_context::BoxEventContext;
use prosody::timers::datetime::CompactDateTime;
use std::sync::Arc;
use tracing::info_span;
use tracing_opentelemetry::OpenTelemetrySpanExt;

/// Nanosecond threshold for rounding Ruby Time objects to the nearest second.
/// At 0.5 seconds, times round up; below 0.5 seconds, they round down.
const NANOSECOND_ROUNDING_THRESHOLD: u32 = 500_000_000;

/// Ruby wrapper for a Kafka message context.
///
/// This struct wraps the native Prosody `MessageContext` and exposes it to Ruby
/// code as the `Prosody::Context` class. The context contains metadata and
/// control capabilities related to the processing of a Kafka message.
#[derive(Educe, Clone)]
#[educe(Debug)]
#[magnus::wrap(class = "Prosody::Context")]
pub struct Context {
    /// The underlying Prosody message context.
    ///
    /// This field is marked as hidden in debug output to prevent logging large
    /// data.
    #[allow(dead_code)]
    #[educe(Debug(ignore))]
    inner: BoxEventContext,

    /// Bridge for handling async operations
    #[educe(Debug(ignore))]
    bridge: Bridge,

    /// OpenTelemetry propagator for distributed tracing
    #[educe(Debug(ignore))]
    propagator: Arc<TextMapCompositePropagator>,
}

impl Context {
    /// Creates a new `Context` from a Prosody `EventContext` and Bridge.
    ///
    /// # Arguments
    ///
    /// * `inner` - The Prosody event context to wrap
    /// * `bridge` - The bridge for handling async operations
    /// * `propagator` - Shared OpenTelemetry propagator for distributed tracing
    pub fn new(
        inner: BoxEventContext,
        bridge: Bridge,
        propagator: Arc<TextMapCompositePropagator>,
    ) -> Self {
        Self {
            inner,
            bridge,
            propagator,
        }
    }

    /// Check if shutdown has been requested.
    ///
    /// # Returns
    ///
    /// Boolean indicating whether shutdown is in progress.
    fn should_shutdown(&self) -> bool {
        self.inner.should_shutdown()
    }

    /// Schedule a timer to fire at the specified time.
    ///
    /// # Arguments
    ///
    /// * `ruby` - Reference to the Ruby VM
    /// * `ruby_time` - Ruby Time object specifying when the timer should fire
    ///
    /// # Returns
    ///
    /// Nothing on success.
    ///
    /// # Errors
    ///
    /// Returns an error if the time is invalid or if scheduling fails.
    fn schedule(ruby: &Ruby, this: &Self, ruby_time: Value) -> Result<(), Error> {
        let compact_time = time_to_compact_datetime(ruby, ruby_time)?;
        let inner = this.inner.clone();

        // Extract OpenTelemetry context from Ruby for distributed tracing
        let context = extract_opentelemetry_context(ruby, &this.propagator)?;

        // Create span for tracing and set parent context
        let span = info_span!("schedule", time = %compact_time);
        span.set_parent(context);

        this.bridge
            .wait_for(
                ruby,
                async move { inner.schedule(compact_time).await },
                span,
            )?
            .map_err(|error| {
                Error::new(
                    runtime_error(),
                    format!("Failed to schedule timer: {error:#}"),
                )
            })
    }

    /// Clear all scheduled timers and schedule a new one at the specified time.
    ///
    /// # Arguments
    ///
    /// * `ruby` - Reference to the Ruby VM
    /// * `ruby_time` - Ruby Time object specifying when the new timer should
    ///   fire
    ///
    /// # Returns
    ///
    /// Nothing on success.
    ///
    /// # Errors
    ///
    /// Returns an error if the time is invalid or if scheduling fails.
    fn clear_and_schedule(ruby: &Ruby, this: &Self, ruby_time: Value) -> Result<(), Error> {
        let compact_time = time_to_compact_datetime(ruby, ruby_time)?;
        let inner = this.inner.clone();

        // Extract OpenTelemetry context from Ruby for distributed tracing
        let context = extract_opentelemetry_context(ruby, &this.propagator)?;

        // Create span for tracing and set parent context
        let span = info_span!("clear_and_schedule", time = %compact_time);
        span.set_parent(context);

        this.bridge
            .wait_for(
                ruby,
                async move { inner.clear_and_schedule(compact_time).await },
                span,
            )?
            .map_err(|error| {
                Error::new(
                    runtime_error(),
                    format!("Failed to clear and schedule timer: {error:#}"),
                )
            })
    }

    /// Unschedule a timer at the specified time.
    ///
    /// # Arguments
    ///
    /// * `ruby` - Reference to the Ruby VM
    /// * `ruby_time` - Ruby Time object specifying which timer to unschedule
    ///
    /// # Returns
    ///
    /// Nothing on success.
    ///
    /// # Errors
    ///
    /// Returns an error if the time is invalid or if unscheduling fails.
    fn unschedule(ruby: &Ruby, this: &Self, ruby_time: Value) -> Result<(), Error> {
        let compact_time = time_to_compact_datetime(ruby, ruby_time)?;
        let inner = this.inner.clone();

        // Extract OpenTelemetry context from Ruby for distributed tracing
        let context = extract_opentelemetry_context(ruby, &this.propagator)?;

        // Create span for tracing and set parent context
        let span = info_span!("unschedule", time = %compact_time);
        span.set_parent(context);

        this.bridge
            .wait_for(
                ruby,
                async move { inner.unschedule(compact_time).await },
                span,
            )?
            .map_err(|error| {
                Error::new(
                    runtime_error(),
                    format!("Failed to unschedule timer: {error:#}"),
                )
            })
    }

    /// Clear all scheduled timers.
    ///
    /// # Arguments
    ///
    /// * `ruby` - Reference to the Ruby VM
    ///
    /// # Returns
    ///
    /// Nothing on success.
    ///
    /// # Errors
    ///
    /// Returns an error if clearing fails.
    fn clear_scheduled(ruby: &Ruby, this: &Self) -> Result<(), Error> {
        let inner = this.inner.clone();

        // Extract OpenTelemetry context from Ruby for distributed tracing
        let context = extract_opentelemetry_context(ruby, &this.propagator)?;

        // Create span for tracing and set parent context
        let span = info_span!("clear_scheduled");
        span.set_parent(context);

        this.bridge
            .wait_for(ruby, async move { inner.clear_scheduled().await }, span)?
            .map_err(|error| {
                Error::new(
                    runtime_error(),
                    format!("Failed to clear scheduled timers: {error:#}"),
                )
            })
    }

    /// Get all scheduled timer times.
    ///
    /// # Arguments
    ///
    /// * `ruby` - Reference to the Ruby VM
    ///
    /// # Returns
    ///
    /// Array of Ruby Time objects representing all scheduled timer times.
    ///
    /// # Errors
    ///
    /// Returns an error if retrieving scheduled times fails or if
    /// converting times to Ruby objects fails.
    fn scheduled(ruby: &Ruby, this: &Self) -> Result<Value, Error> {
        let inner = this.inner.clone();

        // Extract OpenTelemetry context from Ruby for distributed tracing
        let context = extract_opentelemetry_context(ruby, &this.propagator)?;

        // Create span for tracing and set parent context
        let span = info_span!("scheduled");
        span.set_parent(context);

        // Collect the scheduled times stream into a Vec
        let scheduled_times = this
            .bridge
            .wait_for(
                ruby,
                async move { inner.scheduled().try_collect::<Vec<_>>().await },
                span,
            )?
            .map_err(|e| {
                Error::new(
                    runtime_error(),
                    format!("Failed to get scheduled times: {e}"),
                )
            })?;

        // Convert CompactDateTime objects to Ruby Time objects using idiomatic iterator
        // pattern
        let ruby_array = ruby.ary_try_from_iter(
            scheduled_times
                .into_iter()
                .map(|compact_time| compact_datetime_to_time(ruby, compact_time)),
        )?;

        Ok(ruby_array.as_value())
    }
}

/// Initializes the Context class in Ruby.
///
/// Registers the `Prosody::Context` class in the Ruby runtime, making it
/// available to Ruby code with all timer scheduling methods.
///
/// # Arguments
///
/// * `ruby` - Reference to the Ruby VM
///
/// # Errors
///
/// Returns a Magnus error if the class definition fails
pub fn init(ruby: &Ruby) -> Result<(), Error> {
    let module = ruby.get_inner(&ROOT_MOD);
    let class = module.define_class(id!(ruby, "Context"), ruby.class_object())?;

    // Shutdown methods
    class.define_method(
        id!(ruby, "should_shutdown?"),
        method!(Context::should_shutdown, 0),
    )?;

    // Timer scheduling methods
    class.define_method(id!(ruby, "schedule"), method!(Context::schedule, 1))?;
    class.define_method(
        id!(ruby, "clear_and_schedule"),
        method!(Context::clear_and_schedule, 1),
    )?;
    class.define_method(id!(ruby, "unschedule"), method!(Context::unschedule, 1))?;
    class.define_method(
        id!(ruby, "clear_scheduled"),
        method!(Context::clear_scheduled, 0),
    )?;
    class.define_method(id!(ruby, "scheduled"), method!(Context::scheduled, 0))?;

    Ok(())
}

/// Converts a Ruby Time object to `CompactDateTime`.
///
/// `CompactDateTime` stores only epoch seconds (u32) with second-level
/// precision. This function extracts nanoseconds from Ruby Time to implement
/// proper rounding.
///
/// # Arguments
///
/// * `_ruby` - Reference to the Ruby VM (ensures Ruby thread safety)
/// * `ruby_time` - Ruby Time object to convert
///
/// # Returns
///
/// `CompactDateTime` representing the same time, rounded to nearest second.
///
/// # Errors
///
/// Returns an error if the time is outside the `CompactDateTime` range
/// (1970-2106) or if the Ruby Time object is invalid.
fn time_to_compact_datetime(ruby: &Ruby, ruby_time: Value) -> Result<CompactDateTime, Error> {
    // Extract epoch seconds and nanoseconds from Ruby Time
    let epoch_seconds: i64 = ruby_time.funcall(id!(ruby, "to_i"), ())?;
    let nanos: u32 = ruby_time.funcall(id!(ruby, "nsec"), ())?;

    // Validate CompactDateTime range (1970-2106)
    let seconds_u32 = u32::try_from(epoch_seconds).map_err(|_| {
        Error::new(
            arg_error(),
            format!("Time {epoch_seconds} is outside CompactDateTime range (1970-2106)"),
        )
    })?;

    // Apply CompactDateTime's rounding logic
    let final_seconds = if nanos >= NANOSECOND_ROUNDING_THRESHOLD {
        seconds_u32.checked_add(1).ok_or_else(|| {
            Error::new(
                arg_error(),
                "Time overflow during rounding to nearest second",
            )
        })?
    } else {
        seconds_u32
    };

    Ok(CompactDateTime::from(final_seconds))
}

/// Converts a `CompactDateTime` to a Ruby Time object.
///
/// `CompactDateTime` only stores epoch seconds, so the resulting Ruby Time
/// will have zero nanoseconds. This is the most efficient conversion.
///
/// # Arguments
///
/// * `ruby` - Reference to the Ruby VM
/// * `compact_time` - `CompactDateTime` to convert
///
/// # Returns
///
/// Ruby Time object representing the same time with zero nanoseconds.
///
/// # Errors
///
/// Returns an error if the Ruby Time class cannot be accessed or if
/// creating the Time object fails.
fn compact_datetime_to_time(ruby: &Ruby, compact_time: CompactDateTime) -> Result<Value, Error> {
    // Direct access to epoch seconds - CompactDateTime has no sub-second precision
    let epoch_seconds = i64::from(compact_time.epoch_seconds());

    // Create Ruby Time with zero nanoseconds (CompactDateTime precision limit)
    ruby.module_kernel()
        .const_get::<_, RClass>(id!(ruby, "Time"))?
        .funcall(id!(ruby, "at"), (epoch_seconds,))
}
