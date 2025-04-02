use crate::gvl::{without_gvl, GvlError};
use crate::{ROOT_MOD, RUNTIME};
use educe::Educe;
use futures::executor::block_on;
use futures::TryFutureExt;
use kanal::{bounded, AsyncReceiver, AsyncSender};
use magnus::block::Proc;
use magnus::value::{Lazy, ReprValue};
use magnus::{method, Class, Error, Module, RClass, RModule, Ruby, Value};
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::oneshot;
use tokio::{select, try_join};
use tracing::{error, warn};

#[allow(clippy::expect_used)]
pub static BRIDGE_MOD: Lazy<RModule> = Lazy::new(|ruby| {
    ruby.get_inner(&ROOT_MOD)
        .define_module("Bridge")
        .expect("Failed to define Bridge module")
});

trait Command {
    type Output;

    fn call(ruby: &Ruby) -> Self::Output;
}

struct RubyCommand<F, T> {
    function: F,
    result_tx: oneshot::Sender<T>,
}

#[derive(Debug)]
struct Bridge<F, T> {
    tx: AsyncSender<RubyCommand<F, T>>,
    rx: AsyncReceiver<RubyCommand<F, T>>,
}

impl<F, T> Default for Bridge<F, T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<F, T> Bridge<F, T> {
    fn new() -> Self {
        let (tx, rx) = bounded(64);
        Self {
            tx: tx.to_async(),
            rx: rx.to_async(),
        }
    }

    async fn run_in_ruby(&self, function: F) -> Result<T, BridgeError>
    where
        F: FnOnce(&Ruby) -> T,
    {
        let (result_tx, result_rx) = oneshot::channel();
        let command = RubyCommand {
            function,
            result_tx,
        };

        let ((), result) = try_join!(
            self.tx.send(command).map_err(|_| BridgeError::Shutdown),
            result_rx.map_err(|_| BridgeError::Shutdown)
        )?;

        Ok(result)
    }

    fn poll(&self, ruby: &Ruby) -> Result<(), BridgeError>
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
        let result = (command.function)(ruby);

        command
            .result_tx
            .send(result)
            .map_err(|_| BridgeError::CallerWentAway)
    }
}

#[derive(Clone, Educe, Default)]
#[educe(Debug)]
#[magnus::wrap(class = "Prosody::Bridge::DynamicBridge", free_immediately)]
pub struct DynamicBridge {
    #[educe(Debug(ignore))]
    inner: Arc<Bridge<Box<dyn (Fn(&Ruby)) + Send>, ()>>,
}

impl DynamicBridge {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Bridge::new()),
        }
    }

    pub fn initialize(ruby: &Ruby, this: &Self) -> Result<(), Error> {
        let instance = this.clone();
        let thread_class = ruby.class_thread();

        // Create a proc using the recommended API.
        let poll_proc = ruby.proc_from_fn(
            move |ruby: &Ruby, _args: &[Value], _block: Option<Proc>| -> Result<(), Error> {
                loop {
                    if let Err(e) = DynamicBridge::poll(ruby, &instance) {
                        warn!("Error during poll: {:?}", e);
                    }
                }
            },
        );
        // Spawn a Ruby thread that runs the polling loop.
        let _thread: Value = thread_class.funcall_with_block("new", (), poll_proc)?;

        Ok(())
    }

    pub fn poll(ruby: &Ruby, this: &Self) -> Result<(), Error> {
        this.inner
            .poll(ruby)
            .map_err(|error| Error::new(ruby.exception_runtime_error(), error.to_string()))
    }

    pub fn feed(&self) {
        let inner = self.inner.clone();
        RUNTIME.spawn(async move {
            inner
                .run_in_ruby(Box::new(|r| {
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

#[derive(Debug, Error)]
enum BridgeError {
    #[error("An error occurred while polling: {0:#}")]
    Gvl(#[from] GvlError),

    #[error("Command was cancelled by Ruby runtime")]
    Cancelled,

    #[error("Caller went away")]
    CallerWentAway,

    #[error("Bridge was shutdown")]
    Shutdown,
}
