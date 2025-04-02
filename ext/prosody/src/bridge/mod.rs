use crate::bridge::core::RubyBridge;
use crate::{id, ROOT_MOD, RUNTIME};
use educe::Educe;
use magnus::value::{Lazy, ReprValue};
use magnus::{method, Class, Error, IntoValue, Module, RModule, Ruby, Value};

mod callback;
mod core;

pub trait RubyCallable: Send + 'static {
    type Output: Send;

    fn execute(self, ruby: &Ruby) -> Self::Output;
}

pub trait RustCallable: Send + 'static {
    type Output: IntoValue + Send;

    fn execute(self) -> impl Future<Output = Self::Output> + Send;
}

#[allow(clippy::expect_used)]
pub static BRIDGE_MOD: Lazy<RModule> = Lazy::new(|ruby| {
    ruby.get_inner(&ROOT_MOD)
        .define_module("Bridge")
        .expect("Failed to define Bridge module")
});

struct TestCallable;

impl RubyCallable for TestCallable {
    type Output = ();

    fn execute(self, ruby: &Ruby) -> Self::Output {
        ruby.module_kernel()
            .funcall::<_, _, Value>(id!("puts"), ("hello",))
            .unwrap();
    }
}

impl RustCallable for TestCallable {
    type Output = i64;

    async fn execute(self) -> Self::Output {
        1 + 1
    }
}

#[derive(Clone, Educe, Default)]
#[educe(Debug)]
#[magnus::wrap(class = "Prosody::Bridge::DynamicBridge", free_immediately)]
pub struct DynamicBridge {
    #[educe(Debug(ignore))]
    inner: RubyBridge<TestCallable, TestCallable>,
}

impl DynamicBridge {
    pub fn new() -> Self {
        Self {
            inner: RubyBridge::default(),
        }
    }

    pub fn initialize(ruby: &Ruby, this: &Self) {
        this.inner.initialize(ruby);
    }

    pub fn test_ruby_exec(this: &Self) {
        let inner = this.inner.clone();
        RUNTIME.spawn(async move { inner.ruby_exec(TestCallable).await });
    }

    pub fn test_rust_exec(ruby: &Ruby, this: &Self) -> Value {
        this.inner.rust_exec(ruby, TestCallable).unwrap()
    }
}

pub fn init(ruby: &Ruby) -> Result<(), Error> {
    let module = ruby.get_inner(&BRIDGE_MOD);

    let class = module.define_class("DynamicBridge", ruby.class_object())?;
    class.define_alloc_func::<DynamicBridge>();
    class.define_method("initialize", method!(DynamicBridge::initialize, 0))?;
    class.define_method("test_ruby_exec", method!(DynamicBridge::test_ruby_exec, 0))?;
    class.define_method("test_rust_exec", method!(DynamicBridge::test_rust_exec, 0))?;

    Ok(())
}
