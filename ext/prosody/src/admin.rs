//! # Admin Client Module
//!
//! Provides administrative capabilities for Kafka through the Prosody library.
//! This module implements Ruby bindings for creating and deleting Kafka topics.

use crate::bridge::Bridge;
use crate::util::{ForkGuard, ensure_runtime_context, seconds};
use crate::{ROOT_MOD, id};
use magnus::scan_args::{get_kwargs, scan_args};
use magnus::{Error, Module, Object, RHash, Ruby, Value, function, method};
use prosody::admin::{AdminConfiguration, ProsodyAdminClient, TopicConfiguration};
use std::sync::Arc;
use tracing::Span;

/// Ruby wrapper for the Prosody admin client.
///
/// This struct provides administrative operations for Kafka topics, such as
/// creating and deleting topics. It wraps the Rust `ProsodyAdminClient` and
/// uses the bridge mechanism to handle asynchronous operations from Ruby.
#[magnus::wrap(class = "Prosody::AdminClient")]
pub struct AdminClient {
    /// The underlying Prosody admin client
    client: Arc<ProsodyAdminClient>,
    /// Bridge for executing asynchronous operations from Ruby
    bridge: Bridge,
    /// Refuses use in a forked child process
    fork: ForkGuard,
}

impl AdminClient {
    /// Creates a new admin client with the specified bootstrap servers.
    ///
    /// # Arguments
    ///
    /// * `ruby` - Reference to the Ruby VM
    /// * `bootstrap_servers` - List of Kafka bootstrap server addresses
    ///
    /// # Errors
    ///
    /// Returns a `Magnus::Error` if:
    /// - The client cannot be created with the provided bootstrap servers
    /// - The bridge is not initialized
    pub fn new(ruby: &Ruby, bootstrap_servers: Vec<String>) -> Result<Self, Error> {
        let _guard = ensure_runtime_context(ruby);
        let admin_config = AdminConfiguration::new(bootstrap_servers)
            .map_err(|error| Error::new(ruby.exception_runtime_error(), error.to_string()))?;

        let client = Arc::new(
            ProsodyAdminClient::new(&admin_config)
                .map_err(|error| Error::new(ruby.exception_runtime_error(), error.to_string()))?,
        );

        let bridge = crate::BRIDGE
            .get()
            .ok_or(Error::new(
                ruby.exception_runtime_error(),
                "Bridge not initialized",
            ))?
            .clone();

        Ok(Self {
            client,
            bridge,
            fork: ForkGuard::new("Prosody::AdminClient"),
        })
    }

    /// Creates a new Kafka topic.
    ///
    /// Ruby calls it as `create_topic(name, partition_count,
    /// replication_factor, cleanup_policy: nil, retention: nil)`. A `nil`
    /// keyword uses the cluster default. `retention` is in seconds.
    ///
    /// # Errors
    ///
    /// Returns a `Magnus::Error` if:
    /// - An argument has the wrong type, or `retention` has no `Duration` form
    /// - The topic creation fails
    /// - There's an issue with the asynchronous execution
    pub fn create_topic(ruby: &Ruby, this: &Self, args: &[Value]) -> Result<(), Error> {
        this.fork.check(ruby)?;
        let args = scan_args::<(String, u16, u16), (), (), (), RHash, ()>(args)?;
        let (name, partition_count, replication_factor) = args.required;
        let keywords = get_kwargs::<_, (), (Option<Option<String>>, Option<Option<f64>>), ()>(
            args.keywords,
            &[],
            &["cleanup_policy", "retention"],
        )?;
        let (cleanup_policy, retention) = keywords.optional;

        let mut builder = TopicConfiguration::builder();
        builder
            .name(name)
            .partition_count(partition_count)
            .replication_factor(replication_factor);
        if let Some(cleanup_policy) = cleanup_policy.flatten() {
            builder.cleanup_policy(cleanup_policy);
        }
        if let Some(retention) = retention.flatten() {
            builder.retention(
                seconds("retention", retention)
                    .map_err(|error| Error::new(ruby.exception_arg_error(), error))?,
            );
        }
        let topic_config = builder
            .build()
            .map_err(|error| Error::new(ruby.exception_runtime_error(), error.to_string()))?;

        let client = this.client.clone();
        let future = async move { client.create_topic(&topic_config).await };

        this.bridge
            .wait_for(ruby, future, Span::current())?
            .map_err(|error| Error::new(ruby.exception_runtime_error(), error.to_string()))
    }

    /// Deletes a Kafka topic.
    ///
    /// # Arguments
    ///
    /// * `ruby` - Reference to the Ruby VM
    /// * `this` - The admin client instance
    /// * `name` - Name of the topic to delete
    ///
    /// # Errors
    ///
    /// Returns a `Magnus::Error` if:
    /// - The topic deletion fails
    /// - There's an issue with the asynchronous execution
    pub fn delete_topic(ruby: &Ruby, this: &Self, name: String) -> Result<(), Error> {
        this.fork.check(ruby)?;
        let client = this.client.clone();
        let future = async move { client.delete_topic(&name).await };

        this.bridge
            .wait_for(ruby, future, Span::current())?
            .map_err(|error| Error::new(ruby.exception_runtime_error(), error.to_string()))
    }
}

/// Initializes the admin module by registering the `Prosody::AdminClient`
/// class.
///
/// # Arguments
///
/// * `ruby` - Reference to the Ruby VM
///
/// # Errors
///
/// Returns a `Magnus::Error` if class or method definition fails
pub fn init(ruby: &Ruby) -> Result<(), Error> {
    let module = ruby.get_inner(&ROOT_MOD);
    let class_id = id!(ruby, "AdminClient");
    let class = module.define_class(class_id, ruby.class_object())?;

    class.define_singleton_method("new", function!(AdminClient::new, 1))?;
    class.define_method(
        id!(ruby, "create_topic"),
        method!(AdminClient::create_topic, -1),
    )?;
    class.define_method(
        id!(ruby, "delete_topic"),
        method!(AdminClient::delete_topic, 1),
    )?;

    Ok(())
}
