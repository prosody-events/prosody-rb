use crate::bridge::callback::AsyncCallback;
use crate::gvl::{without_gvl, GvlError};
use crate::{id, RUNTIME};
use futures::executor::block_on;
use futures::TryFutureExt;
use kanal::{bounded, AsyncReceiver, AsyncSender};
use magnus::value::{Lazy, ReprValue};
use magnus::{Module, RClass, Ruby, Value};
use thiserror::Error;
use tokio::sync::oneshot;
use tokio::{select, try_join};
use tracing::{error, warn};

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

pub trait RubyCallable: Send + 'static {
    type Output: Send;

    fn execute(self, ruby: &Ruby) -> Self::Output;
}

pub trait RustCallable: Send + 'static {
    type Output: Send;

    fn execute(self) -> impl Future<Output = Self::Output> + Send;

    fn translate(output: Self::Output, ruby: &Ruby) -> Value;
}

impl<F, T> RubyCallable for F
where
    F: FnOnce(&Ruby) -> T + Send + 'static,
    T: Send,
{
    type Output = T;

    fn execute(self, ruby: &Ruby) -> Self::Output {
        self(ruby)
    }
}

enum Command<T, U>
where
    T: RubyCallable,
    U: RustCallable,
{
    Call {
        callable: T,
        result_tx: oneshot::Sender<T::Output>,
    },
    Callback {
        output: U::Output,
        queue: AsyncCallback,
    },
}

#[derive(Debug)]
pub struct RubyBridge<T, U>
where
    T: RubyCallable,
    U: RustCallable,
{
    tx: AsyncSender<Command<T, U>>,
    rx: AsyncReceiver<Command<T, U>>,
}

impl<T, U> Clone for RubyBridge<T, U>
where
    T: RubyCallable,
    U: RustCallable,
{
    fn clone(&self) -> Self {
        Self {
            tx: self.tx.clone(),
            rx: self.rx.clone(),
        }
    }
}

impl<T, U> Default for RubyBridge<T, U>
where
    T: RubyCallable,
    U: RustCallable,
{
    fn default() -> Self {
        Self::new(DEFAULT_BUFFER_SIZE)
    }
}

impl<T, U> RubyBridge<T, U>
where
    T: RubyCallable,
    U: RustCallable,
{
    pub fn new(buffer_size: usize) -> Self {
        let (tx, rx) = bounded(buffer_size);
        Self {
            tx: tx.to_async(),
            rx: rx.to_async(),
        }
    }

    pub fn initialize(&self, ruby: &Ruby) {
        let instance: Self = self.clone();
        ruby.thread_create_from_fn::<_, ()>(move |ruby| {
            loop {
                if let Err(e) = instance.poll(ruby) {
                    error!("Error during poll: {e:?}");
                }
            }
        });
    }

    pub async fn ruby_exec(&self, callable: T) -> Result<T::Output, BridgeError> {
        let (result_tx, result_rx) = oneshot::channel();
        let command = Command::Call {
            callable,
            result_tx,
        };

        let ((), result) = try_join!(
            self.tx.send(command).map_err(|_| BridgeError::Shutdown),
            result_rx.map_err(|_| BridgeError::Shutdown)
        )?;

        Ok(result)
    }

    pub fn rust_exec(&self, ruby: &Ruby, callable: U) -> Result<Value, BridgeError> {
        let queue: Value = ruby
            .get_inner(&QUEUE_CLASS)
            .funcall(id!("new"), ())
            .map_err(|e| BridgeError::QueueCreate(e.to_string()))?;

        let tx = self.tx.clone();
        let callback = AsyncCallback::from_queue(queue);

        RUNTIME.spawn(async move {
            if let Err(error) = tx
                .send(Command::Callback {
                    output: callable.execute().await,
                    queue: callback,
                })
                .await
            {
                error!("Failed to send callback: {error:?}");
            }
        });

        queue
            .funcall(id!("pop"), ())
            .map_err(|e| BridgeError::QueuePop(e.to_string()))
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

        match command {
            Command::Call {
                callable,
                result_tx,
            } => result_tx
                .send(callable.execute(ruby))
                .map_err(|_| BridgeError::CallerWentAway),

            Command::Callback { output, queue } => queue
                .complete(ruby, U::translate(output, ruby))
                .map_err(|e| BridgeError::QueuePush(e.to_string())),
        }
    }
}

#[derive(Debug, Error)]
pub enum BridgeError {
    #[error("An error occurred while polling: {0:#}")]
    Gvl(#[from] GvlError),

    #[error("Failed to create queue: {0:#}")]
    QueueCreate(String),

    #[error("Failed pop value off queue: {0:#}")]
    QueuePop(String),

    #[error("Failed push value onto queue: {0:#}")]
    QueuePush(String),

    #[error("Command was cancelled by Ruby runtime")]
    Cancelled,

    #[error("Caller went away")]
    CallerWentAway,

    #[error("Bridge was shutdown")]
    Shutdown,
}
