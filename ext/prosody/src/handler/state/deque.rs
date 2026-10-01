//! Native deque handles for JSON and message payloads.

use super::{
    NativeJsonDequeScan, NativeMessageDequeScan, json_write_item, message_or_nil,
    message_write_item, outcome_symbol, position_query, run_state, wrapped_class,
};
use crate::bridge::Bridge;
use magnus::{Error, Module, RHash, RModule, Ruby, StaticSymbol, Value, method};
use opentelemetry::propagation::TextMapCompositePropagator;
use prosody::consumer::event_context::DynDequeState;
use prosody::consumer::message::ConsumerMessage;
use serde_json::Value as JsonValue;
use serde_magnus::serialize;
use std::sync::Arc;

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
