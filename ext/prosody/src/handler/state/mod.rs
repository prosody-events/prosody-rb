//! Concrete native handles for keyed state.
//!
//! Each Magnus class holds one collection type and one payload type. JSON
//! values use `serde_magnus`. Message collections return the same [`Message`]
//! objects that handlers receive.
//!
//! [`Bridge::wait_for`] yields the calling fiber while tokio drives each
//! operation. The active OpenTelemetry carrier joins the core operation span.
//!
//! [`ErasedStateError::category`] selects the Ruby error class. Caller mistakes
//! remain transient, so Prosody does not discard the message.

use crate::ROOT_MOD;
use crate::bridge::Bridge;
use crate::handler::message::Message;
use crate::tracing_util::extract_opentelemetry_context;
use magnus::value::{Lazy, ReprValue};
use magnus::{Error, ExceptionClass, IntoValue, Module, Ruby, StaticSymbol, TryConvert, Value};
use opentelemetry::propagation::TextMapCompositePropagator;
use opentelemetry::trace::FutureExt;
use prosody::consumer::event_context::{
    DynDequeState, DynMapState, DynValueState, ErasedCategory, ErasedStateError,
};
use prosody::consumer::message::ConsumerMessage;
use prosody::state::Direction;
use serde_json::Value as JsonValue;
use serde_magnus::{deserialize, serialize};
use std::sync::Arc;
use tracing::Span;

/// Lazily resolved `Prosody::PermanentStateError` class (defined in Ruby).
#[allow(
    clippy::expect_used,
    reason = "mirrors bridge.rs QUEUE_CLASS Lazy pattern"
)]
static PERMANENT_STATE_ERROR: Lazy<ExceptionClass> = Lazy::new(|ruby| {
    ruby.get_inner(&ROOT_MOD)
        .const_get("PermanentStateError")
        .expect("Prosody::PermanentStateError")
});

/// Lazily resolved `Prosody::TransientStateError` class (defined in Ruby).
#[allow(
    clippy::expect_used,
    reason = "mirrors bridge.rs QUEUE_CLASS Lazy pattern"
)]
static TRANSIENT_STATE_ERROR: Lazy<ExceptionClass> = Lazy::new(|ruby| {
    ruby.get_inner(&ROOT_MOD)
        .const_get("TransientStateError")
        .expect("Prosody::TransientStateError")
});

/// Lazily resolved `Prosody::NullValueError` class (defined in Ruby).
#[allow(
    clippy::expect_used,
    reason = "mirrors bridge.rs QUEUE_CLASS Lazy pattern"
)]
static NULL_VALUE_ERROR: Lazy<ExceptionClass> = Lazy::new(|ruby| {
    ruby.get_inner(&ROOT_MOD)
        .const_get("NullValueError")
        .expect("Prosody::NullValueError")
});

/// Maps an erased state error to the matching Ruby state-error class.
///
/// The category is data, so this never parses the human message. Because the
/// Ruby classes subclass the existing error hierarchy, the result bridge's
/// `#permanent?` path reclassifies a rethrown error with no change.
pub(crate) fn state_error(ruby: &Ruby, error: &ErasedStateError) -> Error {
    let class = match error.category() {
        ErasedCategory::Permanent => ruby.get_inner(&PERMANENT_STATE_ERROR),
        ErasedCategory::Transient => ruby.get_inner(&TRANSIENT_STATE_ERROR),
    };
    Error::new(class, error.message().to_owned())
}

/// Builds a transient state error for a caller-caused condition the glue
/// detects (a wrong item shape, an invalid direction token, an unrepresentable
/// value).
fn transient_state_error(ruby: &Ruby, message: String) -> Error {
    Error::new(ruby.get_inner(&TRANSIENT_STATE_ERROR), message)
}

/// Builds a null-value error for a rejected JSON-null write.
fn null_value_error(ruby: &Ruby, message: String) -> Error {
    Error::new(ruby.get_inner(&NULL_VALUE_ERROR), message)
}

/// Converts an optional JSON item into a Ruby value or `nil`.
fn json_or_nil(ruby: &Ruby, item: Option<JsonValue>) -> Result<Value, Error> {
    match item {
        Some(value) => serialize(ruby, &value),
        None => Ok(ruby.qnil().as_value()),
    }
}

/// Converts an optional message item into a `Prosody::Message` or `nil`.
#[allow(
    clippy::unnecessary_wraps,
    reason = "parity with json_or_nil for uniform call sites"
)]
fn message_or_nil(ruby: &Ruby, item: Option<ConsumerMessage<JsonValue>>) -> Result<Value, Error> {
    match item {
        Some(message) => Ok(Message::from(message).into_value_with(ruby)),
        None => Ok(ruby.qnil().as_value()),
    }
}

/// Converts a Ruby argument into a storable JSON item.
///
/// A value with no JSON representation is a caller mistake and rejects
/// transient. JSON `null` is rejected with a [`crate::NullValueError`] naming
/// the deletion verb via `null_advice`.
fn json_write_item(ruby: &Ruby, value: Value, null_advice: &str) -> Result<JsonValue, Error> {
    let value: JsonValue = deserialize(ruby, value).map_err(|error| {
        transient_state_error(ruby, format!("value is not representable as JSON: {error}"))
    })?;
    if value.is_null() {
        return Err(null_value_error(
            ruby,
            format!("JSON null is not a storable value{null_advice}"),
        ));
    }
    Ok(value)
}

/// Converts a Ruby argument into a storable message item.
///
/// A non-message argument is a caller mistake (a JSON payload cannot be stored
/// in a message collection) and rejects transient. The wrapped
/// [`ConsumerMessage`] is cloned; see [`Message::consumer_message`].
fn message_write_item(
    ruby: &Ruby,
    value: Value,
    shape_advice: &str,
) -> Result<ConsumerMessage<JsonValue>, Error> {
    let message = <&Message>::try_convert(value).map_err(|_| {
        transient_state_error(ruby, format!("expected a Prosody::Message{shape_advice}"))
    })?;
    Ok(message.consumer_message())
}

/// Parses a scan-direction token into the core [`Direction`].
///
/// An invalid token is a caller mistake and rejects transient.
pub(crate) fn parse_direction(ruby: &Ruby, direction: StaticSymbol) -> Result<Direction, Error> {
    match direction.name()? {
        "forward" => Ok(Direction::Forward),
        "backward" => Ok(Direction::Backward),
        other => Err(transient_state_error(
            ruby,
            format!("direction: expected :forward or :backward, got :{other}"),
        )),
    }
}

/// Drives an erased async op that returns `Result<_, ErasedStateError>` through
/// [`Bridge::wait_for`] with the extracted carrier active, yielding the op's
/// `Ok` value (state error mapped to the matching Ruby class).
macro_rules! run_op {
    ($ruby:expr, $this:expr, $handle:expr, $call:ident ( $($arg:expr),* )) => {{
        let handle = Arc::clone($handle);
        let context = extract_opentelemetry_context($ruby, &$this.propagator)?;
        $this
            .bridge
            .wait_for(
                $ruby,
                async move { handle.$call($($arg),*).with_context(context).await },
                Span::current(),
            )?
            .map_err(|error| state_error($ruby, &error))
    }};
}

/// Drives an infallible erased async op (returning `()`) through
/// [`Bridge::wait_for`] with the extracted carrier active.
macro_rules! run_infallible {
    ($ruby:expr, $this:expr, $handle:expr, $call:ident ()) => {{
        let handle = Arc::clone($handle);
        let context = extract_opentelemetry_context($ruby, &$this.propagator)?;
        $this.bridge.wait_for(
            $ruby,
            async move { handle.$call().with_context(context).await },
            Span::current(),
        )?
    }};
}

macro_rules! value_state {
    ($name:ident, $class:literal, $item:ty, $prepare:expr, $restore:expr) => {
        /// Native single-value handle with one payload type.
        #[magnus::wrap(class = $class)]
        pub struct $name {
            state: Arc<dyn DynValueState<$item>>,
            bridge: Bridge,
            propagator: Arc<TextMapCompositePropagator>,
        }

        impl $name {
            pub(crate) fn new(
                state: Arc<dyn DynValueState<$item>>,
                bridge: Bridge,
                propagator: Arc<TextMapCompositePropagator>,
            ) -> Self {
                Self {
                    state,
                    bridge,
                    propagator,
                }
            }

            fn get(ruby: &Ruby, this: &Self) -> Result<Value, Error> {
                ($restore)(ruby, run_op!(ruby, this, &this.state, get())?)
            }

            fn set(ruby: &Ruby, this: &Self, value: Value) -> Result<Value, Error> {
                let item = ($prepare)(ruby, value)?;
                run_op!(ruby, this, &this.state, set(item))?;
                Ok(ruby.qnil().as_value())
            }

            fn clear(ruby: &Ruby, this: &Self) -> Result<Value, Error> {
                run_op!(ruby, this, &this.state, clear())?;
                Ok(ruby.qnil().as_value())
            }

            fn commit(ruby: &Ruby, this: &Self) -> Result<Value, Error> {
                run_op!(ruby, this, &this.state, commit())?;
                Ok(ruby.qnil().as_value())
            }

            fn rollback(ruby: &Ruby, this: &Self) -> Result<Value, Error> {
                run_infallible!(ruby, this, &this.state, rollback());
                Ok(ruby.qnil().as_value())
            }
        }
    };
}

value_state!(
    NativeJsonValueState,
    "Prosody::NativeJsonValueState",
    JsonValue,
    |ruby, value| json_write_item(ruby, value, "; use clear to remove the value"),
    json_or_nil
);
value_state!(
    NativeMessageValueState,
    "Prosody::NativeMessageValueState",
    ConsumerMessage<JsonValue>,
    |ruby, value| message_write_item(
        ruby,
        value,
        "; use clear to delete a message value collection"
    ),
    message_or_nil
);

macro_rules! map_state {
    ($name:ident, $class:literal, $item:ty, $scan:ident, $prepare:expr, $restore:expr) => {
        /// Native ordered-map handle with one payload type.
        #[magnus::wrap(class = $class)]
        pub struct $name {
            state: Arc<dyn DynMapState<$item>>,
            bridge: Bridge,
            propagator: Arc<TextMapCompositePropagator>,
        }

        impl $name {
            pub(crate) fn new(
                state: Arc<dyn DynMapState<$item>>,
                bridge: Bridge,
                propagator: Arc<TextMapCompositePropagator>,
            ) -> Self {
                Self {
                    state,
                    bridge,
                    propagator,
                }
            }

            fn get(ruby: &Ruby, this: &Self, key: String) -> Result<Value, Error> {
                ($restore)(ruby, run_op!(ruby, this, &this.state, get(key))?)
            }

            fn contains_key(ruby: &Ruby, this: &Self, key: String) -> Result<Value, Error> {
                Ok(run_op!(ruby, this, &this.state, contains_key(key))?.into_value_with(ruby))
            }

            fn get_many(ruby: &Ruby, this: &Self, keys: Vec<String>) -> Result<Value, Error> {
                let items = run_op!(ruby, this, &this.state, get_many(keys))?;
                let array =
                    ruby.ary_try_from_iter(items.into_iter().map(|item| ($restore)(ruby, item)))?;
                Ok(array.as_value())
            }

            fn set(ruby: &Ruby, this: &Self, key: String, value: Value) -> Result<Value, Error> {
                let item = ($prepare)(ruby, value)?;
                run_op!(ruby, this, &this.state, set(key, item))?;
                Ok(ruby.qnil().as_value())
            }

            fn remove(ruby: &Ruby, this: &Self, key: String) -> Result<Value, Error> {
                run_op!(ruby, this, &this.state, remove(key))?;
                Ok(ruby.qnil().as_value())
            }

            fn clear(ruby: &Ruby, this: &Self) -> Result<Value, Error> {
                run_op!(ruby, this, &this.state, clear())?;
                Ok(ruby.qnil().as_value())
            }

            fn scan(ruby: &Ruby, this: &Self, direction: StaticSymbol) -> Result<$scan, Error> {
                let direction = parse_direction(ruby, direction)?;
                let _guard = extract_opentelemetry_context(ruby, &this.propagator)?.attach();
                $scan::new(
                    ruby,
                    this.state.scan(direction),
                    this.bridge.clone(),
                    Arc::clone(&this.propagator),
                )
            }

            fn keys(
                ruby: &Ruby,
                this: &Self,
                direction: StaticSymbol,
            ) -> Result<NativeMapKeyScan, Error> {
                let direction = parse_direction(ruby, direction)?;
                let _guard = extract_opentelemetry_context(ruby, &this.propagator)?.attach();
                NativeMapKeyScan::new(
                    ruby,
                    this.state.keys(direction),
                    this.bridge.clone(),
                    Arc::clone(&this.propagator),
                )
            }

            fn commit(ruby: &Ruby, this: &Self) -> Result<Value, Error> {
                run_op!(ruby, this, &this.state, commit())?;
                Ok(ruby.qnil().as_value())
            }

            fn rollback(ruby: &Ruby, this: &Self) -> Result<Value, Error> {
                run_infallible!(ruby, this, &this.state, rollback());
                Ok(ruby.qnil().as_value())
            }
        }
    };
}

map_state!(
    NativeJsonMapState,
    "Prosody::NativeJsonMapState",
    JsonValue,
    NativeJsonMapScan,
    |ruby, value| json_write_item(ruby, value, "; use delete(key) to remove the entry"),
    json_or_nil
);
map_state!(
    NativeMessageMapState,
    "Prosody::NativeMessageMapState",
    ConsumerMessage<JsonValue>,
    NativeMessageMapScan,
    |ruby, value| message_write_item(
        ruby,
        value,
        "; use delete(key) to remove a message map entry"
    ),
    message_or_nil
);
macro_rules! deque_state {
    ($name:ident, $class:literal, $item:ty, $scan:ident, $prepare:expr, $restore:expr) => {
        /// Native deque handle with one payload type.
        #[magnus::wrap(class = $class)]
        pub struct $name {
            state: Arc<dyn DynDequeState<$item>>,
            bridge: Bridge,
            propagator: Arc<TextMapCompositePropagator>,
        }

        impl $name {
            pub(crate) fn new(
                state: Arc<dyn DynDequeState<$item>>,
                bridge: Bridge,
                propagator: Arc<TextMapCompositePropagator>,
            ) -> Self {
                Self {
                    state,
                    bridge,
                    propagator,
                }
            }

            fn len(ruby: &Ruby, this: &Self) -> Result<Value, Error> {
                Ok(run_op!(ruby, this, &this.state, len())?.into_value_with(ruby))
            }

            fn is_empty(ruby: &Ruby, this: &Self) -> Result<Value, Error> {
                Ok(run_op!(ruby, this, &this.state, is_empty())?.into_value_with(ruby))
            }

            fn get(ruby: &Ruby, this: &Self, index: usize) -> Result<Value, Error> {
                ($restore)(ruby, run_op!(ruby, this, &this.state, get(index))?)
            }

            fn peek_front(ruby: &Ruby, this: &Self) -> Result<Value, Error> {
                ($restore)(ruby, run_op!(ruby, this, &this.state, peek_front())?)
            }

            fn peek_back(ruby: &Ruby, this: &Self) -> Result<Value, Error> {
                ($restore)(ruby, run_op!(ruby, this, &this.state, peek_back())?)
            }

            fn push_back(ruby: &Ruby, this: &Self, value: Value) -> Result<Value, Error> {
                let item = ($prepare)(ruby, value)?;
                run_op!(ruby, this, &this.state, push_back(item))?;
                Ok(ruby.qnil().as_value())
            }

            fn push_front(ruby: &Ruby, this: &Self, value: Value) -> Result<Value, Error> {
                let item = ($prepare)(ruby, value)?;
                run_op!(ruby, this, &this.state, push_front(item))?;
                Ok(ruby.qnil().as_value())
            }

            fn pop_front(ruby: &Ruby, this: &Self) -> Result<Value, Error> {
                ($restore)(ruby, run_op!(ruby, this, &this.state, pop_front())?)
            }

            fn pop_back(ruby: &Ruby, this: &Self) -> Result<Value, Error> {
                ($restore)(ruby, run_op!(ruby, this, &this.state, pop_back())?)
            }

            fn clear(ruby: &Ruby, this: &Self) -> Result<Value, Error> {
                run_op!(ruby, this, &this.state, clear())?;
                Ok(ruby.qnil().as_value())
            }

            fn scan(ruby: &Ruby, this: &Self, direction: StaticSymbol) -> Result<$scan, Error> {
                let direction = parse_direction(ruby, direction)?;
                let _guard = extract_opentelemetry_context(ruby, &this.propagator)?.attach();
                $scan::new(
                    ruby,
                    this.state.scan(direction),
                    this.bridge.clone(),
                    Arc::clone(&this.propagator),
                )
            }

            fn commit(ruby: &Ruby, this: &Self) -> Result<Value, Error> {
                run_op!(ruby, this, &this.state, commit())?;
                Ok(ruby.qnil().as_value())
            }

            fn rollback(ruby: &Ruby, this: &Self) -> Result<Value, Error> {
                run_infallible!(ruby, this, &this.state, rollback());
                Ok(ruby.qnil().as_value())
            }
        }
    };
}

deque_state!(
    NativeJsonDequeState,
    "Prosody::NativeJsonDequeState",
    JsonValue,
    NativeJsonDequeScan,
    |ruby, value| json_write_item(ruby, value, " in a deque"),
    json_or_nil
);
deque_state!(
    NativeMessageDequeState,
    "Prosody::NativeMessageDequeState",
    ConsumerMessage<JsonValue>,
    NativeMessageDequeScan,
    |ruby, value| message_write_item(ruby, value, " to push into a message deque"),
    message_or_nil
);
mod scan;

pub(crate) use scan::{
    NativeJsonDequeScan, NativeJsonMapScan, NativeMapKeyScan, NativeMessageDequeScan,
    NativeMessageMapScan, published_deque_scan, published_map_key_scan, published_map_scan,
};
mod registration;

pub(crate) use registration::register;
