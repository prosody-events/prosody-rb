//! Conversion of the keyed-state options into a [`KeyedStateConfiguration`].
//!
//! This module parses each declared state collection and registers its
//! descriptor. It also maps the cache, read cache, and subsystem options.

use super::NativeConfiguration;
use prosody::consumer::KeyedStateConfiguration;
use prosody::consumer::kafka_state::{message_deque_state, message_map_state, message_state};
use prosody::loader::KafkaLoader;
use prosody::state::descriptor::{
    DequeDescriptor, MapDescriptor, SetDescriptor, StateDescriptor, deque_state, map_state,
    set_state, value_state,
};
use prosody::state::order_codec::Utf8KeyCodec;
use prosody::subsystem::SubsystemName;
use prosody::timers::duration::CompactDuration;
use prosody::{ByteSize, JsonCodec};
use serde::Deserialize;
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::time::Duration;

/// Declares one keyed-state collection to register before subscribe.
#[derive(Clone, Debug, Deserialize)]
pub(super) struct StateCollectionConfig {
    /// The collection name. Prosody requires it to be non-empty and unique.
    name: String,

    /// The collection kind: `"value"`, `"map"`, `"set"`, or `"deque"`.
    kind: String,

    /// The item payload: `"json"` or `"message"`, or `"presence"` for a set.
    payload: String,

    /// Optional per-write TTL in whole seconds. Crosses as `f64` so
    /// fractional/negative/non-finite values reach the whole-number guard.
    ttl_seconds: Option<f64>,

    /// Optional opt-out of transactional staging.
    read_uncommitted: Option<bool>,

    /// Whether other consumer groups may read this JSON or set collection.
    published: Option<bool>,

    /// Optional map or set keyset bound (`0..=4096`). The binding rejects
    /// values that cannot map to an unsigned integer. Prosody enforces the
    /// ceiling.
    keyset_limit: Option<f64>,

    /// Optional deque-only window capacity (`>= 1`). Runtime-only and not
    /// persisted. Crosses as `f64` so fractional/negative/non-finite values
    /// reach the whole-number guard.
    capacity: Option<f64>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub(super) enum ReadCacheConfig {
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

/// The item payload of a keyed-state collection.
enum CollectionPayload {
    /// JSON values.
    Json,
    /// The full Kafka message the handler received.
    Message,
    /// Membership only. Only sets use it.
    Presence,
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
            "state_collections[{index}].kind: expected \"value\", \"map\", \"set\", or \
             \"deque\", got {other:?}"
        )),
    }
}

/// Parses a collection-payload token.
///
/// # Errors
///
/// Returns a permanent-category error naming the field if the token is not
/// `"json"`, `"message"`, or `"presence"`.
fn parse_payload(index: usize, payload: &str) -> Result<CollectionPayload, String> {
    match payload {
        "json" => Ok(CollectionPayload::Json),
        "message" => Ok(CollectionPayload::Message),
        "presence" => Ok(CollectionPayload::Presence),
        other => Err(format!(
            "state_collections[{index}].payload: expected \"json\", \"message\", or \
             \"presence\", got {other:?}"
        )),
    }
}

/// Validates a numeric field as a whole number within `min..=max`.
///
/// The field arrives as an `f64` (the raw Ruby number, un-coerced) so that
/// fractional, negative, and non-finite values reach this guard instead of
/// being silently truncated or wrapped by an earlier integer conversion.
///
/// # Errors
///
/// Returns a permanent-category error naming the field if the value is not a
/// whole number in the inclusive range.
fn whole_number_field(value: f64, field: &str, min: u32, max: u32) -> Result<u32, String> {
    if value.is_finite()
        && value.fract() == 0.0
        && value >= f64::from(min)
        && value <= f64::from(max)
    {
        Ok(value as u32)
    } else {
        Err(format!("{field}: must be a whole number in {min}..={max}"))
    }
}

/// Applies the shared descriptor options (TTL, commit mode) fluently.
fn with_def<D: StateDescriptor>(
    descriptor: D,
    ttl_seconds: Option<u32>,
    read_uncommitted: Option<bool>,
    published: Option<bool>,
) -> D {
    let mut descriptor = descriptor;
    if let Some(ttl) = ttl_seconds {
        descriptor = descriptor.ttl(CompactDuration::new(ttl));
    }
    if read_uncommitted == Some(true) {
        descriptor = descriptor.read_uncommitted();
    }
    if let Some(published) = published {
        descriptor = descriptor.published(published);
    }
    descriptor
}

/// Applies the map keyset bound when configured.
fn with_keyset<KC, V>(
    descriptor: MapDescriptor<KC, V>,
    keyset_limit: Option<u32>,
) -> MapDescriptor<KC, V> {
    match keyset_limit {
        Some(limit) => descriptor.keyset_limit(limit as usize),
        None => descriptor,
    }
}

/// Applies the set keyset bound when configured.
fn with_set_keyset<KC>(
    descriptor: SetDescriptor<KC>,
    keyset_limit: Option<u32>,
) -> SetDescriptor<KC> {
    match keyset_limit {
        Some(limit) => descriptor.keyset_limit(limit as usize),
        None => descriptor,
    }
}

/// Applies the deque-only window capacity when configured.
fn with_capacity<T>(
    descriptor: DequeDescriptor<T>,
    capacity: Option<NonZeroUsize>,
) -> DequeDescriptor<T> {
    match capacity {
        Some(cap) => descriptor.capacity(cap),
        None => descriptor,
    }
}

/// Maps one collection into its descriptor. Values, maps, and deques hold
/// JSON or messages. Sets, and only sets, hold presence.
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
    let payload = parse_payload(index, &collection.payload)?;

    let ttl_seconds = match collection.ttl_seconds {
        Some(value) => Some(whole_number_field(
            value,
            &format!("state_collections[{index}].ttl_seconds"),
            0,
            u32::MAX,
        )?),
        None => None,
    };

    let keyset_limit = keyset_limit(collection.keyset_limit, &kind, index)?;
    let capacity = capacity(collection.capacity, &kind, index)?;

    let read_uncommitted = collection.read_uncommitted;
    let name = collection.name.as_str();
    match (kind, payload) {
        (CollectionKind::Value, CollectionPayload::Json) => {
            let _ = keyed.register(with_def(
                value_state::<JsonCodec>(name),
                ttl_seconds,
                read_uncommitted,
                collection.published,
            ));
        }
        (CollectionKind::Map, CollectionPayload::Json) => {
            let descriptor = with_def(
                map_state::<Utf8KeyCodec, JsonCodec>(name),
                ttl_seconds,
                read_uncommitted,
                collection.published,
            );
            let _ = keyed.register(with_keyset(descriptor, keyset_limit));
        }
        (CollectionKind::Deque, CollectionPayload::Json) => {
            let descriptor = with_def(
                deque_state::<JsonCodec>(name),
                ttl_seconds,
                read_uncommitted,
                collection.published,
            );
            let _ = keyed.register(with_capacity(descriptor, capacity));
        }
        (CollectionKind::Value, CollectionPayload::Message) => {
            let _ = keyed.register(with_def(
                message_state::<KafkaLoader<JsonCodec>>(name),
                ttl_seconds,
                read_uncommitted,
                collection.published,
            ));
        }
        (CollectionKind::Map, CollectionPayload::Message) => {
            let descriptor = with_def(
                message_map_state::<Utf8KeyCodec, KafkaLoader<JsonCodec>>(name),
                ttl_seconds,
                read_uncommitted,
                collection.published,
            );
            let _ = keyed.register(with_keyset(descriptor, keyset_limit));
        }
        (CollectionKind::Deque, CollectionPayload::Message) => {
            let descriptor = with_def(
                message_deque_state::<KafkaLoader<JsonCodec>>(name),
                ttl_seconds,
                read_uncommitted,
                collection.published,
            );
            let _ = keyed.register(with_capacity(descriptor, capacity));
        }
        (CollectionKind::Set, CollectionPayload::Presence) => {
            let descriptor = with_def(
                set_state::<Utf8KeyCodec>(name),
                ttl_seconds,
                read_uncommitted,
                collection.published,
            );
            let _ = keyed.register(with_set_keyset(descriptor, keyset_limit));
        }
        (CollectionKind::Set, CollectionPayload::Json | CollectionPayload::Message) => {
            return Err(format!(
                "state_collections[{index}].payload: set collections use \"presence\""
            ));
        }
        (
            CollectionKind::Value | CollectionKind::Map | CollectionKind::Deque,
            CollectionPayload::Presence,
        ) => {
            return Err(format!(
                "state_collections[{index}].payload: \"presence\" is only valid for set \
                 collections"
            ));
        }
    }

    Ok(())
}

fn keyset_limit(
    value: Option<f64>,
    kind: &CollectionKind,
    index: usize,
) -> Result<Option<u32>, String> {
    let Some(value) = value else {
        return Ok(None);
    };
    if !matches!(kind, CollectionKind::Map | CollectionKind::Set) {
        return Err(format!(
            "state_collections[{index}].keyset_limit: only valid for map and set collections"
        ));
    }
    whole_number_field(
        value,
        &format!("state_collections[{index}].keyset_limit"),
        0,
        u32::MAX,
    )
    .map(Some)
}

fn capacity(
    value: Option<f64>,
    kind: &CollectionKind,
    index: usize,
) -> Result<Option<NonZeroUsize>, String> {
    let Some(value) = value else {
        return Ok(None);
    };
    if !matches!(kind, CollectionKind::Deque) {
        return Err(format!(
            "state_collections[{index}].capacity: only valid for deque collections"
        ));
    }
    let value = whole_number_field(
        value,
        &format!("state_collections[{index}].capacity"),
        1,
        u32::MAX,
    )?;
    Ok(NonZeroUsize::new(value as usize))
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

    if let Some(cache) = &config.state_read_cache {
        match cache {
            ReadCacheConfig::Disabled(false) => {
                builder.read_cache_ttl(None);
            }
            ReadCacheConfig::Disabled(true) => {
                return Err(
                    "state_read_cache: true is ambiguous; use a duration or false".to_owned(),
                );
            }
            ReadCacheConfig::Ttl(seconds) => {
                let ttl = Duration::try_from_secs_f64(*seconds).map_err(|_| {
                    "state_read_cache: duration must be finite and non-negative".to_owned()
                })?;
                builder.read_cache_ttl(Some(ttl));
            }
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
