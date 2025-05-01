//! Ruby wrapper for the Prosody message context.
//!
//! This module provides a Ruby-compatible wrapper around the Prosody library's
//! `MessageContext`, allowing Ruby code to interact with message context
//! information from Kafka messages.

use crate::{ROOT_MOD, id};
use educe::Educe;
use magnus::{Error, Module, Ruby};
use prosody::consumer::message::MessageContext;

/// Ruby wrapper for a Kafka message context.
///
/// This struct wraps the native Prosody `MessageContext` and exposes it to Ruby
/// code as the `Prosody::Context` class. The context contains metadata and
/// control capabilities related to the processing of a Kafka message.
#[derive(Educe, Clone)]
#[educe(Debug)]
#[magnus::wrap(class = "Prosody::Context", free_immediately, frozen_shareable, size)]
pub struct Context {
    /// The underlying Prosody message context.
    ///
    /// This field is marked as hidden in debug output to prevent logging large
    /// data.
    #[allow(dead_code)]
    #[educe(Debug(ignore))]
    inner: MessageContext,
}

impl From<MessageContext> for Context {
    /// Creates a new `Context` from a Prosody `MessageContext`.
    ///
    /// # Arguments
    ///
    /// * `value` - The Prosody message context to wrap
    fn from(value: MessageContext) -> Self {
        Self { inner: value }
    }
}

/// Initializes the Context class in Ruby.
///
/// Registers the `Prosody::Context` class in the Ruby runtime, making it
/// available to Ruby code.
///
/// # Arguments
///
/// * `ruby` - Reference to the Ruby VM
///
/// # Errors
///
/// Returns a Magnus error if the class definition fails
pub fn init(ruby: &Ruby) -> Result<(), Error> {
    let module = ruby.get_inner(&ROOT_MOD);
    module.define_class(id!("Context"), ruby.class_object())?;

    Ok(())
}
