use crate::bridge::callback::AsyncCallback;
use crate::gvl::{GvlError, without_gvl};
use crate::{ROOT_MOD, RUNTIME, id};
use educe::Educe;
use futures::TryFutureExt;
use futures::executor::block_on;
use kanal::AsyncSender;
use kanal::{AsyncReceiver, bounded};
use magnus::value::{Lazy, ReprValue};
use magnus::{Class, Error, IntoValue, Module, RClass, Ruby, Value, method};
use std::time::Duration;
use thiserror::Error;
use tokio::sync::oneshot;
use tokio::time::sleep;
use tokio::{select, try_join};
use tracing::{error, warn};

mod callback;

#[allow(clippy::expect_used)]
pub static THREAD_CLASS: Lazy<RClass> = Lazy::new(|ruby| {
    ruby.class_object()
        .const_get(id!("Thread"))
        .expect("Failed to load Thread class")
});

#[allow(clippy::expect_used)]
pub static QUEUE_CLASS: Lazy<RClass> = Lazy::new(|ruby| {
    ruby.get_inner(&THREAD_CLASS)
        .const_get(id!("Queue"))
        .expect("Failed to load Queue class")
});

const DEFAULT_BUFFER_SIZE: usize = 64;

type RubyFunction = Box<dyn FnOnce(&Ruby) + Send>;

#[derive(Educe)]
#[educe(Debug)]
#[magnus::wrap(class = "Prosody::Bridge", free_immediately)]
pub struct RubyBridge {
    #[educe(Debug(ignore))]
    tx: AsyncSender<RubyFunction>,

    #[educe(Debug(ignore))]
    rx: AsyncReceiver<RubyFunction>,
}

impl Clone for RubyBridge {
    fn clone(&self) -> Self {
        Self {
            tx: self.tx.clone(),
            rx: self.rx.clone(),
        }
    }
}

impl Default for RubyBridge {
    fn default() -> Self {
        Self::new(DEFAULT_BUFFER_SIZE)
    }
}

impl RubyBridge {
    fn new(buffer_size: usize) -> Self {
        let (tx, rx) = bounded(buffer_size);
        Self {
            tx: tx.to_async(),
            rx: rx.to_async(),
        }
    }

    fn initialize(ruby: &Ruby, this: &Self) {
        let instance: Self = this.clone();
        ruby.thread_create_from_fn::<_, ()>(move |ruby| {
            loop {
                if let Err(e) = instance.poll(ruby) {
                    error!("Error during poll: {e:?}");
                }
            }
        });
    }

    pub async fn ruby_exec<F, T>(&self, function: F) -> Result<T, BridgeError>
    where
        F: FnOnce(&Ruby) -> T + Send + 'static,
        T: Send + 'static,
    {
        let (result_tx, result_rx) = oneshot::channel();

        let function = |ruby: &Ruby| {
            let result = function(ruby);
            if result_tx.send(result).is_err() {
                error!("Failed to send result");
            }
        };

        let ((), result) = try_join!(
            self.tx
                .send(Box::new(function))
                .map_err(|_| BridgeError::Shutdown),
            result_rx.map_err(|_| BridgeError::Shutdown)
        )?;

        Ok(result)
    }

    pub fn rust_exec<Fut>(&self, ruby: &Ruby, future: Fut) -> Result<Value, Error>
    where
        Fut: Future + Send + 'static,
        Fut::Output: IntoValue + Send,
    {
        let queue: Value = ruby.get_inner(&QUEUE_CLASS).funcall(id!("new"), ())?;
        let callback = AsyncCallback::from_queue(queue);
        let tx = self.tx.clone();

        RUNTIME.spawn(async move {
            let result = future.await;
            let function = move |ruby: &Ruby| {
                let value = result.into_value_with(ruby);
                if let Err(error) = callback.complete(ruby, value) {
                    error!("failed to complete Ruby callback: {error:#}");
                }
            };

            if let Err(error) = tx.send(Box::new(function)).await {
                error!("failed to send callback to Ruby: {error:#}");
            }
        });

        queue.funcall(id!("pop"), ())
    }

    fn poll(&self, ruby: &Ruby) -> Result<(), BridgeError> {
        let (cancel_tx, cancel_rx) = oneshot::channel();
        let mut maybe_cancel_tx = Some(cancel_tx);

        let poll_fn = || {
            let maybe_command = block_on(async {
                select! {
                    _ = cancel_rx => Err(BridgeError::Cancelled),
                    result = self.rx.recv() => Ok(result.ok()),
                }
            })?;

            maybe_command.ok_or(BridgeError::Shutdown)
        };

        let cancel_fn = || {
            let Some(cancel_tx) = maybe_cancel_tx.take() else {
                return;
            };

            if cancel_tx.send(()).is_err() {
                warn!("Failed to cancel poll operation");
            }
        };

        let command = without_gvl(poll_fn, cancel_fn)??;
        command(ruby);

        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum BridgeError {
    #[error("An error occurred while polling: {0:#}")]
    Gvl(#[from] GvlError),

    #[error("Command was cancelled by Ruby runtime")]
    Cancelled,

    #[error("Bridge was shutdown")]
    Shutdown,
}

pub fn init(ruby: &Ruby) -> Result<(), Error> {
    let module = ruby.get_inner(&ROOT_MOD);
    let class = module.define_class("Bridge", ruby.class_object())?;

    class.define_alloc_func::<RubyBridge>();
    class.define_method("initialize", method!(RubyBridge::initialize, 0))?;

    Ok(())
}
