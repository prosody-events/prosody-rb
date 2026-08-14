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
use crate::published::{NativePublishedDeque, NativePublishedMap, NativePublishedValue};
use crate::tracing_util::extract_opentelemetry_context;
use crate::util::ensure_runtime_context;
use crate::{BRIDGE, ROOT_MOD, id};
use educe::Educe;
use magnus::value::ReprValue;
use magnus::{
    Class, Error, Module, Object, RClass, RModule, Ruby, StaticSymbol, Value, function, kwargs,
    method,
};
use opentelemetry::propagation::TextMapCompositePropagator;
use prosody::cassandra::config::CassandraConfigurationBuilder;
use prosody::error::ErrorCategory;
use prosody::high_level::ConsumerBuilders;
use prosody::high_level::erased::{
    ErasedConsumerState, ErasedReadCache, SharedHighLevelClient, new_erased,
};
use prosody::high_level::mode::Mode;
use prosody::propagator::new_propagator;
use prosody::requester::ResponseError;
use prosody::subsystem::SubsystemName;
use serde::Deserialize;
use serde_magnus::deserialize;
use serde_magnus::serialize;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tracing::{Span, debug, info_span};
use tracing_opentelemetry::OpenTelemetrySpanExt;

/// Configuration types and conversion between Ruby and Rust representations
mod config;

/// A Ruby-compatible wrapper around the Prosody high-level client.
///
/// This struct bridges Ruby applications with the Prosody messaging system,
/// providing methods for sending messages to Kafka topics and subscribing to
/// events with Ruby handlers.
#[derive(Educe)]
#[educe(Debug)]
#[magnus::wrap(class = "Prosody::Client")]
pub struct Client {
    /// The underlying Prosody client
    #[educe(Debug(ignore))]
    inner: SharedHighLevelClient<RubyHandler>,
    /// Bridge for communicating between Rust and Ruby
    bridge: Bridge,
    /// OpenTelemetry propagator for distributed tracing
    propagator: Arc<TextMapCompositePropagator>,
    /// PID at construction time, used to detect post-fork usage
    pid: u32,
}

#[derive(Deserialize)]
struct NativeRequest {
    topic: String,
    key: String,
    payload: serde_json::Value,
    subsystems: Vec<String>,
    timeout: f64,
    #[serde(default)]
    headers: HashMap<String, String>,
}

impl Client {
    /// Creates a new Prosody client with the given configuration.
    ///
    /// # Arguments
    ///
    /// * `ruby` - The Ruby VM context
    /// * `config` - A Ruby Configuration object or hash containing client
    ///   configuration options
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

        let _guard = ensure_runtime_context(ruby);

        // Check if config is already a Configuration object, if not create one
        let config_class: RClass = ruby
            .get_inner(&ROOT_MOD)
            .const_get(id!(ruby, "Configuration"))?;
        let config_obj = if config.is_kind_of(config_class) {
            config
        } else {
            config_class.funcall(id!(ruby, "new"), (config,))?
        };

        let config_hash: Value = config_obj.funcall(id!(ruby, "to_hash"), ())?;
        let native_config = NativeConfiguration::from_value(ruby, config_hash)?;
        let config_ref = &native_config;

        let mode: Mode = config_ref
            .try_into()
            .map_err(|error: String| Error::new(ruby.exception_arg_error(), error))?;

        let consumer_builders: ConsumerBuilders = config_ref
            .try_into()
            .map_err(|error: String| Error::new(ruby.exception_arg_error(), error))?;

        let bridge = BRIDGE
            .get()
            .ok_or(Error::new(
                ruby.exception_runtime_error(),
                "Bridge not initialized",
            ))?
            .clone();
        let cassandra = Into::<CassandraConfigurationBuilder>::into(config_ref);
        let mut producer = config_ref.into();
        let client = bridge
            .wait_for(
                ruby,
                async move {
                    new_erased(mode, &mut producer, &consumer_builders, &cassandra).await
                },
                Span::current(),
            )?
            .map_err(|error| Error::new(ruby.exception_runtime_error(), error.to_string()))?;

        Ok(Self {
            inner: client,
            bridge,
            propagator: Arc::new(new_propagator()),
            pid: std::process::id(),
        })
    }

    fn check_fork(ruby: &Ruby, this: &Self) -> Result<(), Error> {
        if std::process::id() != this.pid {
            return Err(Error::new(
                ruby.exception_runtime_error(),
                "Prosody::Client cannot be used after fork. Create a new client in the child \
                 process.",
            ));
        }
        Ok(())
    }

    /// Returns the current state of the consumer.
    ///
    /// The consumer can be in one of four states:
    /// - `:shut_down` - The client is shut down
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
    ///
    /// # Errors
    ///
    /// Raises `RuntimeError` if the consumer configuration failed during
    /// build, with the full error message from the underlying
    /// `ModeConfigurationError`.
    pub fn consumer_state(ruby: &Ruby, this: &Self) -> Result<StaticSymbol, Error> {
        Self::check_fork(ruby, this)?;
        let inner = this.inner.clone();
        let state: Result<&'static str, String> = this.bridge.wait_for(
            ruby,
            async move {
                match inner.consumer_state().await {
                    ErasedConsumerState::Shutdown => Ok("shut_down"),
                    ErasedConsumerState::Unconfigured => Ok("unconfigured"),
                    ErasedConsumerState::ConfigurationFailed(error) => {
                        Err(format!("consumer configuration failed: {error}"))
                    }
                    ErasedConsumerState::Configured(_) => Ok("configured"),
                    ErasedConsumerState::Running { .. } => Ok("running"),
                }
            },
            Span::current(),
        )?;

        let state = state.map_err(|msg| Error::new(ruby.exception_runtime_error(), msg))?;

        Ok(ruby.sym_new(state))
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
        Self::check_fork(ruby, this)?;
        let _guard = ensure_runtime_context(ruby);
        let client = this.inner.clone();
        let value = deserialize(ruby, payload)?;
        let context = extract_opentelemetry_context(ruby, &this.propagator)?;

        // Create span for tracing and set parent context
        let span = info_span!("ruby-send", %topic, %key);
        if let Err(err) = span.set_parent(context) {
            debug!("failed to set parent span: {err:#}");
        }

        // Wait for the async send operation to complete
        this.bridge
            .wait_for(
                ruby,
                async move { client.send(topic.as_str().into(), key, value).await },
                span,
            )?
            .map_err(|error| Error::new(ruby.exception_runtime_error(), format!("{error:#}")))
    }

    fn request(ruby: &Ruby, this: &Self, request: Value) -> Result<Value, Error> {
        Self::check_fork(ruby, this)?;
        let _guard = ensure_runtime_context(ruby);
        let request: NativeRequest = deserialize(ruby, request)?;
        let subsystems = request
            .subsystems
            .into_iter()
            .map(SubsystemName::try_new)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| Error::new(ruby.exception_arg_error(), error.to_string()))?;
        let timeout = Duration::try_from_secs_f64(request.timeout).map_err(|_| {
            Error::new(
                ruby.exception_arg_error(),
                "timeout must be a finite, non-negative duration",
            )
        })?;
        let topic = prosody::Topic::from(request.topic.as_str());
        let context = extract_opentelemetry_context(ruby, &this.propagator)?;
        let span = info_span!("ruby-request", topic = %request.topic, key = %request.key);
        if let Err(error) = span.set_parent(context) {
            debug!("failed to set parent span: {error:#}");
        }
        let inner = this.inner.clone();
        let results = this
            .bridge
            .wait_for(
                ruby,
                async move {
                    inner
                        .request(
                            request.headers.into_iter().collect(),
                            topic,
                            request.key,
                            request.payload,
                            subsystems,
                            timeout,
                        )
                        .await
                },
                span,
            )?
            .map_err(|error| Error::new(ruby.exception_runtime_error(), error.to_string()))?;

        let module = ruby.get_inner(&ROOT_MOD);
        let array = ruby.ary_new_capa(results.len());
        for result in results {
            let value = match result {
                Ok(value) => serialize(ruby, &value)?,
                Err(error) => response_error(ruby, module, error)?,
            };
            array.push(value)?;
        }
        Ok(array.as_value())
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
        Self::check_fork(ruby, this)?;
        let _guard = ensure_runtime_context(ruby);
        let wrapper = RubyHandler::new(this.bridge.clone(), ruby, handler)?;
        let inner = this.inner.clone();

        this.bridge
            .wait_for(
                ruby,
                async move { inner.subscribe(wrapper).await },
                Span::current(),
            )?
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
    pub fn assigned_partitions(ruby: &Ruby, this: &Self) -> Result<u32, Error> {
        Self::check_fork(ruby, this)?;
        let inner = this.inner.clone();
        this.bridge.wait_for(
            ruby,
            async move { inner.assigned_partition_count().await },
            Span::current(),
        )
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
    pub fn is_stalled(ruby: &Ruby, this: &Self) -> Result<bool, Error> {
        Self::check_fork(ruby, this)?;
        let inner = this.inner.clone();
        this.bridge.wait_for(
            ruby,
            async move { inner.is_stalled().await },
            Span::current(),
        )
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
        Self::check_fork(ruby, this)?;
        let _guard = ensure_runtime_context(ruby);
        let client = this.inner.clone();

        this.bridge
            .wait_for(
                ruby,
                async move { client.unsubscribe().await },
                Span::current(),
            )?
            .map_err(|error| Error::new(ruby.exception_runtime_error(), format!("{error:#}")))
    }

    /// Shuts down the client and all its services.
    ///
    /// # Errors
    ///
    /// Returns an error if shutdown fails.
    fn shutdown(ruby: &Ruby, this: &Self) -> Result<(), Error> {
        Self::check_fork(ruby, this)?;
        let _guard = ensure_runtime_context(ruby);
        let client = this.inner.clone();

        this.bridge
            .wait_for(
                ruby,
                async move { client.shutdown().await },
                Span::current(),
            )?
            .map_err(|error| Error::new(ruby.exception_runtime_error(), format!("{error:#}")))
    }

    /// Returns the configured source system identifier.
    ///
    /// The source system is used to identify the originating service or
    /// component in produced messages, enabling loop detection.
    ///
    /// # Arguments
    ///
    /// * `this` - The client instance
    ///
    /// # Returns
    ///
    /// The source system identifier.
    fn source_system(this: &Self) -> &str {
        this.inner.source_system()
    }

    fn published_value(
        ruby: &Ruby,
        this: &Self,
        subsystem: String,
        name: String,
        cache_seconds: Option<f64>,
        cache_disabled: bool,
    ) -> Result<NativePublishedValue, Error> {
        Self::check_fork(ruby, this)?;
        let cache = read_cache(ruby, cache_seconds, cache_disabled)?;
        let inner = this.inner.clone();
        let reader = this
            .bridge
            .wait_for(
                ruby,
                async move { inner.value_state(subsystem, name, cache).await },
                Span::current(),
            )?
            .map_err(|error| Error::new(ruby.exception_runtime_error(), error.to_string()))?;
        Ok(NativePublishedValue {
            inner: reader,
            bridge: this.bridge.clone(),
        })
    }

    fn published_map(
        ruby: &Ruby,
        this: &Self,
        subsystem: String,
        name: String,
        cache_seconds: Option<f64>,
        cache_disabled: bool,
    ) -> Result<NativePublishedMap, Error> {
        Self::check_fork(ruby, this)?;
        let cache = read_cache(ruby, cache_seconds, cache_disabled)?;
        let inner = this.inner.clone();
        let reader = this
            .bridge
            .wait_for(
                ruby,
                async move { inner.map_state(subsystem, name, cache).await },
                Span::current(),
            )?
            .map_err(|error| Error::new(ruby.exception_runtime_error(), error.to_string()))?;
        Ok(NativePublishedMap {
            inner: reader,
            bridge: this.bridge.clone(),
            propagator: Arc::clone(&this.propagator),
        })
    }

    fn published_deque(
        ruby: &Ruby,
        this: &Self,
        subsystem: String,
        name: String,
        cache_seconds: Option<f64>,
        cache_disabled: bool,
    ) -> Result<NativePublishedDeque, Error> {
        Self::check_fork(ruby, this)?;
        let cache = read_cache(ruby, cache_seconds, cache_disabled)?;
        let inner = this.inner.clone();
        let reader = this
            .bridge
            .wait_for(
                ruby,
                async move { inner.deque_state(subsystem, name, cache).await },
                Span::current(),
            )?
            .map_err(|error| Error::new(ruby.exception_runtime_error(), error.to_string()))?;
        Ok(NativePublishedDeque {
            inner: reader,
            bridge: this.bridge.clone(),
            propagator: Arc::clone(&this.propagator),
        })
    }
}

fn read_cache(ruby: &Ruby, seconds: Option<f64>, disabled: bool) -> Result<ErasedReadCache, Error> {
    match (seconds, disabled) {
        (None, false) => Ok(ErasedReadCache::Inherit),
        (None, true) => Ok(ErasedReadCache::Disabled),
        (Some(seconds), false) => Duration::try_from_secs_f64(seconds)
            .map(ErasedReadCache::Ttl)
            .map_err(|_| {
                Error::new(
                    ruby.exception_arg_error(),
                    "read_cache must be finite and non-negative",
                )
            }),
        (Some(_), true) => Err(Error::new(
            ruby.exception_arg_error(),
            "read_cache cannot specify a TTL and be disabled",
        )),
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
    let class = module.define_class(id!(ruby, "Client"), ruby.class_object())?;

    class.define_singleton_method("new", function!(Client::new, 1))?;
    class.define_method(
        id!(ruby, "consumer_state"),
        method!(Client::consumer_state, 0),
    )?;
    class.define_method(id!(ruby, "send_message"), method!(Client::send, 3))?;
    class.define_method(id!(ruby, "native_request"), method!(Client::request, 1))?;
    class.define_method(id!(ruby, "subscribe"), method!(Client::subscribe, 1))?;
    class.define_method(
        id!(ruby, "assigned_partitions"),
        method!(Client::assigned_partitions, 0),
    )?;
    class.define_method(id!(ruby, "is_stalled?"), method!(Client::is_stalled, 0))?;
    class.define_method(id!(ruby, "unsubscribe"), method!(Client::unsubscribe, 0))?;
    class.define_method(id!(ruby, "shutdown"), method!(Client::shutdown, 0))?;
    class.define_method(
        id!(ruby, "source_system"),
        method!(Client::source_system, 0),
    )?;
    class.define_method(
        id!(ruby, "published_value"),
        method!(Client::published_value, 4),
    )?;
    class.define_method(
        id!(ruby, "published_map"),
        method!(Client::published_map, 4),
    )?;
    class.define_method(
        id!(ruby, "published_deque"),
        method!(Client::published_deque, 4),
    )?;

    Ok(())
}

fn response_error(ruby: &Ruby, module: RModule, error: ResponseError) -> Result<Value, Error> {
    let display = error.to_string();
    let (name, arguments) = match error {
        ResponseError::Handler {
            category,
            message: handler_message,
        } => {
            let category = match category {
                ErrorCategory::Transient => "transient",
                ErrorCategory::Permanent => "permanent",
                ErrorCategory::Terminal => "terminal",
            };
            let class: RClass = module.const_get(id!(ruby, "HandlerResponseError"))?;
            return class.new_instance((
                kwargs!("category" => ruby.sym_new(category), "handler_message" => handler_message, "message" => display),
            ));
        }
        ResponseError::Timeout => ("ResponseTimeoutError", (display,)),
        ResponseError::FormatMismatch => ("ResponseFormatMismatchError", (display,)),
        ResponseError::Malformed => ("MalformedResponseError", (display,)),
    };
    let class: RClass = module.const_get(name)?;
    class.new_instance(arguments)
}
