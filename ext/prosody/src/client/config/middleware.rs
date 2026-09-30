//! Conversion of [`NativeConfiguration`] into the retry, failure topic,
//! scheduler, monopolization, defer, timeout, and deduplication middleware
//! builders.

use super::NativeConfiguration;
use crate::util::seconds;
use prosody::consumer::middleware::deduplication::DeduplicationConfigurationBuilder;
use prosody::consumer::middleware::defer::DeferConfigurationBuilder;
use prosody::consumer::middleware::monopolization::MonopolizationConfigurationBuilder;
use prosody::consumer::middleware::retry::RetryConfigurationBuilder;
use prosody::consumer::middleware::scheduler::SchedulerConfigurationBuilder;
use prosody::consumer::middleware::timeout::TimeoutConfigurationBuilder;
use prosody::consumer::middleware::topic::FailureTopicConfigurationBuilder;
use std::num::NonZeroUsize;

impl<'a> TryFrom<&'a NativeConfiguration> for RetryConfigurationBuilder {
    type Error = String;

    /// Attempts to convert a `NativeConfiguration` reference into a
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
    /// A configured `RetryConfigurationBuilder` if successful
    ///
    /// # Errors
    ///
    /// Returns a `String` error if a duration is negative, not finite, or too
    /// large.
    fn try_from(config: &'a NativeConfiguration) -> Result<Self, Self::Error> {
        let mut builder = Self::default();

        if let Some(retry_base) = &config.retry_base {
            builder.base(seconds("retry_base", *retry_base)?);
        }

        if let Some(max_retries) = &config.max_retries {
            builder.max_retries(*max_retries);
        }

        if let Some(max_retry_delay) = &config.max_retry_delay {
            builder.max_delay(seconds("max_retry_delay", *max_retry_delay)?);
        }

        Ok(builder)
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

impl<'a> TryFrom<&'a NativeConfiguration> for SchedulerConfigurationBuilder {
    type Error = String;

    /// Attempts to convert a `NativeConfiguration` reference into a
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
    /// A configured `SchedulerConfigurationBuilder` if successful
    ///
    /// # Errors
    ///
    /// Returns a `String` error if a duration is negative, not finite, or too
    /// large.
    fn try_from(config: &'a NativeConfiguration) -> Result<Self, Self::Error> {
        let mut builder = Self::default();

        if let Some(max_concurrency) = &config.max_concurrency {
            builder.max_concurrency(*max_concurrency as usize);
        }

        if let Some(failure_weight) = &config.scheduler_failure_weight {
            builder.failure_weight(*failure_weight);
        }

        if let Some(max_wait) = &config.scheduler_max_wait {
            builder.max_wait(seconds("scheduler_max_wait", *max_wait)?);
        }

        if let Some(wait_weight) = &config.scheduler_wait_weight {
            builder.wait_weight(*wait_weight);
        }

        if let Some(cache_size) = &config.scheduler_cache_size {
            builder.cache_size(*cache_size as usize);
        }

        Ok(builder)
    }
}

impl<'a> TryFrom<&'a NativeConfiguration> for MonopolizationConfigurationBuilder {
    type Error = String;

    /// Attempts to convert a `NativeConfiguration` reference into a
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
    /// A configured `MonopolizationConfigurationBuilder` if successful
    ///
    /// # Errors
    ///
    /// Returns a `String` error if a duration is negative, not finite, or too
    /// large.
    fn try_from(config: &'a NativeConfiguration) -> Result<Self, Self::Error> {
        let mut builder = Self::default();

        if let Some(enabled) = &config.monopolization_enabled {
            builder.enabled(*enabled);
        }

        if let Some(threshold) = &config.monopolization_threshold {
            builder.monopolization_threshold(*threshold);
        }

        if let Some(window) = &config.monopolization_window {
            builder.window_duration(seconds("monopolization_window", *window)?);
        }

        if let Some(cache_size) = &config.monopolization_cache_size {
            builder.cache_size(*cache_size as usize);
        }

        Ok(builder)
    }
}

impl<'a> TryFrom<&'a NativeConfiguration> for DeferConfigurationBuilder {
    type Error = String;

    /// Attempts to convert a `NativeConfiguration` reference into a
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
    /// A configured `DeferConfigurationBuilder` if successful
    ///
    /// # Errors
    ///
    /// Returns a `String` error if a duration is negative, not finite, or too
    /// large.
    fn try_from(config: &'a NativeConfiguration) -> Result<Self, Self::Error> {
        let mut builder = Self::default();

        if let Some(enabled) = &config.defer_enabled {
            builder.enabled(*enabled);
        }

        if let Some(base) = &config.defer_base {
            builder.base(seconds("defer_base", *base)?);
        }

        if let Some(max_delay) = &config.defer_max_delay {
            builder.max_delay(seconds("defer_max_delay", *max_delay)?);
        }

        if let Some(failure_threshold) = &config.defer_failure_threshold {
            builder.failure_threshold(*failure_threshold);
        }

        if let Some(failure_window) = &config.defer_failure_window {
            builder.failure_window(seconds("defer_failure_window", *failure_window)?);
        }

        if let Some(store_cache_size) = &config.defer_store_cache_size {
            builder.store_cache_size(*store_cache_size as usize);
        }

        Ok(builder)
    }
}

impl<'a> TryFrom<&'a NativeConfiguration> for TimeoutConfigurationBuilder {
    type Error = String;

    /// Attempts to convert a `NativeConfiguration` reference into a
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
    /// A configured `TimeoutConfigurationBuilder` if successful
    ///
    /// # Errors
    ///
    /// Returns a `String` error if a duration is negative, not finite, or too
    /// large.
    fn try_from(config: &'a NativeConfiguration) -> Result<Self, Self::Error> {
        let mut builder = Self::default();

        if let Some(timeout) = &config.timeout {
            builder.timeout(Some(seconds("timeout", *timeout)?));
        }

        Ok(builder)
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
    /// Core deduplication is mandatory, so its cache capacity is a
    /// `NonZeroUsize`. An `idempotence_cache_size` of `0` cannot convert to
    /// that type, so the conversion fails.
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
    /// Returns a `String` error if `idempotence_cache_size` is `0`, or if
    /// `idempotence_ttl` is negative, not finite, or too large.
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

        if let Some(ttl) = &config.idempotence_ttl {
            builder.ttl(seconds("idempotence_ttl", *ttl)?);
        }

        Ok(builder)
    }
}
