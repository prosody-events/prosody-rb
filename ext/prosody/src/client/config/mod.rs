//! Conversion of the Ruby client configuration into Prosody configuration.
//!
//! [`NativeConfiguration`] holds every option the Ruby side can set. This
//! module converts it into [`ConsumerBuilders`] and the processing [`Mode`].
//! The submodules own the other conversions: `connections` for the Kafka,
//! Cassandra, and telemetry builders, `middleware` for the middleware
//! builders, and `state` for keyed state.

use crate::util::seconds;
use magnus::{Error, Ruby, Value};
use prosody::PeerConfiguration;
use prosody::PeerEndpoint;
use prosody::consumer::ConsumerConfigurationBuilder;
use prosody::consumer::SpanRelation;
use prosody::high_level::ConsumerBuilders;
use prosody::high_level::mode::Mode;
use prosody::loader::KafkaLoaderConfiguration;
use serde::{Deserialize, Deserializer};
use serde_magnus::deserialize;
use serde_untagged::UntaggedEnumVisitor;
use state::{ReadCacheConfig, StateCollectionConfig, build_keyed_state_config};
use std::net::SocketAddr;

mod connections;
mod middleware;
mod state;

/// Configuration structure for the Prosody client that maps Ruby configuration
/// values to their native Rust equivalents.
///
/// This structure contains all possible configuration options that can be
/// provided by the Ruby side, which are then converted to the appropriate
/// Prosody configuration builder types.
#[derive(Clone, Default, Deserialize)]
pub struct NativeConfiguration {
    /// List of Kafka bootstrap server addresses
    bootstrap_servers: Option<Vec<String>>,

    /// Whether to use mock mode (for testing)
    mock: Option<bool>,

    /// Maximum time to wait for a send operation to complete (in seconds)
    send_timeout: Option<f64>,

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
    max_uncommitted: Option<u32>,

    /// Threshold in seconds after which a stalled consumer is detected
    stall_threshold: Option<f64>,

    /// Maximum time to wait for a clean shutdown (in seconds)
    shutdown_timeout: Option<f64>,

    /// Interval between Kafka poll operations (in seconds)
    poll_interval: Option<f64>,

    /// Interval between offset commit operations (in seconds)
    commit_interval: Option<f64>,

    /// Interval between librdkafka statistics reports (in seconds)
    statistics_interval: Option<f64>,

    /// Operation mode of the client (`pipeline`, `low_latency`, `best_effort`)
    mode: Option<String>,

    /// Base delay for retry operations (in seconds)
    retry_base: Option<f64>,

    /// Maximum number of retry attempts
    max_retries: Option<u32>,

    /// Maximum delay between retries (in seconds)
    max_retry_delay: Option<f64>,

    /// Topic to send failed messages to
    failure_topic: Option<String>,

    /// Configuration for the health probe port
    probe_port: Option<ProbePort>,

    /// List of Cassandra contact nodes (hostnames or IPs)
    cassandra_nodes: Option<Vec<String>>,

    /// Keyspace used for persistent Prosody data in Cassandra.
    cassandra_keyspace: Option<String>,

    /// Preferred datacenter for Cassandra query routing
    cassandra_datacenter: Option<String>,

    /// Preferred rack identifier for Cassandra topology-aware routing
    cassandra_rack: Option<String>,

    /// Username for authenticating with Cassandra
    cassandra_user: Option<String>,

    /// Password for authenticating with Cassandra
    cassandra_password: Option<String>,

    /// Retention period for persistent timer and deferral data in Cassandra,
    /// in seconds.
    cassandra_retention: Option<f64>,

    /// Timer slab partitioning duration in seconds.
    /// Controls how timers are grouped for storage and retrieval.
    slab_size: Option<f64>,

    // Scheduler configuration
    /// Target proportion of execution time for failure/retry task processing
    /// (0.0 to 1.0). Controls bandwidth allocation between Normal and
    /// Failure task classes.
    scheduler_failure_weight: Option<f64>,

    /// Wait duration (in seconds) at which urgency boost reaches maximum
    /// intensity.
    scheduler_max_wait: Option<f64>,

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
    monopolization_window: Option<f64>,

    /// Cache size for tracking key execution intervals.
    monopolization_cache_size: Option<u32>,

    // Defer configuration
    /// Whether deferral is enabled for new messages.
    defer_enabled: Option<bool>,

    /// Base exponential backoff delay for deferred retries (in seconds).
    defer_base: Option<f64>,

    /// Maximum delay between deferred retries (in seconds).
    defer_max_delay: Option<f64>,

    /// Failure rate threshold for disabling deferral (0.0 to 1.0).
    defer_failure_threshold: Option<f64>,

    /// Sliding window duration (in seconds) for failure rate tracking.
    defer_failure_window: Option<f64>,

    /// Maximum messages retained by the shared Kafka loader.
    loader_cache_size: Option<u32>,

    /// Maximum number of deferred store entries kept in the write-through cache
    /// per Cassandra defer store.
    defer_store_cache_size: Option<u32>,

    /// Timeout for Kafka loader seek operations (in seconds).
    loader_seek_timeout: Option<f64>,

    /// Messages to read sequentially before seeking.
    loader_discard_threshold: Option<i64>,

    // Timeout configuration
    /// Fixed timeout duration for handler execution (in seconds).
    timeout: Option<f64>,

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

    /// Address for the peer listener.
    peer_bind_address: Option<String>,

    /// gRPC connect URI that peers use for this client.
    peer_advertised_connect: Option<String>,

    /// Network name used to identify direct routes.
    peer_network_name: Option<String>,

    /// Maximum number of peer channels and registrations held in each cache.
    peer_cache_capacity: Option<usize>,

    /// Duration of each peer registration lease, in seconds.
    peer_registration_ttl: Option<f64>,

    // Keyed-state configuration
    /// Keyed-state collections to register before subscribe.
    state_collections: Option<Vec<StateCollectionConfig>>,

    /// Subsystem under which published collections are advertised.
    subsystem: Option<String>,

    /// Directory for the local keyed-state caches. Each consumer uses a new
    /// subdirectory, so clients can share it. Must not be empty.
    state_cache_dir: Option<String>,

    /// Capacity of the owning keyed-state cache.
    state_owned_cache_size: Option<String>,

    /// Size at which the local keyed-state cache flushes a partition's
    /// in-memory writes to disk. `None` uses `PROSODY_STATE_MEMTABLE_SIZE`,
    /// or the engine default of 64 MiB when that is unset.
    state_memtable_size: Option<String>,

    /// Capacity of the published-state read-through cache.
    state_read_cache_size: Option<String>,

    /// Default cache policy for published-state reads.
    state_read_cache: Option<ReadCacheConfig>,
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
        let mut consumer: ConsumerConfigurationBuilder = config.try_into()?;

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

        // Route the shared Kafka message loader settings onto the consumer.
        if config.loader_cache_size.is_some()
            || config.loader_seek_timeout.is_some()
            || config.loader_discard_threshold.is_some()
        {
            let mut loader = KafkaLoaderConfiguration::builder();

            if let Some(cache_size) = &config.loader_cache_size {
                loader.cache_size(*cache_size as usize);
            }

            if let Some(seek_timeout) = &config.loader_seek_timeout {
                loader.seek_timeout(seconds("loader_seek_timeout", *seek_timeout)?);
            }

            if let Some(discard_threshold) = &config.loader_discard_threshold {
                loader.discard_threshold(*discard_threshold);
            }

            consumer.loader(loader.build().map_err(|e| e.to_string())?);
        }

        Ok(Self {
            consumer,
            retry: config.try_into()?,
            failure_topic: config.into(),
            scheduler: config.try_into()?,
            monopolization: config.try_into()?,
            defer: config.try_into()?,
            timeout: config.try_into()?,
            dedup: config.try_into()?,
            emitter: config.try_into()?,
            keyed_state: build_keyed_state_config(config)?,
            peer: build_peer_config(config)?,
        })
    }
}

fn build_peer_config(config: &NativeConfiguration) -> Result<PeerConfiguration, String> {
    let mut builder = PeerConfiguration::builder();
    if let Some(value) = &config.peer_bind_address {
        builder.bind_address(
            value
                .parse::<SocketAddr>()
                .map_err(|error| format!("peer_bind_address: {error}"))?,
        );
    }
    if let Some(value) = &config.peer_advertised_connect {
        builder.advertised_connect(
            PeerEndpoint::try_from(value.clone())
                .map_err(|error| format!("peer_advertised_connect: {error}"))?,
        );
    }
    if let Some(value) = &config.peer_network_name {
        builder.network_name(value.clone());
    }
    if let Some(value) = config.peer_cache_capacity {
        builder.peer_cache_capacity(value);
    }
    if let Some(value) = config.peer_registration_ttl {
        builder.registration_ttl(seconds("peer_registration_ttl", value)?);
    }
    builder.build().map_err(|error| error.to_string())
}
