use crate::gvl::{without_gvl, GvlError};
use futures::executor::block_on;
use futures::TryFutureExt;
use kanal::{bounded, AsyncReceiver, AsyncSender};
use magnus::Ruby;
use thiserror::Error;
use tokio::sync::oneshot;
use tokio::{select, try_join};
use tracing::{error, warn};

struct RubyFn<F, T> {
    ruby_fn: F,
    result_tx: oneshot::Sender<T>,
}

#[derive(Debug)]
pub struct RubyBridge<F, T> {
    tx: AsyncSender<RubyFn<F, T>>,
    rx: AsyncReceiver<RubyFn<F, T>>,
}

impl<F, T> Clone for RubyBridge<F, T> {
    fn clone(&self) -> Self {
        Self {
            tx: self.tx.clone(),
            rx: self.rx.clone(),
        }
    }
}

impl<F, T> Default for RubyBridge<F, T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<F, T> RubyBridge<F, T> {
    pub fn new() -> Self {
        let (tx, rx) = bounded(64);
        Self {
            tx: tx.to_async(),
            rx: rx.to_async(),
        }
    }

    pub fn initialize(&self, ruby: &Ruby)
    where
        F: FnOnce(&Ruby) -> T + Send + 'static,
        T: Send + 'static,
    {
        let instance: Self = self.clone();
        ruby.thread_create_from_fn::<_, ()>(move |ruby| {
            loop {
                if let Err(e) = instance.poll(ruby) {
                    error!("Error during poll: {e:?}");
                }
            }
        });
    }

    pub async fn ruby_exec(&self, ruby_fn: F) -> Result<T, BridgeError>
    where
        F: FnOnce(&Ruby) -> T,
    {
        let (result_tx, result_rx) = oneshot::channel();
        let command = RubyFn { ruby_fn, result_tx };

        let ((), result) = try_join!(
            self.tx.send(command).map_err(|_| BridgeError::Shutdown),
            result_rx.map_err(|_| BridgeError::Shutdown)
        )?;

        Ok(result)
    }

    pub fn poll(&self, ruby: &Ruby) -> Result<(), BridgeError>
    where
        F: FnOnce(&Ruby) -> T,
    {
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
        let result = (command.ruby_fn)(ruby);

        command
            .result_tx
            .send(result)
            .map_err(|_| BridgeError::CallerWentAway)
    }
}

#[derive(Debug, Error)]
pub enum BridgeError {
    #[error("An error occurred while polling: {0:#}")]
    Gvl(#[from] GvlError),

    #[error("Failed to create queue: {0:#}")]
    Queue(String),

    #[error("Command was cancelled by Ruby runtime")]
    Cancelled,

    #[error("Caller went away")]
    CallerWentAway,

    #[error("Bridge was shutdown")]
    Shutdown,
}
