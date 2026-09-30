//! Conversion of [`NativeConfiguration`] into the Kafka producer, Kafka
//! consumer, Cassandra, and telemetry emitter builders.

use super::{NativeConfiguration, ProbePort};
use prosody::cassandra::config::CassandraConfigurationBuilder;
use prosody::consumer::ConsumerConfigurationBuilder;
use prosody::producer::ProducerConfigurationBuilder;
use prosody::telemetry::emitter::TelemetryEmitterConfiguration;
use std::time::Duration;

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
