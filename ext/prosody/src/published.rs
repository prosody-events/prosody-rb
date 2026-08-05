//! Read-only published-state handles for Ruby.

use crate::bridge::Bridge;
use crate::handler::{
    NativeJsonDequeScan, NativeJsonMapScan, NativeMapKeyScan, parse_direction,
    published_deque_scan, published_map_key_scan, published_map_scan,
};
use crate::{ROOT_MOD, id};
use magnus::value::ReprValue;
use magnus::{Error, Module, Ruby, StaticSymbol, Value, method};
use opentelemetry::propagation::TextMapCompositePropagator;
use prosody::JsonCodec;
use prosody::high_level::erased::{
    ErasedDirection, SharedDequeReader, SharedMapReader, SharedValueReader,
};
use prosody::state::Direction;
use serde_magnus::serialize;
use std::sync::Arc;
use tracing::Span;

fn read_error(ruby: &Ruby, error: &impl ToString) -> Error {
    Error::new(ruby.exception_runtime_error(), error.to_string())
}

fn erased_direction(direction: Direction) -> ErasedDirection {
    match direction {
        Direction::Forward => ErasedDirection::Forward,
        Direction::Backward => ErasedDirection::Backward,
    }
}

#[magnus::wrap(class = "Prosody::NativePublishedValue")]
pub(crate) struct NativePublishedValue {
    pub(crate) inner: SharedValueReader<JsonCodec>,
    pub(crate) bridge: Bridge,
}

impl NativePublishedValue {
    fn get(ruby: &Ruby, this: &Self, key: String) -> Result<Value, Error> {
        let inner = Arc::clone(&this.inner);
        let value = this
            .bridge
            .wait_for(ruby, async move { inner.get(key).await }, Span::current())?
            .map_err(|error| read_error(ruby, &error))?;
        match value {
            Some(value) => serialize(ruby, &value),
            None => Ok(ruby.qnil().as_value()),
        }
    }
}

#[magnus::wrap(class = "Prosody::NativePublishedMap")]
pub(crate) struct NativePublishedMap {
    pub(crate) inner: SharedMapReader<JsonCodec>,
    pub(crate) bridge: Bridge,
    pub(crate) propagator: Arc<TextMapCompositePropagator>,
}

impl NativePublishedMap {
    fn get(ruby: &Ruby, this: &Self, key: String, map_key: String) -> Result<Value, Error> {
        let inner = Arc::clone(&this.inner);
        let value = this
            .bridge
            .wait_for(
                ruby,
                async move { inner.get(key, map_key).await },
                Span::current(),
            )?
            .map_err(|error| read_error(ruby, &error))?;
        match value {
            Some(value) => serialize(ruby, &value),
            None => Ok(ruby.qnil().as_value()),
        }
    }

    fn get_many(
        ruby: &Ruby,
        this: &Self,
        key: String,
        map_keys: Vec<String>,
    ) -> Result<Value, Error> {
        let inner = Arc::clone(&this.inner);
        let values = this
            .bridge
            .wait_for(
                ruby,
                async move { inner.get_many(key, map_keys).await },
                Span::current(),
            )?
            .map_err(|error| read_error(ruby, &error))?;
        serialize(ruby, &values)
    }

    fn contains_key(ruby: &Ruby, this: &Self, key: String, map_key: String) -> Result<bool, Error> {
        let inner = Arc::clone(&this.inner);
        this.bridge
            .wait_for(
                ruby,
                async move { inner.contains_key(key, map_key).await },
                Span::current(),
            )?
            .map_err(|error| read_error(ruby, &error))
    }

    fn scan(
        ruby: &Ruby,
        this: &Self,
        key: String,
        direction: StaticSymbol,
    ) -> Result<NativeJsonMapScan, Error> {
        let direction = erased_direction(parse_direction(ruby, direction)?);
        let inner = Arc::clone(&this.inner);
        let cursor = this
            .bridge
            .wait_for(
                ruby,
                async move { inner.stream(key, direction).await },
                Span::current(),
            )?
            .map_err(|error| read_error(ruby, &error))?;
        published_map_scan(
            ruby,
            cursor,
            this.bridge.clone(),
            Arc::clone(&this.propagator),
        )
    }

    fn keys(
        ruby: &Ruby,
        this: &Self,
        key: String,
        direction: StaticSymbol,
    ) -> Result<NativeMapKeyScan, Error> {
        let direction = erased_direction(parse_direction(ruby, direction)?);
        let inner = Arc::clone(&this.inner);
        let cursor = this
            .bridge
            .wait_for(
                ruby,
                async move { inner.keys(key, direction).await },
                Span::current(),
            )?
            .map_err(|error| read_error(ruby, &error))?;
        published_map_key_scan(
            ruby,
            cursor,
            this.bridge.clone(),
            Arc::clone(&this.propagator),
        )
    }
}

#[magnus::wrap(class = "Prosody::NativePublishedDeque")]
pub(crate) struct NativePublishedDeque {
    pub(crate) inner: SharedDequeReader<JsonCodec>,
    pub(crate) bridge: Bridge,
    pub(crate) propagator: Arc<TextMapCompositePropagator>,
}

impl NativePublishedDeque {
    fn get(ruby: &Ruby, this: &Self, key: String, index: usize) -> Result<Value, Error> {
        let inner = Arc::clone(&this.inner);
        let value = this
            .bridge
            .wait_for(
                ruby,
                async move { inner.get(key, index).await },
                Span::current(),
            )?
            .map_err(|error| read_error(ruby, &error))?;
        match value {
            Some(value) => serialize(ruby, &value),
            None => Ok(ruby.qnil().as_value()),
        }
    }

    fn length(ruby: &Ruby, this: &Self, key: String) -> Result<usize, Error> {
        let inner = Arc::clone(&this.inner);
        this.bridge
            .wait_for(ruby, async move { inner.len(key).await }, Span::current())?
            .map_err(|error| read_error(ruby, &error))
    }

    fn is_empty(ruby: &Ruby, this: &Self, key: String) -> Result<bool, Error> {
        let inner = Arc::clone(&this.inner);
        this.bridge
            .wait_for(
                ruby,
                async move { inner.is_empty(key).await },
                Span::current(),
            )?
            .map_err(|error| read_error(ruby, &error))
    }

    fn peek_front(ruby: &Ruby, this: &Self, key: String) -> Result<Value, Error> {
        let inner = Arc::clone(&this.inner);
        let value = this
            .bridge
            .wait_for(
                ruby,
                async move { inner.peek_front(key).await },
                Span::current(),
            )?
            .map_err(|error| read_error(ruby, &error))?;
        match value {
            Some(value) => serialize(ruby, &value),
            None => Ok(ruby.qnil().as_value()),
        }
    }

    fn peek_back(ruby: &Ruby, this: &Self, key: String) -> Result<Value, Error> {
        let inner = Arc::clone(&this.inner);
        let value = this
            .bridge
            .wait_for(
                ruby,
                async move { inner.peek_back(key).await },
                Span::current(),
            )?
            .map_err(|error| read_error(ruby, &error))?;
        match value {
            Some(value) => serialize(ruby, &value),
            None => Ok(ruby.qnil().as_value()),
        }
    }

    fn scan(
        ruby: &Ruby,
        this: &Self,
        key: String,
        direction: StaticSymbol,
    ) -> Result<NativeJsonDequeScan, Error> {
        let direction = erased_direction(parse_direction(ruby, direction)?);
        let inner = Arc::clone(&this.inner);
        let cursor = this
            .bridge
            .wait_for(
                ruby,
                async move { inner.stream(key, direction).await },
                Span::current(),
            )?
            .map_err(|error| read_error(ruby, &error))?;
        published_deque_scan(
            ruby,
            cursor,
            this.bridge.clone(),
            Arc::clone(&this.propagator),
        )
    }
}

pub(crate) fn init(ruby: &Ruby) -> Result<(), Error> {
    let module = ruby.get_inner(&ROOT_MOD);
    let value = module.define_class(id!(ruby, "NativePublishedValue"), ruby.class_object())?;
    value.define_method("get", method!(NativePublishedValue::get, 1))?;

    let map = module.define_class(id!(ruby, "NativePublishedMap"), ruby.class_object())?;
    map.define_method("get", method!(NativePublishedMap::get, 2))?;
    map.define_method("get_many", method!(NativePublishedMap::get_many, 2))?;
    map.define_method("contains_key", method!(NativePublishedMap::contains_key, 2))?;
    map.define_method("scan", method!(NativePublishedMap::scan, 2))?;
    map.define_method("keys", method!(NativePublishedMap::keys, 2))?;

    let deque = module.define_class(id!(ruby, "NativePublishedDeque"), ruby.class_object())?;
    deque.define_method("get", method!(NativePublishedDeque::get, 2))?;
    deque.define_method("length", method!(NativePublishedDeque::length, 1))?;
    deque.define_method("is_empty", method!(NativePublishedDeque::is_empty, 1))?;
    deque.define_method("peek_front", method!(NativePublishedDeque::peek_front, 1))?;
    deque.define_method("peek_back", method!(NativePublishedDeque::peek_back, 1))?;
    deque.define_method("scan", method!(NativePublishedDeque::scan, 2))?;
    Ok(())
}
