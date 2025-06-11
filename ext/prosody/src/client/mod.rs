//! # Client Module
//!
//! Provides the Ruby interface to the Prosody messaging system. This module
//! defines the `Client` class that allows Ruby applications to send messages
//! and process events from Kafka topics using the Prosody library.
//!
//! The client supports:
//! - Sending messages to Kafka topics
//! - Subscribing to topics with Ruby handler objects
//! - OpenTelemetry context propagation for distributed tracing
//! - Different operation modes (`Pipeline`, `LowLatency`, `BestEffort`)

use crate::bridge::Bridge;
use crate::client::config::NativeConfiguration;
use crate::handler::RubyHandler;
use crate::{BRIDGE, ROOT_MOD, RUNTIME, id};
use magnus::value::ReprValue;
use magnus::{Error, Module, Object, RClass, RHash, RModule, Ruby, StaticSymbol, Value, function, method};
use opentelemetry::propagation::{TextMapCompositePropagator, TextMapPropagator};
use prosody::high_level::HighLevelClient;
use prosody::high_level::mode::Mode;
use prosody::high_level::state::ConsumerState;
use prosody::propagator::new_propagator;
use serde_magnus::deserialize;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{Instrument, info_span};
use tracing_opentelemetry::OpenTelemetrySpanExt;

/// Configuration types and conversion between Ruby and Rust representations
mod config;

/// A Ruby-compatible wrapper around the Prosody high-level client.
///
/// This struct bridges Ruby applications with the Prosody messaging system,
/// providing methods for sending messages to Kafka topics and subscribing to
/// events with Ruby handlers.
#[derive(Debug)]
#[magnus::wrap(class = "Prosody::Client", free_immediately, frozen_shareable, size)]
pub struct Client {
    /// The underlying Prosody client
    inner: Arc<HighLevelClient<RubyHandler>>,
    /// Bridge for communicating between Rust and Ruby
    bridge: Bridge,
    /// OpenTelemetry propagator for distributed tracing
    propagator: TextMapCompositePropagator,
}

impl Client {
    /// Creates a new Prosody client with the given configuration.
    ///
    /// # Arguments
    ///
    /// * `ruby` - The Ruby VM context
    /// * `config` - A Ruby Configuration object or hash containing client configuration options
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The OpenTelemetry API gem cannot be loaded
    /// - The configuration mode is invalid
    /// - The client cannot be initialized with the given configuration
    /// - The bridge is not initialized
    fn new(ruby: &Ruby, config: Value) -> Result<Self, Error> {
        ruby.require("opentelemetry-api")?;

        let _guard = RUNTIME.enter();
        
        // Check if config is already a Configuration object, if not create one
        let config_class: RClass = ruby.get_inner(&ROOT_MOD).const_get(id!("Configuration"))?;
        let config_obj = if config.is_kind_of(config_class) {
            config
        } else {
            config_class.funcall(id!("new"), (config,))?
        };
        
        let native_config: NativeConfiguration = config_obj.funcall(id!("to_hash"), ())?;
        let config_ref = &native_config;

        let mode: Mode = config_ref
            .try_into()
            .map_err(|error: String| Error::new(ruby.exception_arg_error(), error))?;

        let client = HighLevelClient::new(
            mode,
            &mut config_ref.into(),
            &config_ref.into(),
            &config_ref.into(),
            &config_ref.into(),
        )
        .map_err(|error| Error::new(ruby.exception_runtime_error(), error.to_string()))?;

        let bridge = BRIDGE
            .get()
            .ok_or(Error::new(
                ruby.exception_runtime_error(),
                "Bridge not initialized",
            ))?
            .clone();

        Ok(Self {
            inner: Arc::new(client),
            bridge: bridge.clone(),
            propagator: new_propagator(),
        })
    }

    /// Returns the current state of the consumer.
    ///
    /// The consumer can be in one of three states:
    /// - `:unconfigured` - The consumer has not been configured yet
    /// - `:configured` - The consumer is configured but not running
    /// - `:running` - The consumer is actively consuming messages
    ///
    /// # Arguments
    ///
    /// * `ruby` - The Ruby VM context
    /// * `this` - The client instance
    ///
    /// # Returns
    ///
    /// A Ruby symbol representing the current consumer state.
    pub fn consumer_state(ruby: &Ruby, this: &Self) -> StaticSymbol {
        ruby.sym_new(match &*this.inner.consumer_state() {
            ConsumerState::Unconfigured => id!("unconfigured"),
            ConsumerState::Configured(_) => id!("configured"),
            ConsumerState::Running { .. } => id!("running"),
        })
    }

    /// Sends a message to the specified Kafka topic.
    ///
    /// # Arguments
    ///
    /// * `ruby` - The Ruby VM context
    /// * `this` - The client instance
    /// * `topic` - The destination topic name
    /// * `key` - The message key for partitioning
    /// * `payload` - The message payload (will be serialized)
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The payload cannot be deserialized
    /// - OpenTelemetry context extraction fails
    /// - The message cannot be sent to Kafka
    fn send(
        ruby: &Ruby,
        this: &Self,
        topic: String,
        key: String,
        payload: Value,
    ) -> Result<(), Error> {
        let _guard = RUNTIME.enter();
        let client = this.inner.clone();
        let value = deserialize(payload)?;

        // Extract OpenTelemetry context from Ruby for distributed tracing
        let carrier = RHash::new();
        let otel_class: RModule = ruby.class_module().const_get(id!("OpenTelemetry"))?;
        let propagator: Value = otel_class.funcall(id!("propagation"), ())?;
        let _: Value = propagator.funcall(id!("inject"), (carrier,))?;

        let carrier: HashMap<String, String> = carrier.to_hash_map()?;
        let context = this.propagator.extract(&carrier);

        // Create span for tracing and set parent context
        let span = info_span!("ruby-send", %topic, %key);
        span.set_parent(context);

        // Wait for the async send operation to complete
        this.bridge
            .wait_for(ruby, async move {
                client
                    .send(topic.as_str().into(), &key, &value)
                    .instrument(span)
                    .await
            })?
            .map_err(|e| Error::new(ruby.exception_runtime_error(), e.to_string()))
    }

    /// Subscribes to events using the provided Ruby handler.
    ///
    /// The handler must implement an `on_message(context, message)` method
    /// that will be called for each received message.
    ///
    /// # Arguments
    ///
    /// * `ruby` - The Ruby VM context
    /// * `this` - The client instance
    /// * `handler` - A Ruby object that will handle incoming messages
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The handler cannot be wrapped
    /// - The client cannot subscribe with the handler
    fn subscribe(ruby: &Ruby, this: &Self, handler: Value) -> Result<(), Error> {
        let _guard = RUNTIME.enter();
        let wrapper = RubyHandler::new(this.bridge.clone(), ruby, handler)?;
        this.inner
            .subscribe(wrapper)
            .map_err(|error| Error::new(ruby.exception_runtime_error(), error.to_string()))?;

        Ok(())
    }

    /// Returns the number of Kafka partitions currently assigned to this
    /// consumer.
    ///
    /// This method can be used to monitor the consumer's workload and ensure
    /// proper load balancing across multiple consumer instances.
    ///
    /// # Arguments
    ///
    /// * `self` - The client instance
    ///
    /// # Returns
    ///
    /// The number of assigned partitions as a u32.
    pub fn assigned_partitions(&self) -> u32 {
        self.inner.assigned_partition_count()
    }

    /// Checks if the consumer is stalled.
    ///
    /// A stalled consumer is one that has stopped processing messages due to
    /// errors or reaching processing limits. This can be used to detect
    /// unhealthy consumers that need attention.
    ///
    /// # Arguments
    ///
    /// * `self` - The client instance
    ///
    /// # Returns
    ///
    /// `true` if the consumer is stalled, `false` otherwise.
    pub fn is_stalled(&self) -> bool {
        self.inner.is_stalled()
    }

    /// Unsubscribes from all topics, stopping message processing.
    ///
    /// This method gracefully shuts down the consumer, completing any in-flight
    /// messages before stopping.
    ///
    /// # Arguments
    ///
    /// * `ruby` - The Ruby VM context
    /// * `this` - The client instance
    ///
    /// # Errors
    ///
    /// Returns an error if the unsubscribe operation fails.
    fn unsubscribe(ruby: &Ruby, this: &Self) -> Result<(), Error> {
        let _guard = RUNTIME.enter();
        let client = this.inner.clone();

        this.bridge
            .wait_for(ruby, async move { client.unsubscribe().await })?
            .map_err(|e| Error::new(ruby.exception_runtime_error(), e.to_string()))
    }
}

/// Initializes the client module in Ruby.
///
/// Defines the `Prosody::Client` class and its methods, making the client
/// functionality available to Ruby code.
///
/// # Arguments
///
/// * `ruby` - The Ruby VM context
///
/// # Errors
///
/// Returns an error if Ruby class or method definition fails.
pub fn init(ruby: &Ruby) -> Result<(), Error> {
    let module = ruby.get_inner(&ROOT_MOD);
    let class = module.define_class(id!("Client"), ruby.class_object())?;

    class.define_singleton_method("new", function!(Client::new, 1))?;
    class.define_method(id!("consumer_state"), method!(Client::consumer_state, 0))?;
    class.define_method(id!("send_message"), method!(Client::send, 3))?;
    class.define_method(id!("subscribe"), method!(Client::subscribe, 1))?;
    class.define_method(
        id!("assigned_partitions"),
        method!(Client::assigned_partitions, 0),
    )?;
    class.define_method(id!("is_stalled?"), method!(Client::is_stalled, 0))?;
    class.define_method(id!("unsubscribe"), method!(Client::unsubscribe, 0))?;

    Ok(())
}
