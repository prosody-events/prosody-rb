//! # Configuration Module for Prosody Client
//!
//! This module handles the conversion between Ruby configuration objects and
//! the native Rust configuration structures needed by the Prosody library.
//! It defines serialization/deserialization logic and conversion traits that
//! transform Ruby configuration values into appropriate Prosody configuration
//! builders.

use magnus::{Error, TryConvert, Value};
use prosody::consumer::ConsumerConfigurationBuilder;
use prosody::consumer::failure::retry::RetryConfigurationBuilder;
use prosody::consumer::failure::topic::FailureTopicConfigurationBuilder;
use prosody::high_level::mode::Mode;
use prosody::producer::ProducerConfigurationBuilder;
use prosody::timers::store::cassandra::CassandraConfigurationBuilder;
use serde::{Deserialize, Deserializer};
use serde_magnus::deserialize;
use serde_untagged::UntaggedEnumVisitor;
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

    /// Size of the cache used for message idempotence
    idempotence_cache_size: Option<u32>,

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

    /// Maximum number of messages to enqueue for a single key
    max_enqueued_per_key: Option<u16>,

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

    /// Retention period for failed/unprocessed timer data in Cassandra (in seconds)
    cassandra_retention: Option<f32>,
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

impl TryConvert for NativeConfiguration {
    /// Attempts to convert a Ruby Value into a `NativeConfiguration`.
    ///
    /// # Arguments
    ///
    /// * `val` - The Ruby value to convert
    ///
    /// # Returns
    ///
    /// The converted `NativeConfiguration` if successful
    ///
    /// # Errors
    ///
    /// Returns a Magnus error if deserialization fails
    fn try_convert(val: Value) -> Result<Self, Error> {
        deserialize(val)
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

        if let Some(max_concurrency) = &config.max_concurrency {
            builder.max_concurrency(*max_concurrency as usize);
        }

        if let Some(max_uncommitted) = &config.max_uncommitted {
            builder.max_uncommitted(*max_uncommitted as usize);
        }

        if let Some(max_enqueued_per_key) = &config.max_enqueued_per_key {
            builder.max_enqueued_per_key(*max_enqueued_per_key as usize);
        }

        if let Some(idempotence_cache_size) = &config.idempotence_cache_size {
            builder.idempotence_cache_size(*idempotence_cache_size as usize);
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
