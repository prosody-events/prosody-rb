//! Conversion of the keyed-state options into a [`KeyedStateConfiguration`].
//!
//! This module parses each declared state collection and registers its
//! descriptor. It also maps the cache, read cache, and subsystem options.

use super::NativeConfiguration;
use crate::util::seconds;
use prosody::consumer::KeyedStateConfiguration;
use prosody::consumer::kafka_state::{message_deque_state, message_map_state, message_state};
use prosody::loader::KafkaLoader;
use prosody::state::ReadCachePolicy;
use prosody::state::descriptor::{StateDescriptor, deque_state, map_state, set_state, value_state};
use prosody::state::order_codec::Utf8KeyCodec;
use prosody::subsystem::SubsystemName;
use prosody::timers::duration::CompactDuration;
use prosody::{ByteSize, JsonCodec};
use serde::Deserialize;
use std::num::NonZeroUsize;
use std::path::PathBuf;

/// Declares one keyed-state collection to register before subscribe.
#[derive(Clone, Debug, Deserialize)]
pub(super) struct StateCollectionConfig {
    /// The collection name. Prosody requires it to be non-empty and unique.
    name: String,

    /// The collection kind: `"value"`, `"map"`, `"set"`, or `"deque"`.
    kind: String,

    /// The item payload: `"json"` or `"message"`. A set stores membership
    /// only, so it has no payload.
    payload: Option<String>,

    /// Optional per-write TTL in whole seconds.
    ttl_seconds: Option<u32>,

    /// Optional opt-out of transactional staging.
    read_uncommitted: Option<bool>,

    /// Whether other consumer groups may read this JSON or set collection.
    published: Option<bool>,

    /// Optional map or set keyset bound (`0..=4096`). Prosody enforces the
    /// ceiling.
    keyset_limit: Option<usize>,

    /// Optional deque-only window capacity. Runtime-only and not persisted.
    capacity: Option<NonZeroUsize>,
}

/// A read cache option: `false` disables the cache, and a number sets the
/// TTL in seconds.
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(untagged)]
pub(crate) enum ReadCacheConfig {
    Disabled(bool),
    Ttl(f64),
}

/// The kind of a keyed-state collection.
enum CollectionKind {
    /// A single-value collection.
    Value,
    /// A `String`-keyed ordered map.
    Map,
    /// A presence-only ordered set of `String` members.
    Set,
    /// A deque.
    Deque,
}

/// The item payload of a value, map, or deque collection.
enum CollectionPayload {
    /// JSON values.
    Json,
    /// The full Kafka message the handler received.
    Message,
}

/// Parses a collection-kind token.
///
/// # Errors
///
/// Returns a permanent-category error naming the field if the token is not
/// `"value"`, `"map"`, `"set"`, or `"deque"`.
fn parse_kind(index: usize, kind: &str) -> Result<CollectionKind, String> {
    match kind {
        "value" => Ok(CollectionKind::Value),
        "map" => Ok(CollectionKind::Map),
        "set" => Ok(CollectionKind::Set),
        "deque" => Ok(CollectionKind::Deque),
        other => Err(format!(
            "state_collections[{index}].kind: expected \"value\", \"map\", \"set\", or \"deque\", \
             got {other:?}"
        )),
    }
}

/// Parses a collection-payload token.
///
/// # Errors
///
/// Returns a permanent-category error naming the field if the token is not
/// `"json"` or `"message"`.
fn parse_payload(index: usize, payload: &str) -> Result<CollectionPayload, String> {
    match payload {
        "json" => Ok(CollectionPayload::Json),
        "message" => Ok(CollectionPayload::Message),
        other => Err(format!(
            "state_collections[{index}].payload: expected \"json\" or \"message\", got {other:?}"
        )),
    }
}

/// Applies the options that every collection kind takes: TTL, commit mode,
/// and publication.
fn with_def<D: StateDescriptor>(descriptor: D, collection: &StateCollectionConfig) -> D {
    let mut descriptor = descriptor;
    if let Some(ttl) = collection.ttl_seconds {
        descriptor = descriptor.ttl(CompactDuration::new(ttl));
    }
    if collection.read_uncommitted == Some(true) {
        descriptor = descriptor.read_uncommitted();
    }
    if let Some(published) = collection.published {
        descriptor = descriptor.published(published);
    }
    descriptor
}

/// Maps one collection into its descriptor. Values, maps, and deques hold
/// JSON or messages. Sets have no payload.
///
/// # Errors
///
/// Returns a permanent-category error when a host value cannot be mapped into
/// a Prosody type.
fn register_state_collection(
    keyed: &mut KeyedStateConfiguration,
    index: usize,
    collection: &StateCollectionConfig,
) -> Result<(), String> {
    let kind = parse_kind(index, &collection.kind)?;
    let payload = collection
        .payload
        .as_deref()
        .map(|payload| parse_payload(index, payload))
        .transpose()?;
    let keyset_limit = keyset_limit(collection.keyset_limit, &kind, index)?;
    let capacity = capacity(collection.capacity, &kind, index)?;

    let name = collection.name.as_str();
    match (kind, payload) {
        (CollectionKind::Value, Some(CollectionPayload::Json)) => {
            let _ = keyed.register(with_def(value_state::<JsonCodec>(name), collection));
        }
        (CollectionKind::Map, Some(CollectionPayload::Json)) => {
            let mut descriptor = with_def(map_state::<Utf8KeyCodec, JsonCodec>(name), collection);
            if let Some(limit) = keyset_limit {
                descriptor = descriptor.keyset_limit(limit);
            }
            let _ = keyed.register(descriptor);
        }
        (CollectionKind::Deque, Some(CollectionPayload::Json)) => {
            let mut descriptor = with_def(deque_state::<JsonCodec>(name), collection);
            if let Some(capacity) = capacity {
                descriptor = descriptor.capacity(capacity);
            }
            let _ = keyed.register(descriptor);
        }
        (CollectionKind::Value, Some(CollectionPayload::Message)) => {
            let descriptor = message_state::<KafkaLoader<JsonCodec>>(name);
            let _ = keyed.register(with_def(descriptor, collection));
        }
        (CollectionKind::Map, Some(CollectionPayload::Message)) => {
            let descriptor = message_map_state::<Utf8KeyCodec, KafkaLoader<JsonCodec>>(name);
            let mut descriptor = with_def(descriptor, collection);
            if let Some(limit) = keyset_limit {
                descriptor = descriptor.keyset_limit(limit);
            }
            let _ = keyed.register(descriptor);
        }
        (CollectionKind::Deque, Some(CollectionPayload::Message)) => {
            let descriptor = message_deque_state::<KafkaLoader<JsonCodec>>(name);
            let mut descriptor = with_def(descriptor, collection);
            if let Some(capacity) = capacity {
                descriptor = descriptor.capacity(capacity);
            }
            let _ = keyed.register(descriptor);
        }
        (CollectionKind::Set, None) => {
            let mut descriptor = with_def(set_state::<Utf8KeyCodec>(name), collection);
            if let Some(limit) = keyset_limit {
                descriptor = descriptor.keyset_limit(limit);
            }
            let _ = keyed.register(descriptor);
        }
        (CollectionKind::Set, Some(_)) => {
            return Err(format!(
                "state_collections[{index}].payload: omit it for a set collection"
            ));
        }
        (CollectionKind::Value | CollectionKind::Map | CollectionKind::Deque, None) => {
            return Err(format!(
                "state_collections[{index}].payload: required for value, map, and deque \
                 collections"
            ));
        }
    }

    Ok(())
}

/// Rejects a keyset limit on a collection that is not a map or a set.
fn keyset_limit(
    value: Option<usize>,
    kind: &CollectionKind,
    index: usize,
) -> Result<Option<usize>, String> {
    if value.is_some() && !matches!(kind, CollectionKind::Map | CollectionKind::Set) {
        return Err(format!(
            "state_collections[{index}].keyset_limit: only valid for map and set collections"
        ));
    }
    Ok(value)
}

/// Rejects a capacity on a collection that is not a deque.
fn capacity(
    value: Option<NonZeroUsize>,
    kind: &CollectionKind,
    index: usize,
) -> Result<Option<NonZeroUsize>, String> {
    if value.is_some() && !matches!(kind, CollectionKind::Deque) {
        return Err(format!(
            "state_collections[{index}].capacity: only valid for deque collections"
        ));
    }
    Ok(value)
}

/// Converts a read cache option into a core policy. An absent option inherits
/// the default.
///
/// # Errors
///
/// Returns an error that names `option` for `true` or an invalid duration.
pub(crate) fn read_cache_policy(
    option: &str,
    config: Option<ReadCacheConfig>,
) -> Result<ReadCachePolicy, String> {
    match config {
        None => Ok(ReadCachePolicy::Inherit),
        Some(ReadCacheConfig::Disabled(false)) => Ok(ReadCachePolicy::Disabled),
        Some(ReadCacheConfig::Disabled(true)) => Err(format!(
            "{option}: true is ambiguous; use a duration or false"
        )),
        Some(ReadCacheConfig::Ttl(value)) => seconds(option, value).map(ReadCachePolicy::Ttl),
    }
}

/// Builds the `KeyedStateConfiguration` by mapping each declared collection.
/// The normal Prosody construction path validates the result.
///
/// # Errors
///
/// Returns an error if a host value cannot be mapped.
pub(super) fn build_keyed_state_config(
    config: &NativeConfiguration,
) -> Result<KeyedStateConfiguration, String> {
    let mut builder = KeyedStateConfiguration::builder();

    if let Some(dir) = &config.state_cache_dir {
        builder.cache_dir(PathBuf::from(dir));
    }

    if let Some(size) = &config.state_owned_cache_size {
        let size = size
            .parse::<ByteSize>()
            .map_err(|error| format!("state_owned_cache_size: {error}"))?;
        builder.owned_cache_size(Some(size));
    }

    if let Some(size) = &config.state_memtable_size {
        let size = size
            .parse::<ByteSize>()
            .map_err(|error| format!("state_memtable_size: {error}"))?;
        builder.memtable_size(Some(size));
    }

    if let Some(size) = &config.state_read_cache_size {
        let size = size
            .parse::<ByteSize>()
            .map_err(|error| format!("state_read_cache_size: {error}"))?;
        builder.read_cache_size(Some(size));
    }

    match read_cache_policy("state_read_cache", config.state_read_cache)? {
        ReadCachePolicy::Inherit => {}
        ReadCachePolicy::Disabled => {
            builder.read_cache_ttl(None);
        }
        ReadCachePolicy::Ttl(ttl) => {
            builder.read_cache_ttl(Some(ttl));
        }
    }

    if let Some(subsystem) = &config.subsystem {
        builder.subsystem(Some(
            SubsystemName::try_new(subsystem).map_err(|error| error.to_string())?,
        ));
    }

    let mut keyed = builder.build().map_err(|error| error.to_string())?;

    if let Some(collections) = &config.state_collections {
        for (index, collection) in collections.iter().enumerate() {
            register_state_collection(&mut keyed, index, collection)?;
        }
    }

    Ok(keyed)
}
