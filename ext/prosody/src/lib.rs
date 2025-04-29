#![allow(clippy::multiple_crate_versions)]
#![allow(missing_docs, dead_code)] // todo: remove
#![allow(clippy::unwrap_used)] // todo: remove

use crate::bridge::Bridge;
use crate::logging::Logger;
use magnus::value::Lazy;
use magnus::{Error, RModule, Ruby};
use prosody::tracing::initialize_tracing;
use std::sync::{LazyLock, OnceLock};
use tokio::runtime::Runtime;

mod bridge;
mod client;
mod gvl;
mod handler;
mod logging;
mod scheduler;
mod util;

const BRIDGE_BUFFER_SIZE: usize = 64;
pub static BRIDGE: OnceLock<Bridge> = OnceLock::new();

#[allow(clippy::expect_used)]
static RUNTIME: LazyLock<Runtime> =
    LazyLock::new(|| Runtime::new().expect("Failed to create Tokio runtime"));

#[allow(clippy::expect_used)]
pub static ROOT_MOD: Lazy<RModule> = Lazy::new(|ruby| {
    ruby.define_module("Prosody")
        .expect("Failed to define Prosody module")
});

#[magnus::init]
fn init(ruby: &Ruby) -> Result<(), Error> {
    let _guard = RUNTIME.enter();

    bridge::init(ruby)?;
    handler::init(ruby)?;
    client::init(ruby)?;

    let bridge = Bridge::new(ruby, BRIDGE_BUFFER_SIZE);

    BRIDGE
        .set(bridge.clone())
        .map_err(|_| Error::new(ruby.exception_runtime_error(), "Bridge already initialized"))?;

    initialize_tracing(Some(Logger::new(ruby, bridge)?))
        .map_err(|error| Error::new(ruby.exception_runtime_error(), error.to_string()))?;

    Ok(())
}
