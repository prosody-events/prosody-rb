use crate::bridge::Bridge;
use crate::client::config::NativeConfiguration;
use crate::handler::RubyHandler;
use crate::scheduler::handle::TaskHandle;
use crate::scheduler::{Scheduler, SchedulerError};
use crate::{ROOT_MOD, id};
use futures::future::try_join_all;
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
    scheduler: Scheduler,
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

        let bridge = Bridge::new(ruby, BRIDGE_BUFFER_SIZE);

        Ok(Self {
            client: Arc::new(client),
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
        let client = this.client.clone();
        let value = deserialize(payload)?;

        this.bridge
            .wait_for(ruby, async move {
                client.send(topic.as_str().into(), &key, &value).await
            })?
            .map_err(|e| Error::new(ruby.exception_runtime_error(), e.to_string()))
    }

    fn subscribe(ruby: &Ruby, this: &Self, handler: Value) -> Result<(), Error> {
        let cloned_scheduler = this.scheduler.clone();
        this.bridge
            .wait_for(ruby, async move {
                let mut task_futures = vec![];
                for i in 0..10 {
                    let task_id = format!("test-task-{i}");
                    let before = format!("{task_id} is sleeping for 5 seconds");
                    let after = format!("{task_id} finished sleeping");

                    task_futures.push(cloned_scheduler.schedule(task_id, |ruby| {
                        ruby.module_kernel()
                            .funcall::<_, _, Value>(id!("puts"), (before,))
                            .map(|_| ())?;

                        let _: Value = ruby.eval("sleep 5")?;

                        ruby.module_kernel()
                            .funcall::<_, _, Value>(id!("puts"), (after,))
                            .map(|_| ())?;

                        Ok(())
                    }));
                }

                let task_handles: Vec<TaskHandle> = try_join_all(task_futures).await?;
                for handle in task_handles {
                    if let Err(error) = handle.result().await {
                        println!("error: {error:#}");
                    }
                }

                Ok(())
            })?
            .map_err(|e: SchedulerError| {
                Error::new(ruby.exception_runtime_error(), e.to_string())
            })?;

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
