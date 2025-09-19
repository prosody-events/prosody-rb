//! Defines the Ruby-facing wrapper for Kafka consumer messages.
//!
//! This module implements a Ruby-accessible wrapper around the Prosody consumer
//! message type, exposing message properties and content as Ruby objects.

use crate::{ROOT_MOD, id};
use educe::Educe;
use magnus::value::ReprValue;
use magnus::{Error, Module, RClass, Ruby, Value, method};
use prosody::consumer::Keyed;
use prosody::consumer::message::ConsumerMessage;
use serde_magnus::serialize;

/// Ruby-accessible wrapper for Kafka consumer messages.
///
/// Provides Ruby bindings to access Kafka message data including topic,
/// partition, offset, key, timestamp, and payload.
#[derive(Educe, Clone)]
#[educe(Debug)]
#[magnus::wrap(class = "Prosody::Message", frozen_shareable)]
pub struct Message {
    /// The wrapped Prosody consumer message
    #[educe(Debug(ignore))]
    inner: ConsumerMessage,
}

impl Message {
    /// Returns the topic name this message was published to.
    ///
    /// # Returns
    ///
    /// A string slice containing the topic name.
    fn topic(&self) -> &'static str {
        self.inner.topic().as_ref()
    }

    /// Returns the Kafka partition number for this message.
    ///
    /// # Returns
    ///
    /// The partition number as an i32.
    fn partition(&self) -> i32 {
        self.inner.partition()
    }

    /// Returns the Kafka offset for this message within its partition.
    ///
    /// # Returns
    ///
    /// The message offset as an i64.
    fn offset(&self) -> i64 {
        self.inner.offset()
    }

    /// Returns the message key.
    ///
    /// # Returns
    ///
    /// A string slice containing the message key.
    fn key(&self) -> &str {
        self.inner.key()
    }

    /// Converts the message timestamp to a Ruby Time object.
    ///
    /// # Arguments
    ///
    /// * `ruby` - Reference to the Ruby VM
    /// * `this` - Reference to the Message instance
    ///
    /// # Returns
    ///
    /// A Ruby Time object representing the message timestamp.
    ///
    /// # Errors
    ///
    /// Returns an error if the Ruby Time class cannot be accessed or if
    /// creating the Time object fails.
    fn timestamp(ruby: &Ruby, this: &Self) -> Result<Value, Error> {
        let epoch_micros = this.inner.timestamp().timestamp_micros();
        ruby.module_kernel()
            .const_get::<_, RClass>(id!(ruby, "Time"))?
            .funcall(
                id!(ruby, "at"),
                (epoch_micros, ruby.to_symbol("microsecond")),
            )
    }

    /// Deserializes the message payload to a Ruby value.
    ///
    /// # Arguments
    ///
    /// * `_ruby` - Reference to the Ruby VM (unused but required by Magnus)
    /// * `this` - Reference to the Message instance
    ///
    /// # Returns
    ///
    /// A Ruby value containing the deserialized message payload.
    ///
    /// # Errors
    ///
    /// Returns an error if payload deserialization fails.
    fn payload(_ruby: &Ruby, this: &Self) -> Result<Value, Error> {
        serialize(this.inner.payload())
    }
}

impl From<ConsumerMessage> for Message {
    /// Creates a new Message wrapper from a Prosody `ConsumerMessage`.
    ///
    /// # Arguments
    ///
    /// * `value` - The Prosody `ConsumerMessage` to wrap
    fn from(value: ConsumerMessage) -> Self {
        Self { inner: value }
    }
}

/// Initializes the `Prosody::Message` Ruby class and defines its methods.
///
/// # Arguments
///
/// * `ruby` - Reference to the Ruby VM
///
/// # Returns
///
/// OK on successful initialization.
///
/// # Errors
///
/// Returns an error if class or method definition fails.
pub fn init(ruby: &Ruby) -> Result<(), Error> {
    let module = ruby.get_inner(&ROOT_MOD);
    let class = module.define_class(id!(ruby, "Message"), ruby.class_object())?;

    class.define_method(id!(ruby, "topic"), method!(Message::topic, 0))?;
    class.define_method(id!(ruby, "partition"), method!(Message::partition, 0))?;
    class.define_method(id!(ruby, "offset"), method!(Message::offset, 0))?;
    class.define_method(id!(ruby, "key"), method!(Message::key, 0))?;
    class.define_method(id!(ruby, "timestamp"), method!(Message::timestamp, 0))?;
    class.define_method(id!(ruby, "payload"), method!(Message::payload, 0))?;

    Ok(())
}
