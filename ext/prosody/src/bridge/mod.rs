use crate::bridge::core::RubyBridge;
use crate::{ROOT_MOD, RUNTIME};
use educe::Educe;
use magnus::value::{Lazy, ReprValue};
use magnus::{method, Class, Error, Module, RClass, RModule, Ruby, Value};
use std::sync::Arc;
use tracing::error;

mod bidirectional;
mod core;

#[allow(clippy::expect_used)]
pub static BRIDGE_MOD: Lazy<RModule> = Lazy::new(|ruby| {
    ruby.get_inner(&ROOT_MOD)
        .define_module("Bridge")
        .expect("Failed to define Bridge module")
});

#[allow(clippy::expect_used)]
pub static THREAD_CLASS: Lazy<RClass> = Lazy::new(|ruby| {
    ruby.class_object()
        .const_get("Thread")
        .expect("Failed to load Thread class")
});

#[allow(clippy::expect_used)]
pub static QUEUE_CLASS: Lazy<RClass> = Lazy::new(|ruby| {
    ruby.get_inner(&THREAD_CLASS)
        .const_get("Queue")
        .expect("Failed to load Queue class")
});

#[derive(Clone, Educe, Default)]
#[educe(Debug)]
#[magnus::wrap(class = "Prosody::Bridge::DynamicBridge", free_immediately)]
pub struct DynamicBridge {
    #[educe(Debug(ignore))]
    inner: Arc<RubyBridge<Box<dyn (Fn(&Ruby)) + Send>, ()>>,
}

impl DynamicBridge {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RubyBridge::new()),
        }
    }

    pub fn initialize(ruby: &Ruby, this: &Self) {
        this.inner.initialize(ruby);
    }

    pub fn poll(ruby: &Ruby, this: &Self) -> Result<(), Error> {
        this.inner
            .poll(ruby)
            .map_err(|error| Error::new(ruby.exception_runtime_error(), error.to_string()))
    }

    pub fn feed(ruby: &Ruby, this: &Self) {
        // this.inner.rust_async(ruby).unwrap();
        let inner = this.inner.clone();
        RUNTIME.spawn(async move {
            inner
                .ruby_exec(Box::new(|r| {
                    if let Err(error) = r.module_kernel().funcall::<_, _, Value>("puts", ("hello",))
                    {
                        error!("Error while calling puts: {error:?}");
                    }
                }))
                .await
        });
    }
}

pub fn init(ruby: &Ruby) -> Result<(), Error> {
    let module = ruby.get_inner(&BRIDGE_MOD);

    let class = module.define_class("DynamicBridge", ruby.class_object())?;
    class.define_alloc_func::<DynamicBridge>();
    class.define_method("initialize", method!(DynamicBridge::initialize, 0))?;
    class.define_method("poll", method!(DynamicBridge::poll, 0))?;
    class.define_method("feed", method!(DynamicBridge::feed, 0))?;

    Ok(())
}
