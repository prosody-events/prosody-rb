//! # Configuration Module for Prosody Client
//!
//! This module handles the conversion between Ruby configuration objects and
//! the native Rust configuration structures needed by the Prosody library.
//! It defines serialization/deserialization logic and conversion traits that
//! transform Ruby configuration values into appropriate Prosody configuration
//! builders.

use magnus::{Error, Ruby, Value};
use prosody::JsonCodec;
use prosody::cassandra::config::CassandraConfigurationBuilder;
use prosody::consumer::ConsumerConfigurationBuilder;
use prosody::consumer::KeyedStateConfiguration;
use prosody::consumer::SpanRelation;
use prosody::consumer::kafka_state::{message_deque_state, message_map_state, message_state};
use prosody::consumer::middleware::deduplication::DeduplicationConfigurationBuilder;
use prosody::consumer::middleware::defer::DeferConfigurationBuilder;
use prosody::consumer::middleware::monopolization::MonopolizationConfigurationBuilder;
use prosody::consumer::middleware::retry::RetryConfigurationBuilder;
use prosody::consumer::middleware::scheduler::SchedulerConfigurationBuilder;
use prosody::consumer::middleware::timeout::TimeoutConfigurationBuilder;
use prosody::consumer::middleware::topic::FailureTopicConfigurationBuilder;
use prosody::high_level::ConsumerBuilders;
use prosody::high_level::mode::Mode;
use prosody::loader::KafkaLoader;
use prosody::loader::KafkaLoaderConfiguration;
use prosody::producer::ProducerConfigurationBuilder;
use prosody::state::descriptor::{
    DequeDescriptor, MapDescriptor, StateDescriptor, deque_state, map_state, value_state,
};
use prosody::state::order_codec::Utf8KeyCodec;
use prosody::subsystem::SubsystemName;
use prosody::telemetry::emitter::TelemetryEmitterConfiguration;
use prosody::timers::duration::CompactDuration;
use serde::{Deserialize, Deserializer};
use serde_magnus::deserialize;
use serde_untagged::UntaggedEnumVisitor;
use std::collections::HashSet;
use std::num::{NonZeroU64, NonZeroUsize};
use std::path::PathBuf;
use std::time::Duration;

/// Configuration structure for the Prosody client that maps Ruby configuration
/// values to their native Rust equivalents.
///
/// This structure contains all possible configuration options that can be
/// provided by the Ruby side, which are then converted to the appropriate
/// Prosody configuration builder types.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct NativeConfiguration {
    /// List of Kafka bootstrap server addresses
    bootstrap_servers: Option<Vec<String>>,

    /// Whether to use mock mode (for testing)
    mock: Option<bool>,

    /// Maximum time to wait for a send operation to complete (in seconds)
    send_timeout: Option<f32>,

    /// Kafka consumer group ID
    group_id: Option<String>,

    /// Global shared cache capacity across all partitions for message
    /// deduplication
    idempotence_cache_size: Option<u32>,

    /// Version string for cache-busting deduplication hashes
    idempotence_version: Option<String>,

    /// TTL for deduplication records in Cassandra (in seconds)
    idempotence_ttl: Option<f64>,

    /// List of Kafka topics to subscribe to
    subscribed_topics: Option<Vec<String>>,

    /// List of event types that the consumer is allowed to process
    allowed_events: Option<Vec<String>>,

    /// Identifier for the system producing messages
    source_system: Option<String>,

    /// Maximum number of concurrent message processing tasks
    max_concurrency: Option<u32>,

    /// Maximum number of messages to process before committing offsets
    max_uncommitted: Option<u16>,

    /// Threshold in seconds after which a stalled consumer is detected
    stall_threshold: Option<f32>,

    /// Maximum time to wait for a clean shutdown (in seconds)
    shutdown_timeout: Option<f32>,

    /// Interval between Kafka poll operations (in seconds)
    poll_interval: Option<f32>,

    /// Interval between offset commit operations (in seconds)
    commit_interval: Option<f32>,

    /// Operation mode of the client (`pipeline`, `low_latency`, `best_effort`)
    mode: Option<String>,

    /// Base delay for retry operations (in seconds)
    retry_base: Option<f32>,

    /// Maximum number of retry attempts
    max_retries: Option<u32>,

    /// Maximum delay between retries (in seconds)
    max_retry_delay: Option<f32>,

    /// Topic to send failed messages to
    failure_topic: Option<String>,

    /// Configuration for the health probe port
    probe_port: Option<ProbePort>,

    /// List of Cassandra contact nodes (hostnames or IPs)
    cassandra_nodes: Option<Vec<String>>,

    /// Keyspace to use for storing timer data in Cassandra
    cassandra_keyspace: Option<String>,

    /// Preferred datacenter for Cassandra query routing
    cassandra_datacenter: Option<String>,

    /// Preferred rack identifier for Cassandra topology-aware routing
    cassandra_rack: Option<String>,

    /// Username for authenticating with Cassandra
    cassandra_user: Option<String>,

    /// Password for authenticating with Cassandra
    cassandra_password: Option<String>,

    /// Retention period for failed/unprocessed timer data in Cassandra (in
    /// seconds)
    cassandra_retention: Option<f32>,

    /// Timer slab partitioning duration in seconds.
    /// Controls how timers are grouped for storage and retrieval.
    slab_size: Option<f32>,

    // Scheduler configuration
    /// Target proportion of execution time for failure/retry task processing
    /// (0.0 to 1.0). Controls bandwidth allocation between Normal and
    /// Failure task classes.
    scheduler_failure_weight: Option<f64>,

    /// Wait duration (in seconds) at which urgency boost reaches maximum
    /// intensity.
    scheduler_max_wait: Option<f32>,

    /// Maximum urgency boost (in seconds of virtual time) for waiting tasks.
    scheduler_wait_weight: Option<f64>,

    /// Cache capacity for tracking per-key virtual time in the scheduler.
    scheduler_cache_size: Option<u32>,

    // Monopolization configuration
    /// Whether monopolization detection is enabled.
    monopolization_enabled: Option<bool>,

    /// Threshold for monopolization detection (0.0 to 1.0).
    monopolization_threshold: Option<f64>,

    /// Rolling window duration (in seconds) for monopolization detection.
    monopolization_window: Option<f32>,

    /// Cache size for tracking key execution intervals.
    monopolization_cache_size: Option<u32>,

    // Defer configuration
    /// Whether deferral is enabled for new messages.
    defer_enabled: Option<bool>,

    /// Base exponential backoff delay for deferred retries (in seconds).
    defer_base: Option<f32>,

    /// Maximum delay between deferred retries (in seconds).
    defer_max_delay: Option<f32>,

    /// Failure rate threshold for enabling deferral (0.0 to 1.0).
    defer_failure_threshold: Option<f64>,

    /// Sliding window duration (in seconds) for failure rate tracking.
    defer_failure_window: Option<f32>,

    /// Cache size for defer middleware.
    defer_cache_size: Option<u32>,

    /// Maximum number of deferred store entries kept in the write-through cache
    /// per Cassandra defer store.
    defer_store_cache_size: Option<u32>,

    /// Timeout for Kafka seek operations (in seconds).
    defer_seek_timeout: Option<f32>,

    /// Messages to read sequentially before seeking.
    defer_discard_threshold: Option<i64>,

    // Timeout configuration
    /// Fixed timeout duration for handler execution (in seconds).
    timeout: Option<f32>,

    // Telemetry emitter configuration
    /// Kafka topic to produce telemetry events to.
    telemetry_topic: Option<String>,

    /// Whether the telemetry emitter is enabled.
    telemetry_enabled: Option<bool>,

    // OTel span linking
    /// Span linking for message execution spans (`child` or `follows_from`).
    message_spans: Option<String>,

    /// Span linking for timer execution spans (`child` or `follows_from`).
    timer_spans: Option<String>,

    // Keyed-state configuration
    /// Keyed-state collections to register before subscribe.
    state_collections: Option<Vec<StateCollectionConfig>>,

    /// Subsystem under which published collections are advertised.
    subsystem: Option<String>,

    /// Root directory for the local keyed-state cache. Must not be empty.
    state_cache_dir: Option<String>,

    /// Capacity of the in-memory keyed-state cache, in bytes.
    state_cache_size_bytes: Option<u64>,

    /// Byte budget for the published-state read-through cache.
    state_read_cache_size_bytes: Option<u64>,

    /// Default cache policy for published-state reads.
    state_read_cache: Option<ReadCacheConfig>,

    /// Delay in whole seconds before the keyed-state recovery sweep.
    ///
    /// Crosses as an `f64` so fractional/negative/non-finite values reach the
    /// whole-number guard rather than being silently truncated.
    state_recovery_delay: Option<f64>,
}

/// Declares one keyed-state collection to register before subscribe.
#[derive(Clone, Debug, Deserialize)]
struct StateCollectionConfig {
    /// The collection name (non-empty, unique within the client).
    name: String,

    /// The collection kind: `"value"`, `"map"`, or `"deque"`.
    kind: String,

    /// The item payload: `"json"` or `"message"`.
    payload: String,

    /// Optional per-write TTL in whole seconds. Crosses as `f64` so
    /// fractional/negative/non-finite values reach the whole-number guard.
    ttl_seconds: Option<f64>,

    /// Optional opt-out of transactional staging.
    read_uncommitted: Option<bool>,

    /// Whether other consumer groups may read this JSON collection.
    published: Option<bool>,

    /// Optional map-only keyset bound (`0..=4096`). Crosses as `f64` so
    /// fractional/negative/non-finite values reach the whole-number guard.
    keyset_limit: Option<f64>,

    /// Optional deque-only window capacity (`>= 1`). Runtime-only and not
    /// persisted. Crosses as `f64` so fractional/negative/non-finite values
    /// reach the whole-number guard.
    capacity: Option<f64>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
enum ReadCacheConfig {
    Disabled(bool),
    Ttl(f64),
}

/// Configuration for the health probe port.
///
/// This enum represents the three possible states for the probe port
/// configuration:
/// - Unconfigured: The default state, where the standard configuration is used
/// - Disabled: Explicitly disables the probe port
/// - Configured: Sets the probe port to a specific port number
#[derive(Copy, Clone, Debug, Default)]
pub enum ProbePort {
    /// Use default configuration
    #[default]
    Unconfigured,

    /// Explicitly disable the probe port
    Disabled,

    /// Use a specific port number
    Configured(u16),
}

impl<'de> Deserialize<'de> for ProbePort {
    /// Deserializes a probe port configuration from various possible input
    /// formats.
    ///
    /// # Arguments
    ///
    /// * `deserializer` - The deserializer to use
    ///
    /// # Returns
    ///
    /// A `ProbePort` enum variant based on the input:
    /// - If a u16 is provided, it returns `ProbePort::Configured(port)`
    /// - If a boolean `true` is provided, it returns `ProbePort::Unconfigured`
    /// - If a boolean `false` is provided, it returns `ProbePort::Disabled`
    /// - If nothing is provided, it returns `ProbePort::Unconfigured`
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        UntaggedEnumVisitor::new()
            .u16(|port| Ok(Self::Configured(port)))
            .bool(|enabled| {
                if enabled {
                    Ok(Self::Unconfigured)
                } else {
                    Ok(Self::Disabled)
                }
            })
            .unit(|| Ok(Self::Unconfigured))
            .deserialize(deserializer)
    }
}

impl NativeConfiguration {
    /// Converts a Ruby value into a `NativeConfiguration`.
    ///
    /// # Arguments
    ///
    /// * `ruby` - Reference to the Ruby VM
    /// * `val` - The Ruby value to convert
    ///
    /// # Returns
    ///
    /// The converted `NativeConfiguration` if successful
    ///
    /// # Errors
    ///
    /// Returns a Magnus error if deserialization fails
    pub fn from_value(ruby: &Ruby, val: Value) -> Result<Self, Error> {
        deserialize(ruby, val)
    }
}

impl<'a> From<&'a NativeConfiguration> for ProducerConfigurationBuilder {
    /// Converts a `NativeConfiguration` reference into a
    /// `ProducerConfigurationBuilder`.
    ///
    /// This takes the relevant producer settings from the configuration and
    /// sets them on a new `ProducerConfigurationBuilder` instance.
    ///
    /// # Arguments
    ///
    /// * `config` - The configuration to convert
    ///
    /// # Returns
    ///
    /// A configured `ProducerConfigurationBuilder`
    fn from(config: &'a NativeConfiguration) -> Self {
        let mut builder = Self::default();

        if let Some(bootstrap_servers) = &config.bootstrap_servers {
            builder.bootstrap_servers(bootstrap_servers.clone());
        }

        if let Some(send_timeout) = &config.send_timeout {
            builder.send_timeout(Duration::from_secs_f32(*send_timeout));
        }

        if let Some(idempotence_cache_size) = &config.idempotence_cache_size {
            builder.idempotence_cache_size(*idempotence_cache_size as usize);
        }

        if let Some(source_system) = &config.source_system {
            builder.source_system(source_system.clone());
        }

        if let Some(mock) = &config.mock {
            builder.mock(*mock);
        }

        builder
    }
}

impl<'a> From<&'a NativeConfiguration> for ConsumerConfigurationBuilder {
    /// Converts a `NativeConfiguration` reference into a
    /// `ConsumerConfigurationBuilder`.
    ///
    /// This takes the relevant consumer settings from the configuration and
    /// sets them on a new `ConsumerConfigurationBuilder` instance.
    ///
    /// # Arguments
    ///
    /// * `config` - The configuration to convert
    ///
    /// # Returns
    ///
    /// A configured `ConsumerConfigurationBuilder`
    fn from(config: &'a NativeConfiguration) -> Self {
        let mut builder = Self::default();

        if let Some(bootstrap_servers) = &config.bootstrap_servers {
            builder.bootstrap_servers(bootstrap_servers.clone());
        }

        if let Some(group_id) = &config.group_id {
            builder.group_id(group_id.clone());
        }

        if let Some(subscribed_topics) = &config.subscribed_topics {
            builder.subscribed_topics(subscribed_topics.clone());
        }

        if let Some(allowed_events) = &config.allowed_events {
            builder.allowed_events(allowed_events.clone());
        }

        if let Some(max_uncommitted) = &config.max_uncommitted {
            builder.max_uncommitted(*max_uncommitted as usize);
        }

        if let Some(stall_threshold) = &config.stall_threshold {
            builder.stall_threshold(Duration::from_secs_f32(*stall_threshold));
        }

        if let Some(shutdown_timeout) = &config.shutdown_timeout {
            builder.shutdown_timeout(Duration::from_secs_f32(*shutdown_timeout));
        }

        if let Some(poll_interval) = &config.poll_interval {
            builder.poll_interval(Duration::from_secs_f32(*poll_interval));
        }

        if let Some(commit_interval) = &config.commit_interval {
            builder.commit_interval(Duration::from_secs_f32(*commit_interval));
        }

        if let Some(mock) = &config.mock {
            builder.mock(*mock);
        }

        if let Some(probe_port) = &config.probe_port {
            match probe_port {
                ProbePort::Unconfigured => {}
                ProbePort::Disabled => {
                    builder.probe_port(None);
                }
                ProbePort::Configured(port) => {
                    builder.probe_port(*port);
                }
            }
        }

        if let Some(slab_size) = &config.slab_size {
            builder.slab_size(Duration::from_secs_f32(*slab_size));
        }

        builder
    }
}

impl<'a> From<&'a NativeConfiguration> for RetryConfigurationBuilder {
    /// Converts a `NativeConfiguration` reference into a
    /// `RetryConfigurationBuilder`.
    ///
    /// This takes the relevant retry settings from the configuration and
    /// sets them on a new `RetryConfigurationBuilder` instance.
    ///
    /// # Arguments
    ///
    /// * `config` - The configuration to convert
    ///
    /// # Returns
    ///
    /// A configured `RetryConfigurationBuilder`
    fn from(config: &'a NativeConfiguration) -> Self {
        let mut builder = Self::default();

        if let Some(retry_base) = &config.retry_base {
            builder.base(Duration::from_secs_f32(*retry_base));
        }

        if let Some(max_retries) = &config.max_retries {
            builder.max_retries(*max_retries);
        }

        if let Some(max_retry_delay) = &config.max_retry_delay {
            builder.max_delay(Duration::from_secs_f32(*max_retry_delay));
        }

        builder
    }
}

impl<'a> From<&'a NativeConfiguration> for FailureTopicConfigurationBuilder {
    /// Converts a `NativeConfiguration` reference into a
    /// `FailureTopicConfigurationBuilder`.
    ///
    /// This takes the relevant failure topic settings from the configuration
    /// and sets them on a new `FailureTopicConfigurationBuilder` instance.
    ///
    /// # Arguments
    ///
    /// * `config` - The configuration to convert
    ///
    /// # Returns
    ///
    /// A configured `FailureTopicConfigurationBuilder`
    fn from(config: &'a NativeConfiguration) -> Self {
        let mut builder = Self::default();

        if let Some(failure_topic) = &config.failure_topic {
            builder.failure_topic(failure_topic.clone());
        }

        builder
    }
}

impl<'a> TryFrom<&'a NativeConfiguration> for Mode {
    type Error = String;

    /// Attempts to convert a `NativeConfiguration` reference into a Prosody
    /// Mode.
    ///
    /// This extracts the mode setting from the configuration and converts it
    /// to a Prosody Mode enum value.
    ///
    /// # Arguments
    ///
    /// * `value` - The configuration to convert
    ///
    /// # Returns
    ///
    /// The corresponding `Mode` if successful
    ///
    /// # Errors
    ///
    /// Returns a String error if the mode is unrecognized
    fn try_from(value: &'a NativeConfiguration) -> Result<Self, Self::Error> {
        let Some(mode_str) = value.mode.as_deref() else {
            return Ok(Mode::default());
        };

        match mode_str {
            "pipeline" => Ok(Mode::Pipeline),
            "low_latency" => Ok(Mode::LowLatency),
            "best_effort" => Ok(Mode::BestEffort),
            string => Err(format!("unrecognized mode: {string}")),
        }
    }
}

impl<'a> From<&'a NativeConfiguration> for CassandraConfigurationBuilder {
    /// Converts a `NativeConfiguration` reference into a
    /// `CassandraConfigurationBuilder`.
    ///
    /// This takes the relevant Cassandra settings from the configuration and
    /// sets them on a new `CassandraConfigurationBuilder` instance.
    ///
    /// # Arguments
    ///
    /// * `config` - The configuration to convert
    ///
    /// # Returns
    ///
    /// A configured `CassandraConfigurationBuilder`
    fn from(config: &'a NativeConfiguration) -> Self {
        let mut builder = Self::default();

        if let Some(nodes) = &config.cassandra_nodes {
            builder.nodes(nodes.clone());
        }

        if let Some(keyspace) = &config.cassandra_keyspace {
            builder.keyspace(keyspace.clone());
        }

        if let Some(datacenter) = &config.cassandra_datacenter {
            builder.datacenter(Some(datacenter.clone()));
        }

        if let Some(rack) = &config.cassandra_rack {
            builder.rack(Some(rack.clone()));
        }

        if let Some(user) = &config.cassandra_user {
            builder.user(Some(user.clone()));
        }

        if let Some(password) = &config.cassandra_password {
            builder.password(Some(password.clone()));
        }

        if let Some(retention) = &config.cassandra_retention {
            builder.retention(Duration::from_secs_f32(*retention));
        }

        builder
    }
}

impl<'a> From<&'a NativeConfiguration> for SchedulerConfigurationBuilder {
    /// Converts a `NativeConfiguration` reference into a
    /// `SchedulerConfigurationBuilder`.
    ///
    /// This takes the relevant scheduler settings from the configuration and
    /// sets them on a new `SchedulerConfigurationBuilder` instance.
    ///
    /// # Arguments
    ///
    /// * `config` - The configuration to convert
    ///
    /// # Returns
    ///
    /// A configured `SchedulerConfigurationBuilder`
    fn from(config: &'a NativeConfiguration) -> Self {
        let mut builder = Self::default();

        if let Some(max_concurrency) = &config.max_concurrency {
            builder.max_concurrency(*max_concurrency as usize);
        }

        if let Some(failure_weight) = &config.scheduler_failure_weight {
            builder.failure_weight(*failure_weight);
        }

        if let Some(max_wait) = &config.scheduler_max_wait {
            builder.max_wait(Duration::from_secs_f32(*max_wait));
        }

        if let Some(wait_weight) = &config.scheduler_wait_weight {
            builder.wait_weight(*wait_weight);
        }

        if let Some(cache_size) = &config.scheduler_cache_size {
            builder.cache_size(*cache_size as usize);
        }

        builder
    }
}

impl<'a> From<&'a NativeConfiguration> for MonopolizationConfigurationBuilder {
    /// Converts a `NativeConfiguration` reference into a
    /// `MonopolizationConfigurationBuilder`.
    ///
    /// This takes the relevant monopolization settings from the configuration
    /// and sets them on a new `MonopolizationConfigurationBuilder` instance.
    ///
    /// # Arguments
    ///
    /// * `config` - The configuration to convert
    ///
    /// # Returns
    ///
    /// A configured `MonopolizationConfigurationBuilder`
    fn from(config: &'a NativeConfiguration) -> Self {
        let mut builder = Self::default();

        if let Some(enabled) = &config.monopolization_enabled {
            builder.enabled(*enabled);
        }

        if let Some(threshold) = &config.monopolization_threshold {
            builder.monopolization_threshold(*threshold);
        }

        if let Some(window) = &config.monopolization_window {
            builder.window_duration(Duration::from_secs_f32(*window));
        }

        if let Some(cache_size) = &config.monopolization_cache_size {
            builder.cache_size(*cache_size as usize);
        }

        builder
    }
}

impl<'a> From<&'a NativeConfiguration> for DeferConfigurationBuilder {
    /// Converts a `NativeConfiguration` reference into a
    /// `DeferConfigurationBuilder`.
    ///
    /// This takes the relevant defer settings from the configuration and
    /// sets them on a new `DeferConfigurationBuilder` instance.
    ///
    /// # Arguments
    ///
    /// * `config` - The configuration to convert
    ///
    /// # Returns
    ///
    /// A configured `DeferConfigurationBuilder`
    fn from(config: &'a NativeConfiguration) -> Self {
        let mut builder = Self::default();

        if let Some(enabled) = &config.defer_enabled {
            builder.enabled(*enabled);
        }

        if let Some(base) = &config.defer_base {
            builder.base(Duration::from_secs_f32(*base));
        }

        if let Some(max_delay) = &config.defer_max_delay {
            builder.max_delay(Duration::from_secs_f32(*max_delay));
        }

        if let Some(failure_threshold) = &config.defer_failure_threshold {
            builder.failure_threshold(*failure_threshold);
        }

        if let Some(failure_window) = &config.defer_failure_window {
            builder.failure_window(Duration::from_secs_f32(*failure_window));
        }

        if let Some(store_cache_size) = &config.defer_store_cache_size {
            builder.store_cache_size(*store_cache_size as usize);
        }

        builder
    }
}

impl<'a> From<&'a NativeConfiguration> for TimeoutConfigurationBuilder {
    /// Converts a `NativeConfiguration` reference into a
    /// `TimeoutConfigurationBuilder`.
    ///
    /// This takes the relevant timeout settings from the configuration and
    /// sets them on a new `TimeoutConfigurationBuilder` instance.
    ///
    /// # Arguments
    ///
    /// * `config` - The configuration to convert
    ///
    /// # Returns
    ///
    /// A configured `TimeoutConfigurationBuilder`
    fn from(config: &'a NativeConfiguration) -> Self {
        let mut builder = Self::default();

        if let Some(timeout) = &config.timeout {
            builder.timeout(Some(Duration::from_secs_f32(*timeout)));
        }

        builder
    }
}

impl<'a> TryFrom<&'a NativeConfiguration> for DeduplicationConfigurationBuilder {
    type Error = String;

    /// Attempts to convert a `NativeConfiguration` reference into a
    /// `DeduplicationConfigurationBuilder`.
    ///
    /// This takes the relevant deduplication settings from the configuration
    /// and sets them on a new `DeduplicationConfigurationBuilder` instance.
    ///
    /// Consumer deduplication is mandatory in the core (it is the keyed-state
    /// commit oracle), so `cache_capacity` is `NonZeroUsize` and a zero
    /// capacity is unrepresentable rather than a silent "disable". An explicit
    /// `idempotence_cache_size` of `0` is therefore rejected here rather than
    /// silently defaulting; this mirrors the sibling `prosody-js` binding and
    /// the core's own rejection of `PROSODY_IDEMPOTENCE_CACHE_SIZE=0`.
    ///
    /// # Arguments
    ///
    /// * `config` - The configuration to convert
    ///
    /// # Returns
    ///
    /// A configured `DeduplicationConfigurationBuilder` if successful
    ///
    /// # Errors
    ///
    /// Returns a `String` error if `idempotence_cache_size` is explicitly set
    /// to `0`.
    fn try_from(config: &'a NativeConfiguration) -> Result<Self, Self::Error> {
        let mut builder = Self::default();

        if let Some(cache_capacity) = &config.idempotence_cache_size {
            let cache_capacity = NonZeroUsize::new(*cache_capacity as usize)
                .ok_or_else(|| "idempotence_cache_size must be greater than 0".to_owned())?;
            builder.cache_capacity(cache_capacity);
        }

        if let Some(version) = &config.idempotence_version {
            builder.version(version.clone());
        }

        if let Some(ttl) = &config.idempotence_ttl
            && ttl.is_finite()
            && *ttl >= 0.0_f64
        {
            builder.ttl(Duration::from_secs_f64(*ttl));
        }

        Ok(builder)
    }
}

impl<'a> TryFrom<&'a NativeConfiguration> for TelemetryEmitterConfiguration {
    type Error = String;

    /// Attempts to convert a `NativeConfiguration` reference into a
    /// `TelemetryEmitterConfiguration`.
    ///
    /// This takes the relevant telemetry emitter settings from the
    /// configuration and constructs a `TelemetryEmitterConfiguration`,
    /// falling back to environment-variable-aware defaults for any unset
    /// fields.
    ///
    /// # Arguments
    ///
    /// * `config` - The configuration to convert
    ///
    /// # Returns
    ///
    /// A configured `TelemetryEmitterConfiguration` if successful
    ///
    /// # Errors
    ///
    /// Returns a `String` error if a related environment variable contains an
    /// unparseable value.
    fn try_from(config: &'a NativeConfiguration) -> Result<Self, Self::Error> {
        let mut builder = Self::builder();

        if let Some(topic) = &config.telemetry_topic {
            builder.topic(topic.clone());
        }

        if let Some(enabled) = &config.telemetry_enabled {
            builder.enabled(*enabled);
        }

        builder.build().map_err(|e| e.to_string())
    }
}

/// The kind of a keyed-state collection.
enum CollectionKind {
    /// A single-value collection.
    Value,
    /// A `String`-keyed ordered map.
    Map,
    /// A deque.
    Deque,
}

/// The item payload of a keyed-state collection.
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
/// `"value"`, `"map"`, or `"deque"`.
fn parse_kind(index: usize, kind: &str) -> Result<CollectionKind, String> {
    match kind {
        "value" => Ok(CollectionKind::Value),
        "map" => Ok(CollectionKind::Map),
        "deque" => Ok(CollectionKind::Deque),
        other => Err(format!(
            "state_collections[{index}].kind: expected \"value\", \"map\", or \"deque\", got \
             {other:?}"
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

/// Applies the map-only keyset bound when configured.
fn with_keyset<KC, V>(
    descriptor: MapDescriptor<KC, V>,
    keyset_limit: Option<u32>,
) -> MapDescriptor<KC, V> {
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

/// Validates one collection and registers its descriptor over the closed 3×2
/// (kind × payload) matrix.
///
/// # Errors
///
/// Returns a permanent-category error naming the offending field if a field is
/// invalid.
fn register_state_collection(
    keyed: &mut KeyedStateConfiguration,
    index: usize,
    collection: &StateCollectionConfig,
) -> Result<(), String> {
    if collection.name.is_empty() {
        return Err(format!(
            "state_collections[{index}].name: must not be empty"
        ));
    }

    let kind = parse_kind(index, &collection.kind)?;
    let payload = parse_payload(index, &collection.payload)?;

    let ttl_seconds = match collection.ttl_seconds {
        Some(value) => Some(whole_number_field(
            value,
            &format!("state_collections[{index}].ttl_seconds"),
            1,
            u32::MAX,
        )?),
        None => None,
    };

    let keyset_limit = keyset_limit(collection.keyset_limit, &kind, index)?;
    let capacity = capacity(collection.capacity, &kind, index)?;

    let read_uncommitted = collection.read_uncommitted;
    if collection.published == Some(true) && matches!(payload, CollectionPayload::Message) {
        return Err(format!(
            "state_collections[{index}].published: published readers support JSON collections only"
        ));
    }
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
    if !matches!(kind, CollectionKind::Map) {
        return Err(format!(
            "state_collections[{index}].keyset_limit: only valid for map collections"
        ));
    }
    whole_number_field(
        value,
        &format!("state_collections[{index}].keyset_limit"),
        0,
        4096,
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

/// Builds the `KeyedStateConfiguration`, registering each declared collection
/// synchronously (before subscribe) and rejecting duplicate names.
///
/// # Errors
///
/// Returns an error if a keyed-state field is invalid or a name is duplicated.
fn build_keyed_state_config(
    config: &NativeConfiguration,
) -> Result<KeyedStateConfiguration, String> {
    let mut builder = KeyedStateConfiguration::builder();

    if let Some(dir) = &config.state_cache_dir {
        if dir.is_empty() {
            return Err("state_cache_dir: must not be an empty string".to_owned());
        }
        builder.cache_dir(PathBuf::from(dir));
    }

    if let Some(seconds) = config.state_recovery_delay {
        let seconds = whole_number_field(seconds, "state_recovery_delay", 1, u32::MAX)?;
        builder.recovery_delay(CompactDuration::new(seconds));
    }

    if let Some(bytes) = config.state_cache_size_bytes {
        let bytes = NonZeroU64::new(bytes)
            .ok_or_else(|| "state_cache_size_bytes: must be greater than 0".to_owned())?;
        builder.cache_size_bytes(Some(bytes));
    }

    if let Some(bytes) = config.state_read_cache_size_bytes {
        let bytes = NonZeroU64::new(bytes)
            .ok_or_else(|| "state_read_cache_size_bytes: must be greater than 0".to_owned())?;
        builder.read_cache_size_bytes(Some(bytes));
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
            ReadCacheConfig::Ttl(seconds) if seconds.is_finite() && *seconds > 0.0_f64 => {
                builder.read_cache_ttl(Some(Duration::from_secs_f64(*seconds)));
            }
            ReadCacheConfig::Ttl(_) => {
                return Err("state_read_cache: duration must be greater than 0".to_owned());
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
        let mut seen = HashSet::with_capacity(collections.len());
        for (index, collection) in collections.iter().enumerate() {
            if !seen.insert(collection.name.as_str()) {
                return Err(format!(
                    "state_collections[{index}].name: duplicate collection name {:?}",
                    collection.name
                ));
            }
            register_state_collection(&mut keyed, index, collection)?;
        }
    }

    Ok(keyed)
}

impl<'a> TryFrom<&'a NativeConfiguration> for ConsumerBuilders {
    type Error = String;

    /// Attempts to convert a `NativeConfiguration` reference into a
    /// `ConsumerBuilders`.
    ///
    /// This creates all the consumer-related configuration builders from
    /// the configuration.
    ///
    /// # Arguments
    ///
    /// * `config` - The configuration to convert
    ///
    /// # Returns
    ///
    /// A `ConsumerBuilders` containing all consumer-related configuration
    /// builders if successful
    ///
    /// # Errors
    ///
    /// Returns a `String` error if:
    /// - The telemetry emitter configuration cannot be built (e.g. an
    ///   environment variable contains an unparseable value).
    /// - `message_spans` or `timer_spans` contains an unrecognized value
    ///   (expected `"child"` or `"follows_from"`).
    /// - The Kafka loader configuration cannot be built (e.g. a tuning value
    ///   fails validation).
    fn try_from(config: &'a NativeConfiguration) -> Result<Self, Self::Error> {
        let mut consumer: ConsumerConfigurationBuilder = config.into();

        if let Some(s) = &config.message_spans {
            let relation = s
                .parse::<SpanRelation>()
                .map_err(|e| format!("message_spans: {e}"))?;
            consumer.message_spans(relation);
        }

        if let Some(s) = &config.timer_spans {
            let relation = s
                .parse::<SpanRelation>()
                .map_err(|e| format!("timer_spans: {e}"))?;
            consumer.timer_spans(relation);
        }

        // The Kafka message loader that the defer middleware uses to reload
        // failed messages is now consumer-wide configuration. Route the
        // defer-loader tuning knobs onto the consumer builder's loader.
        if config.defer_cache_size.is_some()
            || config.defer_seek_timeout.is_some()
            || config.defer_discard_threshold.is_some()
        {
            let mut loader = KafkaLoaderConfiguration::builder();

            if let Some(cache_size) = &config.defer_cache_size {
                loader.cache_size(*cache_size as usize);
            }

            if let Some(seek_timeout) = &config.defer_seek_timeout {
                loader.seek_timeout(Duration::from_secs_f32(*seek_timeout));
            }

            if let Some(discard_threshold) = &config.defer_discard_threshold {
                loader.discard_threshold(*discard_threshold);
            }

            consumer.loader(loader.build().map_err(|e| e.to_string())?);
        }

        Ok(Self {
            consumer,
            retry: config.into(),
            failure_topic: config.into(),
            scheduler: config.into(),
            monopolization: config.into(),
            defer: config.into(),
            timeout: config.into(),
            dedup: config.try_into()?,
            emitter: config.try_into()?,
            keyed_state: build_keyed_state_config(config)?,
        })
    }
}
