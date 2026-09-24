//! Published-state reader constructors for `Prosody::Client`.
//!
//! Each constructor opens a core reader through the bridge and wraps it in the
//! matching native reader class.

use super::{Client, RubyHandler, read_cache};
use crate::published::{
    NativePublishedDeque, NativePublishedMap, NativePublishedSet, NativePublishedValue,
};
use magnus::{Error, Ruby};
use prosody::high_level::erased::{ErasedReadCache, SharedHighLevelClient};
use std::fmt::Display;
use std::sync::Arc;
use tracing::Span;

/// Opens one published-state reader. `open` receives the shared client and
/// the resolved cache policy.
fn open_reader<F, Fut, R, E>(
    ruby: &Ruby,
    this: &Client,
    cache_seconds: Option<f64>,
    cache_disabled: bool,
    open: F,
) -> Result<R, Error>
where
    F: FnOnce(SharedHighLevelClient<RubyHandler>, ErasedReadCache) -> Fut,
    Fut: Future<Output = Result<R, E>> + Send + 'static,
    R: Send,
    E: Display + Send,
{
    Client::check_fork(ruby, this)?;
    let cache = read_cache(ruby, cache_seconds, cache_disabled)?;
    this.bridge
        .wait_for(ruby, open(this.inner.clone(), cache), Span::current())?
        .map_err(|error| Error::new(ruby.exception_runtime_error(), error.to_string()))
}

pub(super) fn published_value(
    ruby: &Ruby,
    this: &Client,
    subsystem: String,
    name: String,
    cache_seconds: Option<f64>,
    cache_disabled: bool,
) -> Result<NativePublishedValue, Error> {
    let inner = open_reader(
        ruby,
        this,
        cache_seconds,
        cache_disabled,
        |client, cache| async move { client.value_state(subsystem, name, cache).await },
    )?;
    Ok(NativePublishedValue {
        inner,
        bridge: this.bridge.clone(),
    })
}

pub(super) fn published_map(
    ruby: &Ruby,
    this: &Client,
    subsystem: String,
    name: String,
    cache_seconds: Option<f64>,
    cache_disabled: bool,
) -> Result<NativePublishedMap, Error> {
    let inner = open_reader(
        ruby,
        this,
        cache_seconds,
        cache_disabled,
        |client, cache| async move { client.map_state(subsystem, name, cache).await },
    )?;
    Ok(NativePublishedMap {
        inner,
        bridge: this.bridge.clone(),
        propagator: Arc::clone(&this.propagator),
    })
}

pub(super) fn published_set(
    ruby: &Ruby,
    this: &Client,
    subsystem: String,
    name: String,
    cache_seconds: Option<f64>,
    cache_disabled: bool,
) -> Result<NativePublishedSet, Error> {
    let inner = open_reader(
        ruby,
        this,
        cache_seconds,
        cache_disabled,
        |client, cache| async move { client.set_state(subsystem, name, cache).await },
    )?;
    Ok(NativePublishedSet {
        inner,
        bridge: this.bridge.clone(),
        propagator: Arc::clone(&this.propagator),
    })
}

pub(super) fn published_deque(
    ruby: &Ruby,
    this: &Client,
    subsystem: String,
    name: String,
    cache_seconds: Option<f64>,
    cache_disabled: bool,
) -> Result<NativePublishedDeque, Error> {
    let inner = open_reader(
        ruby,
        this,
        cache_seconds,
        cache_disabled,
        |client, cache| async move { client.deque_state(subsystem, name, cache).await },
    )?;
    Ok(NativePublishedDeque {
        inner,
        bridge: this.bridge.clone(),
        propagator: Arc::clone(&this.propagator),
    })
}
