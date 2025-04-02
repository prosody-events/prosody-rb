#![allow(clippy::multiple_crate_versions)]
#![allow(missing_docs, dead_code)] // todo: remove

use magnus::value::Lazy;
use magnus::{function, Error, Object, RModule, Ruby};
use prosody::tracing::{initialize_tracing, Identity};
use std::sync::LazyLock;
use tokio::runtime::Runtime;

mod bridge;
mod gvl;

#[allow(clippy::expect_used)]
static RUNTIME: LazyLock<Runtime> =
    LazyLock::new(|| Runtime::new().expect("Failed to create Tokio runtime"));

#[allow(clippy::expect_used)]
pub static ROOT_MOD: Lazy<RModule> = Lazy::new(|ruby| {
    ruby.define_module("Prosody")
        .expect("Failed to define Prosody module")
});

fn hello(subject: String) -> String {
    format!("Hello from Rust, {subject}!")
}

#[magnus::init]
fn init(ruby: &Ruby) -> Result<(), Error> {
    #[allow(clippy::expect_used)] // todo: remove expect
    initialize_tracing::<Identity>(None).expect("Failed to initialize tracing system");

    let module = ruby.get_inner(&ROOT_MOD);
    bridge::init(ruby)?;
    module.define_singleton_method("hello", function!(hello, 1))?;

    Ok(())
}
