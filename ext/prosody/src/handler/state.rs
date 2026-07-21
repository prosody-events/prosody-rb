//! Native layer for keyed state.
//!
//! Wraps the boxed erased handles from [`prosody::consumer::event_context`] as
//! Magnus classes. Collections are addressed by name; JSON payloads cross as
//! `serde_json::Value` via `serde_magnus` (exactly like [`Message`] payloads),
//! and Kafka-message items cross as the same [`Message`] object handlers
//! already receive.
//!
//! Every operation flows through [`Bridge::wait_for`]: the calling fiber yields
//! (via the fiber-scheduler-integrated `Queue#pop`) while tokio drives the
//! erased future, so the call looks blocking but never blocks the thread. The
//! extracted Ruby OpenTelemetry carrier is activated
//! (`FutureExt::with_context`) while core polls, so core's one semantic
//! collection span joins the event trace with no binding span.
//! Ruby↔`serde_json::Value` conversion runs on the Ruby thread *after*
//! `wait_for` returns, mirroring [`Message::payload`].
//!
//! Errors are STRUCTURAL: [`ErasedStateError::category`] selects the Ruby class
//! directly, and because [`crate::PermanentStateError`]/`TransientStateError`
//! subclass the existing `PermanentError`/`TransientError`, a rethrown state
//! error classifies correctly with no result-bridge change. Every caller
//! mistake the glue detects (a null write, a wrong item shape, an invalid
//! direction token, an unrepresentable value) rejects TRANSIENT so the message
//! retries and stays visible rather than being discarded.
//!
//! # Cancellation honesty
//!
//! `Async::Stop` may unwind the waiting fiber while the dispatched tokio op
//! completes detached — its effect landed before the boundary or is
//! epoch-fenced by core, and the result channel is simply dropped. For a
//! [`StateScan`] pull, the orphaned chunk is lost, but on cancellation the
//! whole attempt aborts and the scan is closed via the Ruby `ensure`, so the
//! dropped chunk is moot. Adding an in-flight replay slot would be new
//! architecture and would reimplement cancellation safety the contract assigns
//! to core.

use crate::bridge::{Bridge, QUEUE_CLASS};
use crate::handler::message::Message;
use crate::tracing_util::extract_opentelemetry_context;
use crate::util::ThreadSafeValue;
use crate::{ROOT_MOD, id};
use magnus::value::{Lazy, ReprValue};
use magnus::{Error, ExceptionClass, IntoValue, Module, Ruby, TryConvert, Value, method};
use opentelemetry::propagation::TextMapCompositePropagator;
use opentelemetry::trace::FutureExt;
use prosody::consumer::event_context::{
    DynDequeState, DynMapState, DynValueState, ErasedCategory, ErasedStateError, StateCursor,
};
use prosody::consumer::message::ConsumerMessage;
use prosody::state::Direction;
use serde_json::Value as JsonValue;
use serde_magnus::{deserialize, serialize};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::num::NonZeroUsize;
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

/// Maximum number of immediately-ready scan items transported in one chunk.
/// Core owns ready-draining, error ordering, and pull serialization; this
/// binding owns only the transport cap and conversion.
#[allow(clippy::unwrap_used, reason = "256 is a nonzero literal; mirrors core")]
const SCAN_READY_CHUNK_SIZE: NonZeroUsize = NonZeroUsize::new(256).unwrap();

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
fn parse_direction(ruby: &Ruby, direction: &str) -> Result<Direction, Error> {
    match direction {
        "forward" => Ok(Direction::Forward),
        "backward" => Ok(Direction::Backward),
        other => Err(transient_state_error(
            ruby,
            format!("direction: expected \"forward\" or \"backward\", got {other:?}"),
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

/// The two payload flavours a value handle wraps.
pub(crate) enum ValueStateVariant {
    /// A JSON value collection.
    Json(Arc<dyn DynValueState<JsonValue>>),
    /// A Kafka-message collection.
    Message(Arc<dyn DynValueState<ConsumerMessage<JsonValue>>>),
}

/// The two payload flavours a map handle wraps.
pub(crate) enum MapStateVariant {
    /// A JSON value collection.
    Json(Arc<dyn DynMapState<JsonValue>>),
    /// A Kafka-message collection.
    Message(Arc<dyn DynMapState<ConsumerMessage<JsonValue>>>),
}

/// The two payload flavours a deque handle wraps.
pub(crate) enum DequeStateVariant {
    /// A JSON value collection.
    Json(Arc<dyn DynDequeState<JsonValue>>),
    /// A Kafka-message collection.
    Message(Arc<dyn DynDequeState<ConsumerMessage<JsonValue>>>),
}

/// Native single-value state handle, vended per event.
#[magnus::wrap(class = "Prosody::NativeValueState")]
pub struct NativeValueState {
    state: ValueStateVariant,
    bridge: Bridge,
    propagator: Arc<TextMapCompositePropagator>,
}

impl NativeValueState {
    /// Wraps a vended value handle with the bridge and propagator.
    pub(crate) fn new(
        state: ValueStateVariant,
        bridge: Bridge,
        propagator: Arc<TextMapCompositePropagator>,
    ) -> Self {
        Self {
            state,
            bridge,
            propagator,
        }
    }

    /// Reads the current value (`nil` when absent).
    fn get(ruby: &Ruby, this: &Self) -> Result<Value, Error> {
        match &this.state {
            ValueStateVariant::Json(handle) => {
                json_or_nil(ruby, run_op!(ruby, this, handle, get())?)
            }
            ValueStateVariant::Message(handle) => {
                message_or_nil(ruby, run_op!(ruby, this, handle, get())?)
            }
        }
    }

    /// Buffers a write of the value. Rejects JSON null and item-shape mismatch.
    fn set(ruby: &Ruby, this: &Self, value: Value) -> Result<Value, Error> {
        match &this.state {
            ValueStateVariant::Json(handle) => {
                let item = json_write_item(ruby, value, "; use clear to remove the value")?;
                run_op!(ruby, this, handle, set(item))?;
            }
            ValueStateVariant::Message(handle) => {
                let item = message_write_item(
                    ruby,
                    value,
                    "; use clear to delete a message value collection",
                )?;
                run_op!(ruby, this, handle, set(item))?;
            }
        }
        Ok(ruby.qnil().as_value())
    }

    /// Buffers a clear of the value.
    fn clear(ruby: &Ruby, this: &Self) -> Result<Value, Error> {
        match &this.state {
            ValueStateVariant::Json(handle) => run_op!(ruby, this, handle, clear())?,
            ValueStateVariant::Message(handle) => run_op!(ruby, this, handle, clear())?,
        }
        Ok(ruby.qnil().as_value())
    }

    /// Durably commits the buffered operations. Returns `nil`: the erased FFI
    /// seam drops the applied/no-op outcome (owner-ratified divergence from the
    /// `:applied|:noop` naming; surfacing it requires a core change).
    fn commit(ruby: &Ruby, this: &Self) -> Result<Value, Error> {
        match &this.state {
            ValueStateVariant::Json(handle) => run_op!(ruby, this, handle, commit())?,
            ValueStateVariant::Message(handle) => run_op!(ruby, this, handle, commit())?,
        }
        Ok(ruby.qnil().as_value())
    }

    /// Discards the buffered uncommitted operations. Returns `nil` (see
    /// [`commit`](Self::commit)).
    fn rollback(ruby: &Ruby, this: &Self) -> Result<Value, Error> {
        match &this.state {
            ValueStateVariant::Json(handle) => run_infallible!(ruby, this, handle, rollback()),
            ValueStateVariant::Message(handle) => run_infallible!(ruby, this, handle, rollback()),
        }
        Ok(ruby.qnil().as_value())
    }
}

/// Native ordered-map state handle, keyed by `String`, vended per event.
#[magnus::wrap(class = "Prosody::NativeMapState")]
pub struct NativeMapState {
    state: MapStateVariant,
    bridge: Bridge,
    propagator: Arc<TextMapCompositePropagator>,
}

impl NativeMapState {
    /// Wraps a vended map handle with the bridge and propagator.
    pub(crate) fn new(
        state: MapStateVariant,
        bridge: Bridge,
        propagator: Arc<TextMapCompositePropagator>,
    ) -> Self {
        Self {
            state,
            bridge,
            propagator,
        }
    }

    /// Reads the value for `key` (`nil` when absent).
    fn get(ruby: &Ruby, this: &Self, key: String) -> Result<Value, Error> {
        match &this.state {
            MapStateVariant::Json(handle) => {
                json_or_nil(ruby, run_op!(ruby, this, handle, get(key))?)
            }
            MapStateVariant::Message(handle) => {
                message_or_nil(ruby, run_op!(ruby, this, handle, get(key))?)
            }
        }
    }

    /// Answers whether a stored cell exists for `key`, read through the event's
    /// dirty overlay. No value decode and no resolver run — a message-backed
    /// map answers presence with zero Kafka fetches — but not no-I/O: a
    /// cache miss still reads the store.
    fn contains_key(ruby: &Ruby, this: &Self, key: String) -> Result<Value, Error> {
        let present = match &this.state {
            MapStateVariant::Json(handle) => run_op!(ruby, this, handle, contains_key(key))?,
            MapStateVariant::Message(handle) => run_op!(ruby, this, handle, contains_key(key))?,
        };
        Ok(present.into_value_with(ruby))
    }

    /// Reads several keys as one isolated batch, one result per input key.
    fn get_many(ruby: &Ruby, this: &Self, keys: Vec<String>) -> Result<Value, Error> {
        match &this.state {
            MapStateVariant::Json(handle) => {
                let items = run_op!(ruby, this, handle, get_many(keys))?;
                let array =
                    ruby.ary_try_from_iter(items.into_iter().map(|item| json_or_nil(ruby, item)))?;
                Ok(array.as_value())
            }
            MapStateVariant::Message(handle) => {
                let items = run_op!(ruby, this, handle, get_many(keys))?;
                let array = ruby
                    .ary_try_from_iter(items.into_iter().map(|item| message_or_nil(ruby, item)))?;
                Ok(array.as_value())
            }
        }
    }

    /// Inserts or overwrites `key`. Rejects JSON null and item-shape mismatch.
    fn set(ruby: &Ruby, this: &Self, key: String, value: Value) -> Result<Value, Error> {
        match &this.state {
            MapStateVariant::Json(handle) => {
                let item = json_write_item(ruby, value, "; use delete(key) to remove the entry")?;
                run_op!(ruby, this, handle, set(key, item))?;
            }
            MapStateVariant::Message(handle) => {
                let item = message_write_item(
                    ruby,
                    value,
                    "; use delete(key) to remove a message map entry",
                )?;
                run_op!(ruby, this, handle, set(key, item))?;
            }
        }
        Ok(ruby.qnil().as_value())
    }

    /// Removes `key`.
    fn remove(ruby: &Ruby, this: &Self, key: String) -> Result<Value, Error> {
        match &this.state {
            MapStateVariant::Json(handle) => run_op!(ruby, this, handle, remove(key))?,
            MapStateVariant::Message(handle) => run_op!(ruby, this, handle, remove(key))?,
        }
        Ok(ruby.qnil().as_value())
    }

    /// Removes every entry.
    fn clear(ruby: &Ruby, this: &Self) -> Result<Value, Error> {
        match &this.state {
            MapStateVariant::Json(handle) => run_op!(ruby, this, handle, clear())?,
            MapStateVariant::Message(handle) => run_op!(ruby, this, handle, clear())?,
        }
        Ok(ruby.qnil().as_value())
    }

    /// Opens a cursor over the live entries in key order, yielding `(key,
    /// value)` pairs. Synchronous; the carrier is active while core constructs
    /// the stream span.
    #[allow(clippy::needless_pass_by_value, reason = "Magnus method argument type")]
    fn scan(ruby: &Ruby, this: &Self, direction: String) -> Result<StateScan, Error> {
        let dir = parse_direction(ruby, &direction)?;
        let _guard = extract_opentelemetry_context(ruby, &this.propagator)?.attach();
        let inner = match &this.state {
            MapStateVariant::Json(handle) => ScanInner::MapJson {
                cursor: Arc::from(handle.scan(dir)),
                buffer: VecDeque::new(),
                done: false,
            },
            MapStateVariant::Message(handle) => ScanInner::MapMessage {
                cursor: Arc::from(handle.scan(dir)),
                buffer: VecDeque::new(),
                done: false,
            },
        };
        StateScan::new(
            ruby,
            inner,
            this.bridge.clone(),
            Arc::clone(&this.propagator),
        )
    }

    /// Opens a cursor over the live keys in key order, yielding bare `String`
    /// keys. Like [`scan`](Self::scan) but skips value decode and the resolver
    /// — a message-backed map enumerates keys with zero Kafka fetches, though
    /// not no-I/O. Synchronous; the carrier is active while core constructs the
    /// stream span.
    #[allow(clippy::needless_pass_by_value, reason = "Magnus method argument type")]
    fn keys(ruby: &Ruby, this: &Self, direction: String) -> Result<StateScan, Error> {
        let dir = parse_direction(ruby, &direction)?;
        let _guard = extract_opentelemetry_context(ruby, &this.propagator)?.attach();
        let inner = match &this.state {
            MapStateVariant::Json(handle) => ScanInner::MapKeys {
                cursor: Arc::from(handle.keys(dir)),
                buffer: VecDeque::new(),
                done: false,
            },
            MapStateVariant::Message(handle) => ScanInner::MapKeys {
                cursor: Arc::from(handle.keys(dir)),
                buffer: VecDeque::new(),
                done: false,
            },
        };
        StateScan::new(
            ruby,
            inner,
            this.bridge.clone(),
            Arc::clone(&this.propagator),
        )
    }

    /// Durably commits the buffered operations. Returns `nil` (see
    /// [`NativeValueState::commit`]).
    fn commit(ruby: &Ruby, this: &Self) -> Result<Value, Error> {
        match &this.state {
            MapStateVariant::Json(handle) => run_op!(ruby, this, handle, commit())?,
            MapStateVariant::Message(handle) => run_op!(ruby, this, handle, commit())?,
        }
        Ok(ruby.qnil().as_value())
    }

    /// Discards the buffered uncommitted operations. Returns `nil`.
    fn rollback(ruby: &Ruby, this: &Self) -> Result<Value, Error> {
        match &this.state {
            MapStateVariant::Json(handle) => run_infallible!(ruby, this, handle, rollback()),
            MapStateVariant::Message(handle) => run_infallible!(ruby, this, handle, rollback()),
        }
        Ok(ruby.qnil().as_value())
    }
}

/// Native deque state handle, vended per event.
#[magnus::wrap(class = "Prosody::NativeDequeState")]
pub struct NativeDequeState {
    state: DequeStateVariant,
    bridge: Bridge,
    propagator: Arc<TextMapCompositePropagator>,
}

impl NativeDequeState {
    /// Wraps a vended deque handle with the bridge and propagator.
    pub(crate) fn new(
        state: DequeStateVariant,
        bridge: Bridge,
        propagator: Arc<TextMapCompositePropagator>,
    ) -> Self {
        Self {
            state,
            bridge,
            propagator,
        }
    }

    /// The number of live elements. Ruby Integers are unbounded, so the full
    /// `usize` crosses uncapped.
    fn len(ruby: &Ruby, this: &Self) -> Result<Value, Error> {
        let len = match &this.state {
            DequeStateVariant::Json(handle) => run_op!(ruby, this, handle, len())?,
            DequeStateVariant::Message(handle) => run_op!(ruby, this, handle, len())?,
        };
        Ok(len.into_value_with(ruby))
    }

    /// Whether the deque holds no live elements.
    fn is_empty(ruby: &Ruby, this: &Self) -> Result<Value, Error> {
        let empty = match &this.state {
            DequeStateVariant::Json(handle) => run_op!(ruby, this, handle, is_empty())?,
            DequeStateVariant::Message(handle) => run_op!(ruby, this, handle, is_empty())?,
        };
        Ok(empty.into_value_with(ruby))
    }

    /// Reads the element at front-relative `index` (`nil` past the end).
    fn get(ruby: &Ruby, this: &Self, index: usize) -> Result<Value, Error> {
        match &this.state {
            DequeStateVariant::Json(handle) => {
                json_or_nil(ruby, run_op!(ruby, this, handle, get(index))?)
            }
            DequeStateVariant::Message(handle) => {
                message_or_nil(ruby, run_op!(ruby, this, handle, get(index))?)
            }
        }
    }

    /// Reads the front endpoint slot (`get(0)` without the length round trip),
    /// `nil` when empty. Under a TTL an expired endpoint slot yields `nil` even
    /// when live interior elements remain — a peek never searches inward.
    fn peek_front(ruby: &Ruby, this: &Self) -> Result<Value, Error> {
        match &this.state {
            DequeStateVariant::Json(handle) => {
                json_or_nil(ruby, run_op!(ruby, this, handle, peek_front())?)
            }
            DequeStateVariant::Message(handle) => {
                message_or_nil(ruby, run_op!(ruby, this, handle, peek_front())?)
            }
        }
    }

    /// Reads the back endpoint slot (`get(len - 1)` without the length round
    /// trip), `nil` when empty. Same endpoint-slot TTL semantics as
    /// [`peek_front`](Self::peek_front).
    fn peek_back(ruby: &Ruby, this: &Self) -> Result<Value, Error> {
        match &this.state {
            DequeStateVariant::Json(handle) => {
                json_or_nil(ruby, run_op!(ruby, this, handle, peek_back())?)
            }
            DequeStateVariant::Message(handle) => {
                message_or_nil(ruby, run_op!(ruby, this, handle, peek_back())?)
            }
        }
    }

    /// Appends an element at the back. Rejects JSON null and item-shape
    /// mismatch.
    fn push_back(ruby: &Ruby, this: &Self, value: Value) -> Result<Value, Error> {
        match &this.state {
            DequeStateVariant::Json(handle) => {
                let item = json_write_item(ruby, value, " in a deque")?;
                run_op!(ruby, this, handle, push_back(item))?;
            }
            DequeStateVariant::Message(handle) => {
                let item = message_write_item(ruby, value, " to push into a message deque")?;
                run_op!(ruby, this, handle, push_back(item))?;
            }
        }
        Ok(ruby.qnil().as_value())
    }

    /// Prepends an element at the front. Rejects JSON null and item-shape
    /// mismatch.
    fn push_front(ruby: &Ruby, this: &Self, value: Value) -> Result<Value, Error> {
        match &this.state {
            DequeStateVariant::Json(handle) => {
                let item = json_write_item(ruby, value, " in a deque")?;
                run_op!(ruby, this, handle, push_front(item))?;
            }
            DequeStateVariant::Message(handle) => {
                let item = message_write_item(ruby, value, " to push into a message deque")?;
                run_op!(ruby, this, handle, push_front(item))?;
            }
        }
        Ok(ruby.qnil().as_value())
    }

    /// Removes and returns the front element (`nil` when empty).
    fn pop_front(ruby: &Ruby, this: &Self) -> Result<Value, Error> {
        match &this.state {
            DequeStateVariant::Json(handle) => {
                json_or_nil(ruby, run_op!(ruby, this, handle, pop_front())?)
            }
            DequeStateVariant::Message(handle) => {
                message_or_nil(ruby, run_op!(ruby, this, handle, pop_front())?)
            }
        }
    }

    /// Removes and returns the back element (`nil` when empty).
    fn pop_back(ruby: &Ruby, this: &Self) -> Result<Value, Error> {
        match &this.state {
            DequeStateVariant::Json(handle) => {
                json_or_nil(ruby, run_op!(ruby, this, handle, pop_back())?)
            }
            DequeStateVariant::Message(handle) => {
                message_or_nil(ruby, run_op!(ruby, this, handle, pop_back())?)
            }
        }
    }

    /// Removes every element.
    fn clear(ruby: &Ruby, this: &Self) -> Result<Value, Error> {
        match &this.state {
            DequeStateVariant::Json(handle) => run_op!(ruby, this, handle, clear())?,
            DequeStateVariant::Message(handle) => run_op!(ruby, this, handle, clear())?,
        }
        Ok(ruby.qnil().as_value())
    }

    /// Opens a cursor over the live elements in index order. Synchronous; the
    /// carrier is active while core constructs the stream span.
    #[allow(clippy::needless_pass_by_value, reason = "Magnus method argument type")]
    fn scan(ruby: &Ruby, this: &Self, direction: String) -> Result<StateScan, Error> {
        let dir = parse_direction(ruby, &direction)?;
        let _guard = extract_opentelemetry_context(ruby, &this.propagator)?.attach();
        let inner = match &this.state {
            DequeStateVariant::Json(handle) => ScanInner::DequeJson {
                cursor: Arc::from(handle.scan(dir)),
                buffer: VecDeque::new(),
                done: false,
            },
            DequeStateVariant::Message(handle) => ScanInner::DequeMessage {
                cursor: Arc::from(handle.scan(dir)),
                buffer: VecDeque::new(),
                done: false,
            },
        };
        StateScan::new(
            ruby,
            inner,
            this.bridge.clone(),
            Arc::clone(&this.propagator),
        )
    }

    /// Durably commits the buffered operations. Returns `nil` (see
    /// [`NativeValueState::commit`]).
    fn commit(ruby: &Ruby, this: &Self) -> Result<Value, Error> {
        match &this.state {
            DequeStateVariant::Json(handle) => run_op!(ruby, this, handle, commit())?,
            DequeStateVariant::Message(handle) => run_op!(ruby, this, handle, commit())?,
        }
        Ok(ruby.qnil().as_value())
    }

    /// Discards the buffered uncommitted operations. Returns `nil`.
    fn rollback(ruby: &Ruby, this: &Self) -> Result<Value, Error> {
        match &this.state {
            DequeStateVariant::Json(handle) => run_infallible!(ruby, this, handle, rollback()),
            DequeStateVariant::Message(handle) => run_infallible!(ruby, this, handle, rollback()),
        }
        Ok(ruby.qnil().as_value())
    }
}

/// The four cursor flavours a scan yields, one per (collection, payload) pair.
///
/// Each retains a `buffer` of the items pulled in the current ready-chunk plus
/// a `done` flag; the erased [`StateCursor`] behind the [`Arc`] owns
/// exhaustion, error ordering, and close-idempotence.
enum ScanInner {
    /// A deque JSON scan yielding values.
    DequeJson {
        /// The erased cursor.
        cursor: Arc<StateCursor<JsonValue>>,
        /// Items pulled but not yet yielded.
        buffer: VecDeque<JsonValue>,
        /// Whether the cursor is exhausted.
        done: bool,
    },
    /// A map JSON scan yielding `(key, value)` entries.
    MapJson {
        /// The erased cursor.
        cursor: Arc<StateCursor<(String, JsonValue)>>,
        /// Items pulled but not yet yielded.
        buffer: VecDeque<(String, JsonValue)>,
        /// Whether the cursor is exhausted.
        done: bool,
    },
    /// A deque message scan yielding messages.
    DequeMessage {
        /// The erased cursor.
        cursor: Arc<StateCursor<ConsumerMessage<JsonValue>>>,
        /// Items pulled but not yet yielded.
        buffer: VecDeque<ConsumerMessage<JsonValue>>,
        /// Whether the cursor is exhausted.
        done: bool,
    },
    /// A map message scan yielding `(key, message)` entries.
    MapMessage {
        /// The erased cursor.
        cursor: Arc<StateCursor<(String, ConsumerMessage<JsonValue>)>>,
        /// Items pulled but not yet yielded.
        buffer: VecDeque<(String, ConsumerMessage<JsonValue>)>,
        /// Whether the cursor is exhausted.
        done: bool,
    },
    /// A map key-only scan yielding bare keys (payload-agnostic).
    MapKeys {
        /// The erased cursor.
        cursor: Arc<StateCursor<String>>,
        /// Keys pulled but not yet yielded.
        buffer: VecDeque<String>,
        /// Whether the cursor is exhausted.
        done: bool,
    },
}

/// Yields the next buffered item, or pulls a fresh ready-chunk through
/// [`Bridge::wait_for`], for one [`ScanInner`] arm. On exhaustion it closes the
/// cursor (idempotent) and returns `nil`.
macro_rules! drive_scan {
    ($ruby:expr, $this:expr, $cursor:expr, $buffer:expr, $done:expr, |$item:ident| $conv:block) => {{
        loop {
            if let Some($item) = $buffer.pop_front() {
                return $conv;
            }
            if *$done {
                return Ok($ruby.qnil().as_value());
            }
            let cursor = Arc::clone($cursor);
            let context = extract_opentelemetry_context($ruby, &$this.propagator)?;
            let chunk = $this
                .bridge
                .wait_for(
                    $ruby,
                    async move {
                        cursor
                            .next_ready_chunk(SCAN_READY_CHUNK_SIZE)
                            .with_context(context)
                            .await
                    },
                    Span::current(),
                )?
                .map_err(|error| state_error($ruby, &error))?;
            match chunk {
                Some(items) => $buffer.extend(items),
                None => {
                    *$done = true;
                    let cursor = Arc::clone($cursor);
                    $this.bridge.wait_for(
                        $ruby,
                        async move { cursor.close().await },
                        Span::current(),
                    )?;
                    return Ok($ruby.qnil().as_value());
                }
            }
        }
    }};
}

/// Closes the erased cursor for one [`ScanInner`] arm (idempotent).
///
/// Marks the arm terminal and drops any buffered-but-unyielded items *before*
/// yielding, so a `#next` after `#close` returns `nil` immediately rather than
/// draining stale items.
macro_rules! close_scan {
    ($ruby:expr, $this:expr, $cursor:expr, $buffer:expr, $done:expr) => {{
        *$done = true;
        $buffer.clear();
        let cursor = Arc::clone($cursor);
        $this
            .bridge
            .wait_for($ruby, async move { cursor.close().await }, Span::current())?;
    }};
}

/// Item-oriented scan cursor over a map or deque collection.
///
/// `StateScan#next` yields individual items, pulling a fresh ready-chunk from
/// core when the buffer drains and returning `nil` at exhaustion (unambiguous
/// under the null ban). A single fiber-aware permit (a one-token
/// `Thread::Queue` held through [`ThreadSafeValue`]) serializes `#next` and
/// `#close` in invocation order across chunks: concurrent fibers block on the
/// permit rather than racing the buffer, an exception does not poison cleanup
/// (the permit is released on every path), and `#close` cannot run under an
/// active `#next`. `#close` is idempotent (core-owned) and wired into every
/// traversal path via the Ruby `ensure`.
#[magnus::wrap(class = "Prosody::StateScan")]
pub struct StateScan {
    inner: RefCell<ScanInner>,
    lock: ThreadSafeValue,
    bridge: Bridge,
    propagator: Arc<TextMapCompositePropagator>,
}

impl StateScan {
    /// Builds a scan around an erased cursor, seeding the one-token permit.
    fn new(
        ruby: &Ruby,
        inner: ScanInner,
        bridge: Bridge,
        propagator: Arc<TextMapCompositePropagator>,
    ) -> Result<Self, Error> {
        let queue: Value = ruby.get_inner(&QUEUE_CLASS).funcall(id!(ruby, "new"), ())?;
        let _: Value = queue.funcall(id!(ruby, "push"), (ruby.qnil(),))?;
        Ok(Self {
            inner: RefCell::new(inner),
            lock: ThreadSafeValue::new(queue, bridge.clone()),
            bridge,
            propagator,
        })
    }

    /// Acquires the permit, fiber-yielding until it is free.
    fn acquire(&self, ruby: &Ruby) -> Result<(), Error> {
        let _: Value = self.lock.get(ruby).funcall(id!(ruby, "pop"), ())?;
        Ok(())
    }

    /// Releases the permit (best-effort; runs on both success and failure).
    fn release(&self, ruby: &Ruby) {
        let _: Result<Value, Error> = self
            .lock
            .get(ruby)
            .funcall(id!(ruby, "push"), (ruby.qnil(),));
    }

    /// Yields the next scanned item, or `nil` at exhaustion.
    fn next(ruby: &Ruby, this: &Self) -> Result<Value, Error> {
        this.acquire(ruby)?;
        let out = Self::next_locked(ruby, this);
        this.release(ruby);
        out
    }

    /// The permit-protected body of [`next`](Self::next).
    fn next_locked(ruby: &Ruby, this: &Self) -> Result<Value, Error> {
        match &mut *this.inner.borrow_mut() {
            ScanInner::DequeJson {
                cursor,
                buffer,
                done,
            } => drive_scan!(ruby, this, cursor, buffer, done, |item| {
                serialize(ruby, &item)
            }),
            ScanInner::MapJson {
                cursor,
                buffer,
                done,
            } => drive_scan!(ruby, this, cursor, buffer, done, |item| {
                let (key, value) = item;
                let value: Value = serialize(ruby, &value)?;
                Ok((key, value).into_value_with(ruby))
            }),
            ScanInner::DequeMessage {
                cursor,
                buffer,
                done,
            } => drive_scan!(ruby, this, cursor, buffer, done, |item| {
                Ok(Message::from(item).into_value_with(ruby))
            }),
            ScanInner::MapMessage {
                cursor,
                buffer,
                done,
            } => drive_scan!(ruby, this, cursor, buffer, done, |item| {
                let (key, message) = item;
                Ok((key, Message::from(message)).into_value_with(ruby))
            }),
            ScanInner::MapKeys {
                cursor,
                buffer,
                done,
            } => drive_scan!(ruby, this, cursor, buffer, done, |item| {
                Ok(item.into_value_with(ruby))
            }),
        }
    }

    /// Closes the cursor, releasing the underlying stream. Idempotent, and
    /// cannot run under an active `#next` (both take the permit).
    fn close(ruby: &Ruby, this: &Self) -> Result<Value, Error> {
        this.acquire(ruby)?;
        let out = Self::close_locked(ruby, this);
        this.release(ruby);
        out
    }

    /// The permit-protected body of [`close`](Self::close).
    fn close_locked(ruby: &Ruby, this: &Self) -> Result<Value, Error> {
        match &mut *this.inner.borrow_mut() {
            ScanInner::DequeJson {
                cursor,
                buffer,
                done,
            } => close_scan!(ruby, this, cursor, buffer, done),
            ScanInner::MapJson {
                cursor,
                buffer,
                done,
            } => close_scan!(ruby, this, cursor, buffer, done),
            ScanInner::DequeMessage {
                cursor,
                buffer,
                done,
            } => close_scan!(ruby, this, cursor, buffer, done),
            ScanInner::MapMessage {
                cursor,
                buffer,
                done,
            } => close_scan!(ruby, this, cursor, buffer, done),
            ScanInner::MapKeys {
                cursor,
                buffer,
                done,
            } => close_scan!(ruby, this, cursor, buffer, done),
        }
        Ok(ruby.qnil().as_value())
    }
}

/// Registers the native keyed-state classes and their methods.
///
/// # Errors
///
/// Returns a Magnus error if class or method definition fails.
pub fn init(ruby: &Ruby) -> Result<(), Error> {
    let module = ruby.get_inner(&ROOT_MOD);

    let value = module.define_class(id!(ruby, "NativeValueState"), ruby.class_object())?;
    value.define_method(id!(ruby, "get"), method!(NativeValueState::get, 0))?;
    value.define_method(id!(ruby, "set"), method!(NativeValueState::set, 1))?;
    value.define_method(id!(ruby, "clear"), method!(NativeValueState::clear, 0))?;
    value.define_method(id!(ruby, "commit"), method!(NativeValueState::commit, 0))?;
    value.define_method(
        id!(ruby, "rollback"),
        method!(NativeValueState::rollback, 0),
    )?;

    let map = module.define_class(id!(ruby, "NativeMapState"), ruby.class_object())?;
    map.define_method(id!(ruby, "get"), method!(NativeMapState::get, 1))?;
    map.define_method(
        id!(ruby, "contains_key"),
        method!(NativeMapState::contains_key, 1),
    )?;
    map.define_method(id!(ruby, "get_many"), method!(NativeMapState::get_many, 1))?;
    map.define_method(id!(ruby, "set"), method!(NativeMapState::set, 2))?;
    map.define_method(id!(ruby, "remove"), method!(NativeMapState::remove, 1))?;
    map.define_method(id!(ruby, "clear"), method!(NativeMapState::clear, 0))?;
    map.define_method(id!(ruby, "scan"), method!(NativeMapState::scan, 1))?;
    map.define_method(id!(ruby, "keys"), method!(NativeMapState::keys, 1))?;
    map.define_method(id!(ruby, "commit"), method!(NativeMapState::commit, 0))?;
    map.define_method(id!(ruby, "rollback"), method!(NativeMapState::rollback, 0))?;

    let deque = module.define_class(id!(ruby, "NativeDequeState"), ruby.class_object())?;
    deque.define_method(id!(ruby, "len"), method!(NativeDequeState::len, 0))?;
    deque.define_method(
        id!(ruby, "is_empty"),
        method!(NativeDequeState::is_empty, 0),
    )?;
    deque.define_method(id!(ruby, "get"), method!(NativeDequeState::get, 1))?;
    deque.define_method(
        id!(ruby, "peek_front"),
        method!(NativeDequeState::peek_front, 0),
    )?;
    deque.define_method(
        id!(ruby, "peek_back"),
        method!(NativeDequeState::peek_back, 0),
    )?;
    deque.define_method(
        id!(ruby, "push_back"),
        method!(NativeDequeState::push_back, 1),
    )?;
    deque.define_method(
        id!(ruby, "push_front"),
        method!(NativeDequeState::push_front, 1),
    )?;
    deque.define_method(
        id!(ruby, "pop_front"),
        method!(NativeDequeState::pop_front, 0),
    )?;
    deque.define_method(
        id!(ruby, "pop_back"),
        method!(NativeDequeState::pop_back, 0),
    )?;
    deque.define_method(id!(ruby, "clear"), method!(NativeDequeState::clear, 0))?;
    deque.define_method(id!(ruby, "scan"), method!(NativeDequeState::scan, 1))?;
    deque.define_method(id!(ruby, "commit"), method!(NativeDequeState::commit, 0))?;
    deque.define_method(
        id!(ruby, "rollback"),
        method!(NativeDequeState::rollback, 0),
    )?;

    let scan = module.define_class(id!(ruby, "StateScan"), ruby.class_object())?;
    scan.define_method(id!(ruby, "next"), method!(StateScan::next, 0))?;
    scan.define_method(id!(ruby, "close"), method!(StateScan::close, 0))?;

    Ok(())
}
