use crate::bridge::callback::AsyncCallback;
use crate::gvl::{GvlError, without_gvl};
use crate::{ROOT_MOD, RUNTIME, id};
use atomic_take::AtomicTake;
use educe::Educe;
use futures::TryFutureExt;
use futures::executor::block_on;
use magnus::value::{Lazy, ReprValue};
use magnus::{Error, Module, Object, RClass, Ruby, Value, function};
use parking_lot::Mutex;
use std::any::Any;
use thiserror::Error;
use tokio::sync::mpsc::{Receiver, Sender, channel};
use tokio::sync::oneshot;
use tokio::{select, try_join};
use tracing::{debug, error, warn};

mod callback;

const POLL_BATCH_SIZE: usize = 16;

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

type RubyFunction = Box<dyn FnOnce(&Ruby) + Send>;

#[derive(Debug)]
#[magnus::wrap(class = "Prosody::DynamicResult", free_immediately)]
struct DynamicResult(AtomicTake<Box<dyn Any + Send>>);

#[derive(Educe, Clone)]
#[educe(Debug)]
#[magnus::wrap(class = "Prosody::Bridge", free_immediately, frozen_shareable, size)]
pub struct Bridge {
    #[educe(Debug(ignore))]
    tx: Sender<RubyFunction>,
}

impl Bridge {
    pub fn new(ruby: &Ruby, buffer_size: usize) -> Self {
        let (tx, mut rx) = channel(buffer_size);

        ruby.thread_create_from_fn(move |ruby| {
            loop {
                let Err(error) = poll(&mut rx, ruby) else {
                    continue;
                };

                if matches!(error, BridgeError::Shutdown) {
                    debug!("shutting down Ruby bridge");
                    break;
                }

                error!("error during Ruby bridge poll: {error:#}");
            }
        });

        Self { tx }
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
                error!("failed to send Ruby bridge result");
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

    pub fn run_future<Fut>(&self, ruby: &Ruby, future: Fut) -> Result<Fut::Output, Error>
    where
        Fut: Future + Send + 'static,
        Fut::Output: Send,
    {
        let queue: Value = ruby.get_inner(&QUEUE_CLASS).funcall(id!("new"), ())?;
        let callback = AsyncCallback::from_queue(queue);
        let tx = self.tx.clone();

        RUNTIME.spawn(async move {
            let result = future.await;
            let function = move |ruby: &Ruby| {
                let value = DynamicResult(AtomicTake::new(Box::new(result)));
                if let Err(error) = callback.complete(ruby, value) {
                    error!("failed to complete Ruby callback: {error:#}");
                }
            };

            if let Err(error) = tx.send(Box::new(function)).await {
                error!("failed to send callback to Ruby: {error:#}");
            }
        });

        let result: &DynamicResult = queue.funcall(id!("pop"), ())?;
        let result = result.0.take().ok_or_else(|| {
            Error::new(ruby.exception_runtime_error(), "failed to extract result")
        })?;

        let result: Box<Fut::Output> = result
            .downcast()
            .map_err(|_| Error::new(ruby.exception_runtime_error(), "failed to extract result"))?;

        Ok(*result)
    }
}

fn poll(rx: &mut Receiver<RubyFunction>, ruby: &Ruby) -> Result<(), BridgeError> {
    let (cancel_tx, cancel_rx) = oneshot::channel();
    let mut maybe_cancel_tx = Some(cancel_tx);

    let poll_fn = || {
        let maybe_command = block_on(async {
            select! {
                _ = cancel_rx => Err(BridgeError::Cancelled),
                result = rx.recv() => Ok(result),
            }
        })?;

        let first_command = maybe_command.ok_or(BridgeError::Shutdown)?;

        let mut commands = Vec::with_capacity(POLL_BATCH_SIZE);
        commands.push(first_command);

        while commands.len() < POLL_BATCH_SIZE {
            if let Ok(command) = rx.try_recv() {
                commands.push(command);
            } else {
                break;
            }
        }

        Result::<_, BridgeError>::Ok(commands)
    };

    let cancel_fn = || {
        let Some(cancel_tx) = maybe_cancel_tx.take() else {
            return;
        };

        if cancel_tx.send(()).is_err() {
            warn!("Failed to cancel poll operation");
        }
    };

    let commands = without_gvl(poll_fn, cancel_fn)??;
    for command in commands {
        command(ruby);
    }

    Ok(())
}

#[derive(Debug, Error)]
pub enum BridgeError {
    #[error("an error occurred while polling: {0:#}")]
    Gvl(#[from] GvlError),

    #[error("command was cancelled by Ruby runtime")]
    Cancelled,

    #[error("Ruby bridge was shutdown")]
    Shutdown,
}

pub fn init(ruby: &Ruby) -> Result<(), Error> {
    let module = ruby.get_inner(&ROOT_MOD);
    let result_class = module.define_class("DynamicResult", ruby.class_object())?;
    let bridge_class = module.define_class("Bridge", ruby.class_object())?;
    bridge_class.define_singleton_method("new", function!(Bridge::new, 1))?;

    Ok(())
}
