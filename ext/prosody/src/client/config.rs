use either::Either;
use magnus::{Error, TryConvert, Value};
use prosody::consumer::ConsumerConfigurationBuilder;
use prosody::consumer::failure::retry::RetryConfigurationBuilder;
use prosody::consumer::failure::topic::FailureTopicConfigurationBuilder;
use prosody::high_level::mode::Mode;
use prosody::producer::ProducerConfigurationBuilder;
use serde::Deserialize;
use serde_magnus::deserialize;
use std::time::Duration;

#[derive(Clone, Debug, Default, Deserialize)]
pub struct NativeConfiguration {
    bootstrap_servers: Option<Vec<String>>,
    mock: Option<bool>,
    send_timeout: Option<f32>,
    group_id: Option<String>,
    idempotence_cache_size: Option<u32>,
    subscribed_topics: Option<Vec<String>>,
    allowed_events: Option<Vec<String>>,
    source_system: Option<String>,
    max_concurrency: Option<u32>,
    max_uncommitted: Option<u16>,
    max_enqueued_per_key: Option<u16>,
    stall_threshold: Option<f32>,
    shutdown_timeout: Option<f32>,
    poll_interval: Option<f32>,
    commit_interval: Option<f32>,
    mode: Option<String>,
    retry_base: Option<f32>,
    max_retries: Option<u32>,
    max_retry_delay: Option<f32>,
    failure_topic: Option<String>,
    probe_port: Option<Either<bool, u16>>,
}

impl TryConvert for NativeConfiguration {
    fn try_convert(val: Value) -> Result<Self, Error> {
        deserialize(val)
    }
}

impl<'a> From<&'a NativeConfiguration> for ProducerConfigurationBuilder {
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
                Either::Left(true) => {}
                Either::Left(false) => {
                    builder.probe_port(None);
                }
                Either::Right(port) => {
                    builder.probe_port(*port);
                }
            }
        }

        builder
    }
}

impl<'a> From<&'a NativeConfiguration> for RetryConfigurationBuilder {
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
