use crate::bridge::Bridge;
use crate::client::config::NativeConfiguration;
use crate::handler::RubyHandler;
use crate::{ROOT_MOD, id};
use magnus::value::ReprValue;
use magnus::{Error, Module, Object, Ruby, Value, function, method};
use prosody::high_level::HighLevelClient;
use prosody::high_level::mode::Mode;
use serde_magnus::deserialize;
use std::sync::Arc;

mod config;

const BRIDGE_BUFFER_SIZE: usize = 64;

#[derive(Debug)]
#[magnus::wrap(
    class = "Prosody::NativeClient",
    free_immediately,
    frozen_shareable,
    size
)]
pub struct NativeClient {
    client: Arc<HighLevelClient<RubyHandler>>,
    bridge: Bridge,
}

impl NativeClient {
    fn new(ruby: &Ruby, config: Value) -> Result<Self, Error> {
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

        Ok(Self {
            client: Arc::new(client),
            bridge: Bridge::new(ruby, BRIDGE_BUFFER_SIZE),
        })
    }

    fn send_message(
        ruby: &Ruby,
        this: &Self,
        topic: String,
        key: String,
        payload: Value,
    ) -> Result<(), Error> {
        let client = this.client.clone();
        let value = deserialize(payload)?;

        this.bridge
            .wait_for(ruby, async move {
                client.send(topic.as_str().into(), &key, &value).await
            })?
            .map_err(|e| Error::new(ruby.exception_runtime_error(), e.to_string()))
    }

    fn subscribe(ruby: &Ruby, this: &Self, handler: Value) -> Result<(), Error> {
        // let bridge = this.bridge.clone();
        // let function = move |ruby: &Ruby| {
        //     ruby.module_kernel()
        //         .funcall::<_, _, Value>(id!("puts"), ("sleeping",))
        //         .unwrap();
        //
        //     ruby.module_kernel()
        //         .funcall::<_, _, Value>(id!("sleep"), (2.0_f64,))
        //         .unwrap();
        //
        //     ruby.module_kernel()
        //         .funcall::<_, _, Value>(id!("puts"), ("done sleeping",))
        //         .unwrap();
        // };
        //
        // let bridge = this.bridge.clone();
        // let future = async move {
        //     let task = bridge.run_async(function).await?;
        //     task.await?;
        //     Result::<(), BridgeError>::Ok(())
        // };
        //
        // this.bridge.wait_for(ruby, future)?.unwrap();

        Ok(())
    }
}

pub fn init(ruby: &Ruby) -> Result<(), Error> {
    let module = ruby.get_inner(&ROOT_MOD);
    let class = module.define_class(id!("NativeClient"), ruby.class_object())?;

    class.define_singleton_method("new", function!(NativeClient::new, 1))?;
    class.define_method(id!("send_message"), method!(NativeClient::send_message, 3))?;
    class.define_method(id!("subscribe"), method!(NativeClient::subscribe, 1))?;

    Ok(())
}
