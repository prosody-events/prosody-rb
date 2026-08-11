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
#![recursion_limit = "256"]

use crate::bridge::Bridge;
use magnus::value::Lazy;
use magnus::{Error, RModule, Ruby};
use mimalloc::MiMalloc;
use std::ops::Deref;
use std::process;
use std::sync::{LazyLock, OnceLock};
use tokio::runtime::{Handle, Runtime};
use tracing::error;

mod admin;
mod bridge;
mod client;
mod gvl;
mod handler;
mod logging;
mod published;
mod scheduler;
mod tracing_util;
mod util;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

/// Global instance of the Ruby-Rust communication bridge.
/// Initialized during extension startup and used throughout the library.
pub static BRIDGE: OnceLock<Bridge> = OnceLock::new();

/// Ensures tracing initialization occurs exactly once.
pub static TRACING_INIT: OnceLock<()> = OnceLock::new();

/// Global Tokio runtime for asynchronous operations.
///
/// This runtime powers all async operations in the extension, including
/// message processing, scheduling, and communication with Ruby.
static RUNTIME: LazyLock<RuntimeOwner> = LazyLock::new(RuntimeOwner::new);

/// Owns the runtime without blocking Ruby process exit.
struct RuntimeOwner {
    handle: Handle,
    runtime: Option<Runtime>,
}

impl RuntimeOwner {
    fn new() -> Self {
        let runtime = match Runtime::new() {
            Ok(runtime) => runtime,
            Err(error) => {
                error!(%error, "failed to create Tokio runtime");
                process::abort();
            }
        };
        Self {
            handle: runtime.handle().clone(),
            runtime: Some(runtime),
        }
    }
}

impl Deref for RuntimeOwner {
    type Target = Handle;

    fn deref(&self) -> &Self::Target {
        &self.handle
    }
}

impl Drop for RuntimeOwner {
    fn drop(&mut self) {
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown_background();
        }
    }
}

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
    published::init(ruby)?;
    client::init(ruby)?;
    util::init(ruby)?;

    Ok(())
}
