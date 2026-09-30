//! # Prosody Ruby Extension
//!
//! This crate provides Ruby bindings for the Prosody event processing library.
//! It bridges the Rust implementation of Prosody with Ruby, allowing Ruby
//! applications to use Prosody for event processing and messaging.
//!
//! The extension handles asynchronous communication between Rust and Ruby,
//! provides client functionality for interacting with message brokers,
//! manages message handling, and includes logging and scheduling capabilities.

#![expect(
    clippy::multiple_crate_versions,
    reason = "the dependency graph resolves some crates at two versions"
)]
#![recursion_limit = "256"]

use crate::bridge::Bridge;
use crate::util::report;
use magnus::RModule;
use magnus::value::Lazy;
use rustfs_mimalloc::MiMalloc;
use std::process;
use std::sync::{LazyLock, OnceLock};
use tokio::runtime::{Builder, Runtime};

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

/// Stack size of each Tokio worker thread.
///
/// Core futures are large in debug builds. A timer write that polls through
/// the Cassandra driver overflows the Tokio default of 2 MiB.
const WORKER_STACK_SIZE: usize = 8 * 1024 * 1024;

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
static RUNTIME: LazyLock<Runtime> = LazyLock::new(|| {
    let runtime = Builder::new_multi_thread()
        .enable_all()
        .thread_stack_size(WORKER_STACK_SIZE)
        .build();

    match runtime {
        Ok(runtime) => runtime,
        Err(error) => {
            report(format_args!("failed to create Tokio runtime: {error:#}"));
            process::abort();
        }
    }
});

/// Reference to the root Ruby module for this extension.
///
/// This is lazily initialized during extension startup and provides
/// access to the `Prosody` module in Ruby.
#[expect(
    clippy::expect_used,
    reason = "magnus Lazy takes an infallible initializer"
)]
pub static ROOT_MOD: Lazy<RModule> = Lazy::new(|ruby| {
    ruby.define_module("Prosody")
        .expect("Failed to define Prosody module")
});

/// The extension entry point that Ruby calls when it loads the library.
///
/// `#[magnus::init]` generates the exported `Init_prosody` function beside
/// `init`. The module is private, so that generated function is not public
/// API.
mod entry {
    use magnus::{Error, Ruby};

    /// Initializes the Prosody Ruby extension.
    ///
    /// # Errors
    ///
    /// Returns a Magnus error if any initialization step fails, such as
    /// defining Ruby classes or configuring components.
    #[magnus::init]
    fn init(ruby: &Ruby) -> Result<(), Error> {
        crate::admin::init(ruby)?;
        crate::bridge::init(ruby)?;
        crate::handler::init(ruby)?;
        crate::published::init(ruby)?;
        crate::client::init(ruby)?;
        crate::util::init(ruby)?;

        Ok(())
    }
}
