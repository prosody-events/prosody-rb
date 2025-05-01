//! Provides functionality to schedule and execute Rust functions within the
//! Ruby runtime.
//!
//! This module contains the implementation of `RubyProcessor`, which interfaces
//! with a Ruby `AsyncTaskProcessor` class to manage task execution,
//! cancellation, and lifecycle management.

use crate::scheduler::cancellation::CancellationToken;
use crate::scheduler::result::ResultSender;
use crate::util::ThreadSafeValue;
use crate::{ROOT_MOD, id};
use educe::Educe;
use magnus::block::Proc;
use magnus::value::ReprValue;
use magnus::{Class, Error, Module, RClass, RHash, Ruby, Value};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::Relaxed;

/// A thread-safe wrapper around a Ruby `AsyncTaskProcessor` instance.
///
/// This struct provides an interface for submitting Rust functions to be
/// executed in the Ruby runtime, managing task lifecycle, and handling task
/// cleanup upon shutdown.
#[derive(Clone, Educe)]
#[educe(Debug)]
pub struct RubyProcessor {
    /// Thread-safe reference to the Ruby `AsyncTaskProcessor` instance
    processor: Arc<ThreadSafeValue>,

    /// Flag indicating whether the processor is in shutdown state
    is_shutdown: Arc<AtomicBool>,
}

impl RubyProcessor {
    /// Creates a new `RubyProcessor` instance.
    ///
    /// Instantiates a Ruby `AsyncTaskProcessor` class and starts it, making it
    /// ready to accept tasks.
    ///
    /// # Arguments
    ///
    /// * `ruby` - Reference to the Ruby VM
    ///
    /// # Returns
    ///
    /// A new `RubyProcessor` instance
    ///
    /// # Errors
    ///
    /// Returns a Magnus error if:
    /// - The `AsyncTaskProcessor` Ruby class cannot be found
    /// - Instantiation of the processor fails
    /// - Starting the processor fails
    pub fn new(ruby: &Ruby) -> Result<Self, Error> {
        let class: RClass = ruby
            .get_inner(&ROOT_MOD)
            .const_get(id!("AsyncTaskProcessor"))?;

        let instance: Value = class.new_instance(())?; // todo: pass in logger
        let _: Value = instance.funcall(id!("start"), ())?;
        Ok(Self {
            processor: Arc::new(ThreadSafeValue::new(instance)),
            is_shutdown: Arc::default(),
        })
    }

    /// Submits a function to be executed in the Ruby runtime.
    ///
    /// This method packages a Rust function along with metadata and callback
    /// mechanisms to be executed by the Ruby processor. It handles
    /// propagation of tracing context and ensures proper result handling.
    ///
    /// # Arguments
    ///
    /// * `ruby` - Reference to the Ruby VM
    /// * `task_id` - Unique identifier for the task
    /// * `carrier` - OpenTelemetry context carrier for trace propagation
    /// * `result_receiver` - Channel for reporting task execution results
    /// * `function` - The Rust function to execute in Ruby's context
    ///
    /// # Returns
    ///
    /// A `CancellationToken` that can be used to cancel the task if needed
    ///
    /// # Errors
    ///
    /// Returns a Magnus error if:
    /// - The processor is shutting down
    /// - The function submission to Ruby fails
    pub fn submit<F>(
        &self,
        ruby: &Ruby,
        task_id: &str,
        carrier: HashMap<String, String>,
        result_receiver: ResultSender,
        function: F,
    ) -> Result<CancellationToken, Error>
    where
        F: FnOnce(&Ruby) -> Result<(), Error> + Send + 'static,
    {
        if self.is_shutdown.load(Relaxed) {
            return Err(Error::new(
                ruby.exception_runtime_error(),
                String::from("Processor is shutting down"),
            ));
        }

        // Convert the result sender into a Ruby callback
        let callback = result_receiver.into_proc(ruby);

        // Wrap the function in a Ruby block that can be executed later
        let mut maybe_function = Some(function);
        let block = Proc::from_fn(move |ruby, _, _| {
            if let Some(function) = maybe_function.take() {
                function(ruby)
            } else {
                Ok(())
            }
        });

        // Convert the tracing context carrier to a Ruby hash
        let carrier: RHash = carrier.into_iter().collect();

        // Call the Ruby method: def submit(task_id, callback, &task_block)
        let token = CancellationToken::new(self.processor.get(ruby).funcall_with_block(
            id!("submit"),
            (task_id, carrier, callback),
            block,
        )?);

        Ok(token)
    }

    /// Stops the processor, preventing it from accepting new tasks.
    ///
    /// This method is idempotent - calling it multiple times will only
    /// stop the processor once.
    ///
    /// # Arguments
    ///
    /// * `ruby` - Reference to the Ruby VM
    ///
    /// # Errors
    ///
    /// Returns a Magnus error if the stop operation fails in Ruby
    pub fn stop(&self, ruby: &Ruby) -> Result<(), Error> {
        if !self.is_shutdown.fetch_or(true, Relaxed) {
            let _: Value = self.processor.get(ruby).funcall(id!("stop"), ())?;
        }

        Ok(())
    }
}
