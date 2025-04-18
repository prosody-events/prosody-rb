#![allow(clippy::multiple_crate_versions)]
#![allow(missing_docs, dead_code)] // todo: remove
#![allow(clippy::unwrap_used)] // todo: remove

use magnus::value::Lazy;
use magnus::{Error, RModule, Ruby};
use prosody::tracing::{Identity, initialize_tracing};
use std::sync::LazyLock;
use tokio::runtime::Runtime;

mod bridge;
mod client;
mod gvl;
mod handler;
mod scheduler;
mod util;

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

    #[allow(clippy::expect_used)] // todo: remove expect
    initialize_tracing::<Identity>(None).expect("Failed to initialize tracing system");

    bridge::init(ruby)?;
    handler::init(ruby)?;
    client::init(ruby)?;

    Ok(())
}
