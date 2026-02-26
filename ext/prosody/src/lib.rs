//! # Prosody Ruby Extension
//!
//! This crate provides Ruby bindings for the Prosody event processing library.
//! It bridges the Rust implementation of Prosody with Ruby, allowing Ruby
//! applications to use Prosody for event processing and messaging.
//!
//! The extension handles asynchronous communication between Rust and Ruby,
//! provides client functionality for interacting with message brokers,
//! manages message handling, and includes logging and scheduling capabilities.

// Temporarily removing allows to see what lints we have

#![allow(clippy::multiple_crate_versions, missing_docs)]

use crate::bridge::Bridge;
use magnus::value::Lazy;
use magnus::{Error, RModule, Ruby};
use std::sync::{LazyLock, OnceLock};
#[cfg(not(target_env = "msvc"))]
use tikv_jemallocator::Jemalloc;
use tokio::runtime::Runtime;

mod admin;
mod bridge;
mod client;
mod gvl;
mod handler;
mod logging;
mod scheduler;
mod tracing_util;
mod util;

#[cfg(not(target_env = "msvc"))]
#[global_allocator]
static GLOBAL: Jemalloc = Jemalloc;

/// Buffer size for the communication channel between Ruby and Rust.
/// This controls how many operations can be queued before backpressure is
/// applied.
const BRIDGE_BUFFER_SIZE: usize = 64;

/// Global instance of the Ruby-Rust communication bridge.
/// Initialized during extension startup and used throughout the library.
pub static BRIDGE: OnceLock<Bridge> = OnceLock::new();

/// Ensures tracing initialization occurs exactly once.
pub static TRACING_INIT: OnceLock<()> = OnceLock::new();

/// Global Tokio runtime for asynchronous operations.
///
/// This runtime powers all async operations in the extension, including
/// message processing, scheduling, and communication with Ruby.
#[allow(clippy::expect_used)]
static RUNTIME: LazyLock<Runtime> =
    LazyLock::new(|| Runtime::new().expect("Failed to create Tokio runtime"));

/// Reference to the root Ruby module for this extension.
///
/// This is lazily initialized during extension startup and provides
/// access to the `Prosody` module in Ruby.
#[allow(clippy::expect_used)]
pub static ROOT_MOD: Lazy<RModule> = Lazy::new(|ruby| {
    ruby.define_module("Prosody")
        .expect("Failed to define Prosody module")
});

/// Initializes the Prosody Ruby extension.
///
/// This function initializes the various components of the extension.
///
/// # Arguments
///
/// * `ruby` - Reference to the Ruby VM instance
///
/// # Errors
///
/// Returns a Magnus error if any initialization step fails, such as
/// defining Ruby classes or configuring components.
#[magnus::init]
fn init(ruby: &Ruby) -> Result<(), Error> {
    admin::init(ruby)?;
    bridge::init(ruby)?;
    handler::init(ruby)?;
    client::init(ruby)?;

    Ok(())
}
