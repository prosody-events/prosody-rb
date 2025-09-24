//! # Admin Client Module
//!
//! Provides administrative capabilities for Kafka through the Prosody library.
//! This module implements Ruby bindings for creating and deleting Kafka topics.

use crate::bridge::Bridge;
use crate::{BRIDGE, ROOT_MOD, id};
use magnus::value::ReprValue;
use magnus::{Error, Module, Object, Ruby, Value, function, method};
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
    #[allow(clippy::needless_pass_by_value)]
    pub fn new(ruby: &Ruby, bootstrap_servers: Vec<String>) -> Result<Self, Error> {
        let admin_config = AdminConfiguration::new(bootstrap_servers)
            .map_err(|error| Error::new(ruby.exception_runtime_error(), error.to_string()))?;

        let client = Arc::new(
            ProsodyAdminClient::new(&admin_config)
                .map_err(|error| Error::new(ruby.exception_runtime_error(), error.to_string()))?,
        );

        let bridge = BRIDGE
            .get()
            .ok_or(Error::new(
                ruby.exception_runtime_error(),
                "Bridge not initialized",
            ))?
            .clone();

        Ok(Self { client, bridge })
    }

    /// Creates a new Kafka topic.
    ///
    /// # Arguments
    ///
    /// * `ruby` - Reference to the Ruby VM
    /// * `this` - The admin client instance
    /// * `name` - Name of the topic to create
    /// * `partition_count` - Number of partitions for the topic
    /// * `replication_factor` - Replication factor for the topic
    ///
    /// # Errors
    ///
    /// Returns a `Magnus::Error` if:
    /// - The topic creation fails
    /// - There's an issue with the asynchronous execution
    pub fn create_topic(
        ruby: &Ruby,
        this: &Self,
        name: String,
        partition_count: u16,
        replication_factor: u16,
    ) -> Result<(), Error> {
        let topic_config = TopicConfiguration::builder()
            .name(name)
            .partition_count(partition_count)
            .replication_factor(replication_factor)
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
        method!(AdminClient::create_topic, 3),
    )?;
    class.define_method(
        id!(ruby, "delete_topic"),
        method!(AdminClient::delete_topic, 1),
    )?;

    // Make the admin client class private
    let _: Value = module.funcall(id!(ruby, "private_constant"), (class_id,))?;

    Ok(())
}
