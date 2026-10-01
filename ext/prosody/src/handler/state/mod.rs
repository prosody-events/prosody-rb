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
use magnus::value::ReprValue;
use magnus::{
    Error, ExceptionClass, IntoValue, Module, RClass, RHash, RModule, Ruby, StaticSymbol,
    TryConvert, Value, method,
};
use opentelemetry::propagation::TextMapCompositePropagator;
use opentelemetry::trace::FutureExt;
use prosody::consumer::event_context::{
    DynDequeState, DynMapState, DynValueState, ErasedCategory, ErasedStateError,
};
use prosody::consumer::message::ConsumerMessage;
use prosody::state::StoreOutcome;
use serde_json::Value as JsonValue;
use serde_magnus::{deserialize, serialize};
use std::sync::Arc;
use tracing::Span;

/// Builds an error of the Ruby-defined `Prosody::<class>` exception class.
///
/// A failed class lookup returns the lookup error in place of the state
/// error.
fn ruby_error(ruby: &Ruby, class: &str, message: String) -> Error {
    match ruby
        .get_inner(&ROOT_MOD)
        .const_get::<_, ExceptionClass>(class)
    {
        Ok(class) => Error::new(class, message),
        Err(error) => error,
    }
}

/// Maps an erased state error to the matching Ruby state-error class.
///
/// The category is data, so this never parses the human message. Because the
/// Ruby classes subclass the existing error hierarchy, the result bridge's
/// `#permanent?` path reclassifies a rethrown error with no change.
pub(crate) fn state_error(ruby: &Ruby, error: &ErasedStateError) -> Error {
    let class = match error.category() {
        ErasedCategory::Permanent => "PermanentStateError",
        ErasedCategory::Transient => "TransientStateError",
    };
    ruby_error(ruby, class, error.message().to_owned())
}

/// Builds a transient state error for a caller-caused condition the glue
/// detects (a wrong item shape, an invalid direction token, an unrepresentable
/// value).
fn transient_state_error(ruby: &Ruby, message: String) -> Error {
    ruby_error(ruby, "TransientStateError", message)
}

/// Converts an optional message item into a `Prosody::Message` or `nil`.
fn message_or_nil(ruby: &Ruby, item: Option<ConsumerMessage<JsonValue>>) -> Value {
    item.map(Message::from).into_value_with(ruby)
}

/// Converts a Ruby argument into a JSON item.
///
/// A value with no JSON representation is a caller mistake and rejects
/// transient. Core rejects a JSON `null` write as permanent.
fn json_write_item(ruby: &Ruby, value: Value) -> Result<JsonValue, Error> {
    deserialize(ruby, value).map_err(|error| {
        transient_state_error(ruby, format!("value is not representable as JSON: {error}"))
    })
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

/// Maps a core [`StoreOutcome`] to `:applied` or `:no_op`.
///
/// `:applied` means the call drained buffered operations. `:no_op` means
/// nothing was buffered.
fn outcome_symbol(ruby: &Ruby, outcome: StoreOutcome) -> StaticSymbol {
    match outcome {
        StoreOutcome::Applied => ruby.sym_new("applied"),
        StoreOutcome::NoOp => ruby.sym_new("no_op"),
    }
}

/// Defines the class that a `magnus::wrap` path such as
/// `"Prosody::NativeSetState"` names.
fn wrapped_class(ruby: &Ruby, module: RModule, path: &str) -> Result<RClass, Error> {
    module.define_class(path.trim_start_matches("Prosody::"), ruby.class_object())
}

/// Waits for one state operation in the caller's trace. An
/// [`ErasedStateError`] raises its matching Ruby class.
pub(crate) fn run_state<F, T>(
    ruby: &Ruby,
    bridge: &Bridge,
    propagator: &Arc<TextMapCompositePropagator>,
    operation: F,
) -> Result<T, Error>
where
    F: Future<Output = Result<T, ErasedStateError>> + Send + 'static,
    T: Send + 'static,
{
    let context = extract_opentelemetry_context(ruby, propagator)?;
    bridge
        .wait_for(ruby, operation.with_context(context), Span::current())?
        .map_err(|error| state_error(ruby, &error))
}

/// Calls one method of an erased handle through [`run_state`].
macro_rules! run_op {
    ($ruby:expr, $this:expr, $handle:expr, $call:ident ( $($arg:expr),* )) => {{
        let handle = Arc::clone($handle);
        run_state($ruby, &$this.bridge, &$this.propagator, async move {
            handle.$call($($arg),*).await
        })
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

            fn set(ruby: &Ruby, this: &Self, value: Value) -> Result<(), Error> {
                let item = ($prepare)(ruby, value)?;
                run_op!(ruby, this, &this.state, set(item))
            }

            fn clear(ruby: &Ruby, this: &Self) -> Result<(), Error> {
                run_op!(ruby, this, &this.state, clear())
            }

            fn commit(ruby: &Ruby, this: &Self) -> Result<StaticSymbol, Error> {
                let outcome = run_op!(ruby, this, &this.state, commit())?;
                Ok(outcome_symbol(ruby, outcome))
            }

            fn rollback(ruby: &Ruby, this: &Self) -> Result<StaticSymbol, Error> {
                let state = Arc::clone(&this.state);
                let outcome = run_state(ruby, &this.bridge, &this.propagator, async move {
                    Ok(state.rollback().await)
                })?;
                Ok(outcome_symbol(ruby, outcome))
            }

            pub(super) fn register(ruby: &Ruby, module: RModule) -> Result<(), Error> {
                let class = wrapped_class(ruby, module, $class)?;
                class.define_method("get", method!($name::get, 0))?;
                class.define_method("set", method!($name::set, 1))?;
                class.define_method("clear", method!($name::clear, 0))?;
                class.define_method("commit", method!($name::commit, 0))?;
                class.define_method("rollback", method!($name::rollback, 0))?;
                Ok(())
            }
        }
    };
}

value_state!(
    NativeJsonValueState,
    "Prosody::NativeJsonValueState",
    JsonValue,
    json_write_item,
    |ruby, item| serialize::<_, Value>(ruby, &item)
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
    |ruby, item| Ok::<_, Error>(message_or_nil(ruby, item))
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

            fn contains_key(ruby: &Ruby, this: &Self, key: String) -> Result<bool, Error> {
                run_op!(ruby, this, &this.state, contains_key(key))
            }

            fn is_empty(ruby: &Ruby, this: &Self) -> Result<bool, Error> {
                run_op!(ruby, this, &this.state, is_empty())
            }

            fn get_many(ruby: &Ruby, this: &Self, keys: Vec<String>) -> Result<Value, Error> {
                let items = run_op!(ruby, this, &this.state, get_many(keys))?;
                let array =
                    ruby.ary_try_from_iter(items.into_iter().map(|item| ($restore)(ruby, item)))?;
                Ok(array.as_value())
            }

            fn contains_many(
                ruby: &Ruby,
                this: &Self,
                keys: Vec<String>,
            ) -> Result<Vec<bool>, Error> {
                run_op!(ruby, this, &this.state, contains_many(keys))
            }

            fn set(ruby: &Ruby, this: &Self, key: String, value: Value) -> Result<(), Error> {
                let item = ($prepare)(ruby, value)?;
                run_op!(ruby, this, &this.state, set(key, item))
            }

            fn remove(ruby: &Ruby, this: &Self, key: String) -> Result<(), Error> {
                run_op!(ruby, this, &this.state, remove(key))
            }

            fn clear(ruby: &Ruby, this: &Self) -> Result<(), Error> {
                run_op!(ruby, this, &this.state, clear())
            }

            fn scan(
                ruby: &Ruby,
                this: &Self,
                direction: StaticSymbol,
                options: RHash,
            ) -> Result<$scan, Error> {
                let query = key_query(ruby, direction, options)?;
                Ok($scan::new(
                    this.state.entries().with_query(query).stream(),
                    this.bridge.clone(),
                    Arc::clone(&this.propagator),
                ))
            }

            fn keys(
                ruby: &Ruby,
                this: &Self,
                direction: StaticSymbol,
                options: RHash,
            ) -> Result<NativeMapKeyScan, Error> {
                let query = key_query(ruby, direction, options)?;
                Ok(NativeMapKeyScan::new(
                    this.state.keys().with_query(query).stream(),
                    this.bridge.clone(),
                    Arc::clone(&this.propagator),
                ))
            }

            fn commit(ruby: &Ruby, this: &Self) -> Result<StaticSymbol, Error> {
                let outcome = run_op!(ruby, this, &this.state, commit())?;
                Ok(outcome_symbol(ruby, outcome))
            }

            fn rollback(ruby: &Ruby, this: &Self) -> Result<StaticSymbol, Error> {
                let state = Arc::clone(&this.state);
                let outcome = run_state(ruby, &this.bridge, &this.propagator, async move {
                    Ok(state.rollback().await)
                })?;
                Ok(outcome_symbol(ruby, outcome))
            }

            pub(super) fn register(ruby: &Ruby, module: RModule) -> Result<(), Error> {
                let class = wrapped_class(ruby, module, $class)?;
                class.define_method("get", method!($name::get, 1))?;
                class.define_method("contains_key", method!($name::contains_key, 1))?;
                class.define_method("is_empty", method!($name::is_empty, 0))?;
                class.define_method("get_many", method!($name::get_many, 1))?;
                class.define_method("contains_many", method!($name::contains_many, 1))?;
                class.define_method("set", method!($name::set, 2))?;
                class.define_method("remove", method!($name::remove, 1))?;
                class.define_method("clear", method!($name::clear, 0))?;
                class.define_method("scan", method!($name::scan, 2))?;
                class.define_method("keys", method!($name::keys, 2))?;
                class.define_method("commit", method!($name::commit, 0))?;
                class.define_method("rollback", method!($name::rollback, 0))?;
                Ok(())
            }
        }
    };
}

map_state!(
    NativeJsonMapState,
    "Prosody::NativeJsonMapState",
    JsonValue,
    NativeJsonMapScan,
    json_write_item,
    |ruby, item| serialize::<_, Value>(ruby, &item)
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
    |ruby, item| Ok::<_, Error>(message_or_nil(ruby, item))
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

            fn len(ruby: &Ruby, this: &Self) -> Result<usize, Error> {
                run_op!(ruby, this, &this.state, len())
            }

            fn is_empty(ruby: &Ruby, this: &Self) -> Result<bool, Error> {
                run_op!(ruby, this, &this.state, is_empty())
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

            fn push_back(ruby: &Ruby, this: &Self, value: Value) -> Result<(), Error> {
                let item = ($prepare)(ruby, value)?;
                run_op!(ruby, this, &this.state, push_back(item))
            }

            fn push_front(ruby: &Ruby, this: &Self, value: Value) -> Result<(), Error> {
                let item = ($prepare)(ruby, value)?;
                run_op!(ruby, this, &this.state, push_front(item))
            }

            fn pop_front(ruby: &Ruby, this: &Self) -> Result<Value, Error> {
                ($restore)(ruby, run_op!(ruby, this, &this.state, pop_front())?)
            }

            fn pop_back(ruby: &Ruby, this: &Self) -> Result<Value, Error> {
                ($restore)(ruby, run_op!(ruby, this, &this.state, pop_back())?)
            }

            fn clear(ruby: &Ruby, this: &Self) -> Result<(), Error> {
                run_op!(ruby, this, &this.state, clear())
            }

            fn scan(
                ruby: &Ruby,
                this: &Self,
                direction: StaticSymbol,
                options: RHash,
            ) -> Result<$scan, Error> {
                let query = position_query(ruby, direction, options)?;
                Ok($scan::new(
                    this.state.values().with_query(query).stream(),
                    this.bridge.clone(),
                    Arc::clone(&this.propagator),
                ))
            }

            fn commit(ruby: &Ruby, this: &Self) -> Result<StaticSymbol, Error> {
                let outcome = run_op!(ruby, this, &this.state, commit())?;
                Ok(outcome_symbol(ruby, outcome))
            }

            fn rollback(ruby: &Ruby, this: &Self) -> Result<StaticSymbol, Error> {
                let state = Arc::clone(&this.state);
                let outcome = run_state(ruby, &this.bridge, &this.propagator, async move {
                    Ok(state.rollback().await)
                })?;
                Ok(outcome_symbol(ruby, outcome))
            }

            pub(super) fn register(ruby: &Ruby, module: RModule) -> Result<(), Error> {
                let class = wrapped_class(ruby, module, $class)?;
                class.define_method("len", method!($name::len, 0))?;
                class.define_method("is_empty", method!($name::is_empty, 0))?;
                class.define_method("get", method!($name::get, 1))?;
                class.define_method("peek_front", method!($name::peek_front, 0))?;
                class.define_method("peek_back", method!($name::peek_back, 0))?;
                class.define_method("push_back", method!($name::push_back, 1))?;
                class.define_method("push_front", method!($name::push_front, 1))?;
                class.define_method("pop_front", method!($name::pop_front, 0))?;
                class.define_method("pop_back", method!($name::pop_back, 0))?;
                class.define_method("clear", method!($name::clear, 0))?;
                class.define_method("scan", method!($name::scan, 2))?;
                class.define_method("commit", method!($name::commit, 0))?;
                class.define_method("rollback", method!($name::rollback, 0))?;
                Ok(())
            }
        }
    };
}

deque_state!(
    NativeJsonDequeState,
    "Prosody::NativeJsonDequeState",
    JsonValue,
    NativeJsonDequeScan,
    json_write_item,
    |ruby, item| serialize::<_, Value>(ruby, &item)
);
deque_state!(
    NativeMessageDequeState,
    "Prosody::NativeMessageDequeState",
    ConsumerMessage<JsonValue>,
    NativeMessageDequeScan,
    |ruby, value| message_write_item(ruby, value, " to push into a message deque"),
    |ruby, item| Ok::<_, Error>(message_or_nil(ruby, item))
);
mod query;
mod scan;
mod set;

pub(crate) use query::{key_query, position_query};
pub(crate) use scan::{
    NativeJsonDequeScan, NativeJsonMapScan, NativeMapKeyScan, NativeMessageDequeScan,
    NativeMessageMapScan,
};
pub(crate) use set::NativeSetState;
mod registration;

pub(crate) use registration::register;
