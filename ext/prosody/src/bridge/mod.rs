//! # Bridge Module
//!
//! Facilitates communication between Ruby and Rust, particularly for handling
//! asynchronous operations. This module creates a dedicated Ruby thread that
//! processes functions sent from Rust, allowing async Rust code to interact
//! safely with the Ruby runtime.

use crate::bridge::callback::AsyncCallback;
use crate::gvl::{GvlError, without_gvl};
use crate::{ROOT_MOD, RUNTIME, id};
use atomic_take::AtomicTake;
use educe::Educe;
use futures::executor::block_on;
use magnus::value::{Lazy, ReprValue};
use magnus::{Error, Module, RClass, Ruby, Value};
use std::any::Any;
use thiserror::Error;
use tokio::select;
use tokio::sync::mpsc::{Receiver, Sender, channel};
use tokio::sync::oneshot;
use tracing::{Instrument, Span, debug, error, warn};

/// Helper module for handling callbacks from async Rust code to Ruby.
mod callback;

/// Maximum number of commands to process in a single poll operation.
const POLL_BATCH_SIZE: usize = 16;

/// Lazily initialized reference to Ruby's Thread class.
#[allow(clippy::expect_used)]
pub static THREAD_CLASS: Lazy<RClass> = Lazy::new(|ruby| {
    ruby.class_object()
        .const_get(id!(ruby, "Thread"))
        .expect("Failed to load Thread class")
});

/// Lazily initialized reference to Ruby's `Thread::Queue` class.
#[allow(clippy::expect_used)]
pub static QUEUE_CLASS: Lazy<RClass> = Lazy::new(|ruby| {
    ruby.get_inner(&THREAD_CLASS)
        .const_get(id!(ruby, "Queue"))
        .expect("Failed to load Queue class")
});

/// Type alias for a boxed closure that can be executed in a Ruby context.
type RubyFunction = Box<dyn FnOnce(&Ruby) + Send>;

/// A wrapper for results returned from async operations to Ruby.
///
/// Uses `AtomicTake` to ensure the value can only be extracted once.
#[derive(Debug)]
#[magnus::wrap(class = "Prosody::DynamicResult")]
struct DynamicResult(AtomicTake<Box<dyn Any + Send>>);

/// Bridge between Ruby and Rust for async operations.
///
/// Creates a dedicated Ruby thread that processes functions sent from Rust,
/// allowing async Rust code to interact safely with the Ruby runtime.
#[derive(Educe, Clone)]
#[educe(Debug)]
pub struct Bridge {
    /// Channel sender for submitting functions to be executed in the Ruby
    /// context.
    #[educe(Debug(ignore))]
    tx: Sender<RubyFunction>,
}

impl Bridge {
    /// Creates a new Bridge with a specified buffer size.
    ///
    /// # Arguments
    ///
    /// * `ruby` - Reference to the Ruby VM.
    /// * `buffer_size` - Size of the internal channel buffer for pending
    ///   operations.
    ///
    /// # Returns
    ///
    /// A new `Bridge` instance.
    pub fn new(ruby: &Ruby, buffer_size: usize) -> Self {
        let (tx, mut rx) = channel(buffer_size);

        // Create a dedicated Ruby thread to process functions
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

    /// Runs a function in the Ruby context and returns its result
    /// asynchronously.
    ///
    /// # Arguments
    ///
    /// * `function` - Closure to run in the Ruby context.
    ///
    /// # Returns
    ///
    /// The result of the function wrapped in a `Result`.
    ///
    /// # Errors
    ///
    /// Returns `BridgeError::Shutdown` if the bridge has been shut down.
    pub async fn run<F, T>(&self, function: F) -> Result<T, BridgeError>
    where
        F: FnOnce(&Ruby) -> T + Send + 'static,
        T: Send + 'static,
    {
        let (result_tx, result_rx) = oneshot::channel();

        // Wrap the function to capture its result
        let function = |ruby: &Ruby| {
            let result = function(ruby);
            if result_tx.send(result).is_err() {
                error!("failed to send Ruby bridge result");
            }
        };

        self.tx
            .send(Box::new(function))
            .await
            .map_err(|_| BridgeError::Shutdown)?;

        result_rx.await.map_err(|_| BridgeError::Shutdown)
    }

    /// Waits for a future to complete and returns its result to Ruby.
    ///
    /// This method bridges asynchronous Rust code with synchronous Ruby code
    /// by:
    /// 1. Creating a Ruby Queue to receive the result
    /// 2. Spawning a Rust task to await the future
    /// 3. Yielding until the result is pushed to the Queue
    ///
    /// # Arguments
    ///
    /// * `ruby` - Reference to the Ruby VM.
    /// * `future` - Future to await.
    ///
    /// # Returns
    ///
    /// The output of the future.
    ///
    /// # Errors
    ///
    /// Returns a Magnus error if there's an issue with the Ruby interaction or
    /// type conversion.
    pub fn wait_for<Fut>(&self, ruby: &Ruby, future: Fut, span: Span) -> Result<Fut::Output, Error>
    where
        Fut: Future + Send + 'static,
        Fut::Output: Send,
    {
        let cloned_span = span.clone();
        let _enter = cloned_span.enter();

        // Create a Ruby Queue to receive the result
        let queue: Value = ruby.get_inner(&QUEUE_CLASS).funcall(id!(ruby, "new"), ())?;
        let callback = AsyncCallback::from_queue(queue);
        let tx = self.tx.clone();

        // Spawn a task to await the future and send the result back to Ruby
        RUNTIME.spawn(
            async move {
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
            }
            .instrument(span),
        );

        // Yield until the result is available, then extract and return it
        queue
            .funcall::<_, _, &DynamicResult>(id!(ruby, "pop"), ())?
            .0
            .take()
            .ok_or_else(|| Error::new(ruby.exception_runtime_error(), "result already taken"))?
            .downcast::<Fut::Output>()
            .map_err(|_| Error::new(ruby.exception_runtime_error(), "failed to downcast result"))
            .map(|output| *output)
    }
}

/// Polls for and executes functions in the Ruby context.
///
/// This function processes commands from the receiver channel, allowing them to
/// be executed in the Ruby context. Since the GVL is expensive to acquire, this
/// executes multiple commands in a single poll operation to reduce the
/// overhead.
///
/// # Arguments
///
/// * `rx` - Receiver for commands to execute.
/// * `ruby` - Reference to the Ruby VM.
///
/// # Returns
///
/// `Ok(())` if commands were processed successfully.
///
/// # Errors
///
/// Returns a `BridgeError` if there was an issue with polling or executing
/// commands.
fn poll(rx: &mut Receiver<RubyFunction>, ruby: &Ruby) -> Result<(), BridgeError> {
    // Set up cancellation channel
    let (cancel_tx, cancel_rx) = oneshot::channel();
    let mut maybe_cancel_tx = Some(cancel_tx);

    // Function to be executed without the GVL (Global VM Lock)
    let poll_fn = || {
        // Wait for either a command or cancellation
        let maybe_command = block_on(async {
            select! {
                _ = cancel_rx => Err(BridgeError::Cancelled),
                result = rx.recv() => Ok(result),
            }
        })?;

        let first_command = maybe_command.ok_or(BridgeError::Shutdown)?;

        // Batch up to POLL_BATCH_SIZE commands
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

    // Function to cancel polling if needed
    let cancel_fn = || {
        let Some(cancel_tx) = maybe_cancel_tx.take() else {
            return;
        };

        if cancel_tx.send(()).is_err() {
            warn!("Failed to cancel poll operation");
        }
    };

    // Execute poll function without the GVL
    let commands = without_gvl(poll_fn, cancel_fn)??;

    // Execute each command in the Ruby context
    for command in commands {
        command(ruby);
    }

    Ok(())
}

/// Errors that can occur during bridge operations.
#[derive(Debug, Error)]
pub enum BridgeError {
    /// An error occurred while polling with the GVL released.
    #[error("an error occurred while polling: {0:#}")]
    Gvl(#[from] GvlError),

    /// Failed to create an async task.
    #[error("failed to create async task: {0:#}")]
    TaskCreate(String),

    /// An async task was dropped before completion.
    #[error("async task was dropped before completion")]
    TaskDropped,

    /// A command was cancelled by the Ruby runtime.
    #[error("command was cancelled by Ruby runtime")]
    Cancelled,

    /// The Ruby bridge was shut down.
    #[error("Ruby bridge was shutdown")]
    Shutdown,
}

/// Initializes the bridge module in Ruby.
///
/// # Arguments
///
/// * `ruby` - Reference to the Ruby VM.
///
/// # Returns
///
/// `Ok(())` if initialization was successful.
///
/// # Errors
///
/// Returns a Magnus error if there's an issue with class definition.
pub fn init(ruby: &Ruby) -> Result<(), Error> {
    let module = ruby.get_inner(&ROOT_MOD);
    module.define_class("DynamicResult", ruby.class_object())?;

    Ok(())
}
