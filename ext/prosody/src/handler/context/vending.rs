//! Keyed-state vending for `Prosody::Context`.
//!
//! Each vend method opens the handle for one registered collection in the
//! current event and wraps it in its native handle class. Vending verifies the
//! collection's registration in core. It opens no span: vended handles outlive
//! the call, and every handle operation opens its own span.

use super::Context;
use crate::bridge::Bridge;
use crate::handler::state::{
    NativeJsonDequeState, NativeJsonMapState, NativeJsonValueState, NativeMessageDequeState,
    NativeMessageMapState, NativeMessageValueState, NativeSetState, state_error,
};
use crate::id;
use magnus::{Error, Module, RClass, RString, Ruby, method};
use opentelemetry::propagation::TextMapCompositePropagator;
use prosody::consumer::event_context::ErasedStateError;
use std::sync::Arc;

/// Wraps a core handle in its native handle class.
///
/// # Errors
///
/// Returns a permanent state error if the name is unregistered or its
/// registered identity mismatches.
fn vend<T: ?Sized, N>(
    ruby: &Ruby,
    this: &Context,
    handle: Result<Box<T>, ErasedStateError>,
    new: fn(Arc<T>, Bridge, Arc<TextMapCompositePropagator>) -> N,
) -> Result<N, Error> {
    let handle = handle.map_err(|error| state_error(ruby, &error))?;
    Ok(new(
        Arc::from(handle),
        this.bridge.clone(),
        Arc::clone(&this.propagator),
    ))
}

fn value_state(ruby: &Ruby, this: &Context, name: RString) -> Result<NativeJsonValueState, Error> {
    vend(
        ruby,
        this,
        this.inner.value_state(&name.to_string()?),
        NativeJsonValueState::new,
    )
}

fn map_state(ruby: &Ruby, this: &Context, name: RString) -> Result<NativeJsonMapState, Error> {
    vend(
        ruby,
        this,
        this.inner.map_state(&name.to_string()?),
        NativeJsonMapState::new,
    )
}

fn set_state(ruby: &Ruby, this: &Context, name: RString) -> Result<NativeSetState, Error> {
    vend(
        ruby,
        this,
        this.inner.set_state(&name.to_string()?),
        NativeSetState::new,
    )
}

fn deque_state(ruby: &Ruby, this: &Context, name: RString) -> Result<NativeJsonDequeState, Error> {
    vend(
        ruby,
        this,
        this.inner.deque_state(&name.to_string()?),
        NativeJsonDequeState::new,
    )
}

fn message_value_state(
    ruby: &Ruby,
    this: &Context,
    name: RString,
) -> Result<NativeMessageValueState, Error> {
    vend(
        ruby,
        this,
        this.inner.message_value_state(&name.to_string()?),
        NativeMessageValueState::new,
    )
}

fn message_map_state(
    ruby: &Ruby,
    this: &Context,
    name: RString,
) -> Result<NativeMessageMapState, Error> {
    vend(
        ruby,
        this,
        this.inner.message_map_state(&name.to_string()?),
        NativeMessageMapState::new,
    )
}

fn message_deque_state(
    ruby: &Ruby,
    this: &Context,
    name: RString,
) -> Result<NativeMessageDequeState, Error> {
    vend(
        ruby,
        this,
        this.inner.message_deque_state(&name.to_string()?),
        NativeMessageDequeState::new,
    )
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
