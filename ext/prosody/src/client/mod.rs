use crate::bridge::Bridge;
use crate::client::config::NativeConfiguration;
use crate::handler::RubyHandler;
use crate::scheduler::Scheduler;
use crate::{BRIDGE, ROOT_MOD, RUNTIME, id};
use magnus::value::ReprValue;
use magnus::{Error, Module, Object, Ruby, Value, function, method};
use prosody::high_level::HighLevelClient;
use prosody::high_level::mode::Mode;
use serde_magnus::deserialize;
use std::sync::Arc;

mod config;

#[derive(Debug)]
#[magnus::wrap(class = "Prosody::Client", free_immediately, frozen_shareable, size)]
pub struct Client {
    inner: Arc<HighLevelClient<RubyHandler>>,
    bridge: Bridge,
    scheduler: Scheduler,
}

impl Client {
    fn new(ruby: &Ruby, config: Value) -> Result<Self, Error> {
        let _guard = RUNTIME.enter();
        let native_config: NativeConfiguration = config.funcall(id!("to_hash"), ())?;
        let config_ref = &native_config;

        let mode: Mode = config_ref
            .try_into()
            .map_err(|error: String| Error::new(ruby.exception_arg_error(), error))?;

        let client = HighLevelClient::new(
            mode,
            &mut config_ref.into(),
            &config_ref.into(),
            &config_ref.into(),
            &config_ref.into(),
        )
        .map_err(|error| Error::new(ruby.exception_runtime_error(), error.to_string()))?;

        let bridge = BRIDGE
            .get()
            .ok_or(Error::new(
                ruby.exception_runtime_error(),
                "Bridge not initialized",
            ))?
            .clone();

        Ok(Self {
            inner: Arc::new(client),
            bridge: bridge.clone(),
            scheduler: Scheduler::new(ruby, bridge)?,
        })
    }

    fn send_message(
        ruby: &Ruby,
        this: &Self,
        topic: String,
        key: String,
        payload: Value,
    ) -> Result<(), Error> {
        let _guard = RUNTIME.enter();
        let client = this.inner.clone();
        let value = deserialize(payload)?;

        this.bridge
            .wait_for(ruby, async move {
                client.send(topic.as_str().into(), &key, &value).await
            })?
            .map_err(|e| Error::new(ruby.exception_runtime_error(), e.to_string()))
    }

    fn subscribe(ruby: &Ruby, this: &Self, handler: Value) -> Result<(), Error> {
        let _guard = RUNTIME.enter();
        let wrapper = RubyHandler::new(this.bridge.clone(), ruby, handler)?;
        this.inner
            .subscribe(wrapper)
            .map_err(|error| Error::new(ruby.exception_runtime_error(), error.to_string()))?;

        Ok(())
    }

    fn unsubscribe(ruby: &Ruby, this: &Self) -> Result<(), Error> {
        let _guard = RUNTIME.enter();
        let client = this.inner.clone();

        this.bridge
            .wait_for(ruby, async move { client.unsubscribe().await })?
            .map_err(|e| Error::new(ruby.exception_runtime_error(), e.to_string()))
    }
}

pub fn init(ruby: &Ruby) -> Result<(), Error> {
    let module = ruby.get_inner(&ROOT_MOD);
    let class = module.define_class(id!("Client"), ruby.class_object())?;

    class.define_singleton_method("new", function!(Client::new, 1))?;
    class.define_method(id!("send_message"), method!(Client::send_message, 3))?;
    class.define_method(id!("subscribe"), method!(Client::subscribe, 1))?;
    class.define_method(id!("unsubscribe"), method!(Client::unsubscribe, 0))?;

    Ok(())
}
