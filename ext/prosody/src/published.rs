//! Read-only published-state handles for Ruby.

use crate::bridge::Bridge;
use crate::handler::{
    NativeJsonDequeScan, NativeJsonMapScan, NativeMapKeyScan, key_query, position_query,
    published_deque_scan, published_map_key_scan, published_map_scan, published_scan_arguments,
};
use crate::{ROOT_MOD, id};
use magnus::value::ReprValue;
use magnus::{Error, Module, Ruby, Value, method};
use opentelemetry::propagation::TextMapCompositePropagator;
use prosody::high_level::erased::{SharedDequeReader, SharedMapReader, SharedValueReader};
use serde_json::Value as JsonValue;
use serde_magnus::serialize;
use std::sync::Arc;
use tracing::Span;

fn read_error(ruby: &Ruby, error: &impl ToString) -> Error {
    Error::new(ruby.exception_runtime_error(), error.to_string())
}

#[magnus::wrap(class = "Prosody::NativePublishedValue")]
pub(crate) struct NativePublishedValue {
    pub(crate) inner: SharedValueReader<JsonValue>,
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
    pub(crate) inner: SharedMapReader<JsonValue>,
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

    fn scan(ruby: &Ruby, this: &Self, args: &[Value]) -> Result<NativeJsonMapScan, Error> {
        let (key, direction, options) = published_scan_arguments(args)?;
        let query = key_query(ruby, direction, options)?;
        published_map_scan(
            ruby,
            this.inner.entries(key).with_query(query).stream(),
            this.bridge.clone(),
            Arc::clone(&this.propagator),
        )
    }

    fn keys(ruby: &Ruby, this: &Self, args: &[Value]) -> Result<NativeMapKeyScan, Error> {
        let (key, direction, options) = published_scan_arguments(args)?;
        let query = key_query(ruby, direction, options)?;
        published_map_key_scan(
            ruby,
            this.inner.keys(key).with_query(query).stream(),
            this.bridge.clone(),
            Arc::clone(&this.propagator),
        )
    }
}

#[magnus::wrap(class = "Prosody::NativePublishedDeque")]
pub(crate) struct NativePublishedDeque {
    pub(crate) inner: SharedDequeReader<JsonValue>,
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

    fn scan(ruby: &Ruby, this: &Self, args: &[Value]) -> Result<NativeJsonDequeScan, Error> {
        let (key, direction, options) = published_scan_arguments(args)?;
        let query = position_query(ruby, direction, options)?;
        published_deque_scan(
            ruby,
            this.inner.values(key).with_query(query).stream(),
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
    map.define_method("scan", method!(NativePublishedMap::scan, -1))?;
    map.define_method("keys", method!(NativePublishedMap::keys, -1))?;

    let deque = module.define_class(id!(ruby, "NativePublishedDeque"), ruby.class_object())?;
    deque.define_method("get", method!(NativePublishedDeque::get, 2))?;
    deque.define_method("length", method!(NativePublishedDeque::length, 1))?;
    deque.define_method("is_empty", method!(NativePublishedDeque::is_empty, 1))?;
    deque.define_method("peek_front", method!(NativePublishedDeque::peek_front, 1))?;
    deque.define_method("peek_back", method!(NativePublishedDeque::peek_back, 1))?;
    deque.define_method("scan", method!(NativePublishedDeque::scan, -1))?;
    Ok(())
}
