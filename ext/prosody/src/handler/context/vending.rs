//! Keyed-state vending for `Prosody::Context`.
//!
//! Each vend method opens the handle for one registered collection in the
//! current event and wraps it in its native handle class. Vending verifies the
//! collection's registration in core. It opens no span: vended handles outlive
//! the call, and every handle operation opens its own span.

use super::Context;
use crate::handler::state::{
    NativeJsonDequeState, NativeJsonMapState, NativeJsonValueState, NativeMessageDequeState,
    NativeMessageMapState, NativeMessageValueState, NativeSetState, state_error,
};
use crate::id;
use magnus::{Error, Module, RClass, RString, Ruby, method};
use std::sync::Arc;

/// Vends the handle for the named JSON value collection.
///
/// # Errors
///
/// Returns a permanent state error if the name is unregistered or its
/// registered identity mismatches.
#[allow(clippy::needless_pass_by_value, reason = "Magnus method argument type")]
fn value_state(ruby: &Ruby, this: &Context, name: String) -> Result<NativeJsonValueState, Error> {
    let handle = this
        .inner
        .value_state(&name)
        .map_err(|error| state_error(ruby, &error))?;
    Ok(NativeJsonValueState::new(
        Arc::from(handle),
        this.bridge.clone(),
        Arc::clone(&this.propagator),
    ))
}

/// Vends the handle for the named JSON map collection.
///
/// # Errors
///
/// See [`value_state`].
#[allow(clippy::needless_pass_by_value, reason = "Magnus method argument type")]
fn map_state(ruby: &Ruby, this: &Context, name: String) -> Result<NativeJsonMapState, Error> {
    let handle = this
        .inner
        .map_state(&name)
        .map_err(|error| state_error(ruby, &error))?;
    Ok(NativeJsonMapState::new(
        Arc::from(handle),
        this.bridge.clone(),
        Arc::clone(&this.propagator),
    ))
}

/// Vends the handle for the named set collection.
///
/// # Errors
///
/// See [`value_state`].
fn set_state(ruby: &Ruby, this: &Context, name: RString) -> Result<NativeSetState, Error> {
    let handle = this
        .inner
        .set_state(&name.to_string()?)
        .map_err(|error| state_error(ruby, &error))?;
    Ok(NativeSetState::new(
        Arc::from(handle),
        this.bridge.clone(),
        Arc::clone(&this.propagator),
    ))
}

/// Vends the handle for the named JSON deque collection.
///
/// # Errors
///
/// See [`value_state`].
#[allow(clippy::needless_pass_by_value, reason = "Magnus method argument type")]
fn deque_state(ruby: &Ruby, this: &Context, name: String) -> Result<NativeJsonDequeState, Error> {
    let handle = this
        .inner
        .deque_state(&name)
        .map_err(|error| state_error(ruby, &error))?;
    Ok(NativeJsonDequeState::new(
        Arc::from(handle),
        this.bridge.clone(),
        Arc::clone(&this.propagator),
    ))
}

/// Vends the handle for the named Kafka-message value collection.
///
/// # Errors
///
/// See [`value_state`].
#[allow(clippy::needless_pass_by_value, reason = "Magnus method argument type")]
fn message_value_state(
    ruby: &Ruby,
    this: &Context,
    name: String,
) -> Result<NativeMessageValueState, Error> {
    let handle = this
        .inner
        .message_value_state(&name)
        .map_err(|error| state_error(ruby, &error))?;
    Ok(NativeMessageValueState::new(
        Arc::from(handle),
        this.bridge.clone(),
        Arc::clone(&this.propagator),
    ))
}

/// Vends the handle for the named Kafka-message map collection.
///
/// # Errors
///
/// See [`value_state`].
#[allow(clippy::needless_pass_by_value, reason = "Magnus method argument type")]
fn message_map_state(
    ruby: &Ruby,
    this: &Context,
    name: String,
) -> Result<NativeMessageMapState, Error> {
    let handle = this
        .inner
        .message_map_state(&name)
        .map_err(|error| state_error(ruby, &error))?;
    Ok(NativeMessageMapState::new(
        Arc::from(handle),
        this.bridge.clone(),
        Arc::clone(&this.propagator),
    ))
}

/// Vends the handle for the named Kafka-message deque collection.
///
/// # Errors
///
/// See [`value_state`].
#[allow(clippy::needless_pass_by_value, reason = "Magnus method argument type")]
fn message_deque_state(
    ruby: &Ruby,
    this: &Context,
    name: String,
) -> Result<NativeMessageDequeState, Error> {
    let handle = this
        .inner
        .message_deque_state(&name)
        .map_err(|error| state_error(ruby, &error))?;
    Ok(NativeMessageDequeState::new(
        Arc::from(handle),
        this.bridge.clone(),
        Arc::clone(&this.propagator),
    ))
}

/// Registers the keyed-state vend methods on the `Prosody::Context` class.
///
/// # Errors
///
/// Returns a Magnus error if a method definition fails.
pub(super) fn define_methods(ruby: &Ruby, class: RClass) -> Result<(), Error> {
    class.define_method(id!(ruby, "value_state"), method!(value_state, 1))?;
    class.define_method(id!(ruby, "map_state"), method!(map_state, 1))?;
    class.define_method(id!(ruby, "set_state"), method!(set_state, 1))?;
    class.define_method(id!(ruby, "deque_state"), method!(deque_state, 1))?;
    class.define_method(
        id!(ruby, "message_value_state"),
        method!(message_value_state, 1),
    )?;
    class.define_method(
        id!(ruby, "message_map_state"),
        method!(message_map_state, 1),
    )?;
    class.define_method(
        id!(ruby, "message_deque_state"),
        method!(message_deque_state, 1),
    )?;
    Ok(())
}
