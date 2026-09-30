//! Read-only published-state handles for Ruby.
//!
//! Every reader method runs through [`Reads`], which refuses a forked child,
//! joins the caller's OpenTelemetry context, and raises a failed read as
//! `RuntimeError`.

use crate::bridge::Bridge;
use crate::handler::{
    NativeJsonDequeScan, NativeJsonMapScan, NativeMapKeyScan, key_query, position_query,
    published_deque_scan, published_map_key_scan, published_map_scan, published_scan_arguments,
};
use crate::tracing_util::extract_opentelemetry_context;
use crate::util::ForkGuard;
use crate::{ROOT_MOD, id};
use magnus::{Error, Module, Ruby, Value, method};
use opentelemetry::propagation::TextMapCompositePropagator;
use opentelemetry::trace::FutureExt;
use prosody::high_level::erased::{
    SharedDequeReader, SharedMapReader, SharedSetReader, SharedValueReader,
};
use serde_json::Value as JsonValue;
use serde_magnus::serialize;
use std::fmt::Display;
use std::sync::Arc;
use tracing::Span;

/// The bridge, trace propagator, and fork guard that every reader shares.
#[derive(Clone)]
pub(crate) struct Reads {
    bridge: Bridge,
    propagator: Arc<TextMapCompositePropagator>,
    fork: ForkGuard,
}

impl Reads {
    pub(crate) fn new(
        bridge: Bridge,
        propagator: Arc<TextMapCompositePropagator>,
        fork: ForkGuard,
    ) -> Self {
        Self {
            bridge,
            propagator,
            fork,
        }
    }

    /// Waits for one read in the caller's trace.
    ///
    /// # Errors
    ///
    /// Raises `RuntimeError` after fork or when the read fails.
    fn read<F, T, E>(&self, ruby: &Ruby, read: F) -> Result<T, Error>
    where
        F: Future<Output = Result<T, E>> + Send + 'static,
        T: Send + 'static,
        E: Display + Send + 'static,
    {
        self.fork.check(ruby)?;
        let context = extract_opentelemetry_context(ruby, &self.propagator)?;
        self.bridge
            .wait_for(ruby, read.with_context(context), Span::current())?
            .map_err(|error| Error::new(ruby.exception_runtime_error(), error.to_string()))
    }

    /// Waits for one optional JSON read and returns the value or `nil`.
    ///
    /// # Errors
    ///
    /// See [`Reads::read`].
    fn read_json<F, E>(&self, ruby: &Ruby, read: F) -> Result<Value, Error>
    where
        F: Future<Output = Result<Option<JsonValue>, E>> + Send + 'static,
        E: Display + Send + 'static,
    {
        serialize(ruby, &self.read(ruby, read)?)
    }

    /// Returns the bridge and propagator for a new scan.
    ///
    /// # Errors
    ///
    /// Raises `RuntimeError` after fork.
    fn scan_parts(&self, ruby: &Ruby) -> Result<(Bridge, Arc<TextMapCompositePropagator>), Error> {
        self.fork.check(ruby)?;
        Ok((self.bridge.clone(), Arc::clone(&self.propagator)))
    }
}

#[magnus::wrap(class = "Prosody::NativePublishedValue")]
pub(crate) struct NativePublishedValue {
    pub(crate) inner: SharedValueReader<JsonValue>,
    pub(crate) reads: Reads,
}

impl NativePublishedValue {
    fn get(ruby: &Ruby, this: &Self, key: String) -> Result<Value, Error> {
        let inner = Arc::clone(&this.inner);
        this.reads
            .read_json(ruby, async move { inner.get(key).await })
    }
}

#[magnus::wrap(class = "Prosody::NativePublishedMap")]
pub(crate) struct NativePublishedMap {
    pub(crate) inner: SharedMapReader<JsonValue>,
    pub(crate) reads: Reads,
}

impl NativePublishedMap {
    fn get(ruby: &Ruby, this: &Self, key: String, map_key: String) -> Result<Value, Error> {
        let inner = Arc::clone(&this.inner);
        this.reads
            .read_json(ruby, async move { inner.get(key, map_key).await })
    }

    fn get_many(
        ruby: &Ruby,
        this: &Self,
        key: String,
        map_keys: Vec<String>,
    ) -> Result<Value, Error> {
        let inner = Arc::clone(&this.inner);
        let values = this
            .reads
            .read(ruby, async move { inner.get_many(key, map_keys).await })?;
        serialize(ruby, &values)
    }

    fn contains_many(
        ruby: &Ruby,
        this: &Self,
        key: String,
        map_keys: Vec<String>,
    ) -> Result<Vec<bool>, Error> {
        let inner = Arc::clone(&this.inner);
        this.reads.read(
            ruby,
            async move { inner.contains_many(key, map_keys).await },
        )
    }

    fn is_empty(ruby: &Ruby, this: &Self, key: String) -> Result<bool, Error> {
        let inner = Arc::clone(&this.inner);
        this.reads
            .read(ruby, async move { inner.is_empty(key).await })
    }

    fn contains_key(ruby: &Ruby, this: &Self, key: String, map_key: String) -> Result<bool, Error> {
        let inner = Arc::clone(&this.inner);
        this.reads
            .read(ruby, async move { inner.contains_key(key, map_key).await })
    }

    fn scan(ruby: &Ruby, this: &Self, args: &[Value]) -> Result<NativeJsonMapScan, Error> {
        let (key, direction, options) = published_scan_arguments(args)?;
        let query = key_query(ruby, direction, options)?;
        let (bridge, propagator) = this.reads.scan_parts(ruby)?;
        let entries = this.inner.entries(key).with_query(query).stream();
        published_map_scan(ruby, entries, bridge, propagator)
    }

    fn keys(ruby: &Ruby, this: &Self, args: &[Value]) -> Result<NativeMapKeyScan, Error> {
        let (key, direction, options) = published_scan_arguments(args)?;
        let query = key_query(ruby, direction, options)?;
        let (bridge, propagator) = this.reads.scan_parts(ruby)?;
        let keys = this.inner.keys(key).with_query(query).stream();
        published_map_key_scan(ruby, keys, bridge, propagator)
    }
}

#[magnus::wrap(class = "Prosody::NativePublishedSet")]
pub(crate) struct NativePublishedSet {
    pub(crate) inner: SharedSetReader,
    pub(crate) reads: Reads,
}

impl NativePublishedSet {
    fn contains(ruby: &Ruby, this: &Self, key: String, member: String) -> Result<bool, Error> {
        let inner = Arc::clone(&this.inner);
        this.reads
            .read(ruby, async move { inner.contains(key, member).await })
    }

    fn contains_many(
        ruby: &Ruby,
        this: &Self,
        key: String,
        members: Vec<String>,
    ) -> Result<Vec<bool>, Error> {
        let inner = Arc::clone(&this.inner);
        this.reads
            .read(ruby, async move { inner.contains_many(key, members).await })
    }

    fn is_empty(ruby: &Ruby, this: &Self, key: String) -> Result<bool, Error> {
        let inner = Arc::clone(&this.inner);
        this.reads
            .read(ruby, async move { inner.is_empty(key).await })
    }

    /// Opens a member cursor. Members are bare `String` keys, so the map key
    /// cursor carries them.
    fn keys(ruby: &Ruby, this: &Self, args: &[Value]) -> Result<NativeMapKeyScan, Error> {
        let (key, direction, options) = published_scan_arguments(args)?;
        let query = key_query(ruby, direction, options)?;
        let (bridge, propagator) = this.reads.scan_parts(ruby)?;
        let members = this.inner.keys(key).with_query(query).stream();
        published_map_key_scan(ruby, members, bridge, propagator)
    }
}

#[magnus::wrap(class = "Prosody::NativePublishedDeque")]
pub(crate) struct NativePublishedDeque {
    pub(crate) inner: SharedDequeReader<JsonValue>,
    pub(crate) reads: Reads,
}

impl NativePublishedDeque {
    fn get(ruby: &Ruby, this: &Self, key: String, index: usize) -> Result<Value, Error> {
        let inner = Arc::clone(&this.inner);
        this.reads
            .read_json(ruby, async move { inner.get(key, index).await })
    }

    fn length(ruby: &Ruby, this: &Self, key: String) -> Result<usize, Error> {
        let inner = Arc::clone(&this.inner);
        this.reads.read(ruby, async move { inner.len(key).await })
    }

    fn is_empty(ruby: &Ruby, this: &Self, key: String) -> Result<bool, Error> {
        let inner = Arc::clone(&this.inner);
        this.reads
            .read(ruby, async move { inner.is_empty(key).await })
    }

    fn peek_front(ruby: &Ruby, this: &Self, key: String) -> Result<Value, Error> {
        let inner = Arc::clone(&this.inner);
        this.reads
            .read_json(ruby, async move { inner.peek_front(key).await })
    }

    fn peek_back(ruby: &Ruby, this: &Self, key: String) -> Result<Value, Error> {
        let inner = Arc::clone(&this.inner);
        this.reads
            .read_json(ruby, async move { inner.peek_back(key).await })
    }

    fn scan(ruby: &Ruby, this: &Self, args: &[Value]) -> Result<NativeJsonDequeScan, Error> {
        let (key, direction, options) = published_scan_arguments(args)?;
        let query = position_query(ruby, direction, options)?;
        let (bridge, propagator) = this.reads.scan_parts(ruby)?;
        let values = this.inner.values(key).with_query(query).stream();
        published_deque_scan(ruby, values, bridge, propagator)
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
    map.define_method(
        "contains_many",
        method!(NativePublishedMap::contains_many, 2),
    )?;
    map.define_method("is_empty", method!(NativePublishedMap::is_empty, 1))?;
    map.define_method("scan", method!(NativePublishedMap::scan, -1))?;
    map.define_method("keys", method!(NativePublishedMap::keys, -1))?;

    let set = module.define_class(id!(ruby, "NativePublishedSet"), ruby.class_object())?;
    set.define_method("contains", method!(NativePublishedSet::contains, 2))?;
    set.define_method(
        "contains_many",
        method!(NativePublishedSet::contains_many, 2),
    )?;
    set.define_method("is_empty", method!(NativePublishedSet::is_empty, 1))?;
    set.define_method("keys", method!(NativePublishedSet::keys, -1))?;

    let deque = module.define_class(id!(ruby, "NativePublishedDeque"), ruby.class_object())?;
    deque.define_method("get", method!(NativePublishedDeque::get, 2))?;
    deque.define_method("length", method!(NativePublishedDeque::length, 1))?;
    deque.define_method("is_empty", method!(NativePublishedDeque::is_empty, 1))?;
    deque.define_method("peek_front", method!(NativePublishedDeque::peek_front, 1))?;
    deque.define_method("peek_back", method!(NativePublishedDeque::peek_back, 1))?;
    deque.define_method("scan", method!(NativePublishedDeque::scan, -1))?;
    Ok(())
}
