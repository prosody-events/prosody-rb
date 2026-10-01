//! Published-state reader constructors for `Prosody::Client`.
//!
//! Each constructor opens a core reader through the bridge and wraps it in the
//! matching native reader class.

use super::config::read_cache_policy;
use super::{Client, RubyHandler};
use crate::handler::state_error;
use crate::published::{
    NativePublishedDeque, NativePublishedMap, NativePublishedSet, NativePublishedValue, Reads,
};
use magnus::{Error, Ruby, Value};
use prosody::high_level::HighLevelClientError;
use prosody::high_level::erased::{ErasedReadCache, ErasedReaderBuildError, SharedHighLevelClient};
use serde_magnus::deserialize;
use std::fmt::Display;
use std::sync::Arc;
use tracing::Span;

/// Opens one published-state reader, and returns it with the client's
/// bridge, propagator, and fork guard. `open` receives the shared client and
/// the resolved cache policy. A state reader error raises the matching typed
/// state error. Any other open failure raises `RuntimeError`.
fn open_reader<F, Fut, R, E>(
    ruby: &Ruby,
    this: &Client,
    read_cache: Value,
    open: F,
) -> Result<(R, Reads), Error>
where
    F: FnOnce(SharedHighLevelClient<RubyHandler>, ErasedReadCache) -> Fut,
    Fut: Future<Output = Result<R, ErasedReaderBuildError<E>>> + Send + 'static,
    R: Send,
    E: Display + Send,
{
    this.fork.check(ruby)?;
    let cache = read_cache_policy("read_cache", deserialize(ruby, read_cache)?)
        .map_err(|error| Error::new(ruby.exception_arg_error(), error))?;
    let reader = this
        .bridge
        .wait_for(ruby, open(this.inner.clone(), cache), Span::current())?
        .map_err(|error| match error {
            ErasedReaderBuildError::Client(HighLevelClientError::StateReader(error)) => {
                state_error(ruby, &error.into())
            }
            error => Error::new(ruby.exception_runtime_error(), error.to_string()),
        })?;
    let reads = Reads {
        bridge: this.bridge.clone(),
        propagator: Arc::clone(&this.propagator),
        fork: this.fork,
    };
    Ok((reader, reads))
}

pub(super) fn published_value(
    ruby: &Ruby,
    this: &Client,
    subsystem: String,
    name: String,
    read_cache: Value,
) -> Result<NativePublishedValue, Error> {
    let (inner, reads) = open_reader(ruby, this, read_cache, |client, cache| async move {
        client.value_state(subsystem, name, cache).await
    })?;
    Ok(NativePublishedValue { inner, reads })
}

pub(super) fn published_map(
    ruby: &Ruby,
    this: &Client,
    subsystem: String,
    name: String,
    read_cache: Value,
) -> Result<NativePublishedMap, Error> {
    let (inner, reads) = open_reader(ruby, this, read_cache, |client, cache| async move {
        client.map_state(subsystem, name, cache).await
    })?;
    Ok(NativePublishedMap { inner, reads })
}

pub(super) fn published_set(
    ruby: &Ruby,
    this: &Client,
    subsystem: String,
    name: String,
    read_cache: Value,
) -> Result<NativePublishedSet, Error> {
    let (inner, reads) = open_reader(ruby, this, read_cache, |client, cache| async move {
        client.set_state(subsystem, name, cache).await
    })?;
    Ok(NativePublishedSet { inner, reads })
}

pub(super) fn published_deque(
    ruby: &Ruby,
    this: &Client,
    subsystem: String,
    name: String,
    read_cache: Value,
) -> Result<NativePublishedDeque, Error> {
    let (inner, reads) = open_reader(ruby, this, read_cache, |client, cache| async move {
        client.deque_state(subsystem, name, cache).await
    })?;
    Ok(NativePublishedDeque { inner, reads })
}
