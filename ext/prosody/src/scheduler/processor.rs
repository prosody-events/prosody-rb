use crate::scheduler::cancellation::CancellationToken;
use crate::scheduler::result::ResultSender;
use crate::util::ThreadSafeValue;
use crate::{ROOT_MOD, id};
use educe::Educe;
use magnus::block::Proc;
use magnus::value::ReprValue;
use magnus::{Class, Error, Module, RClass, Ruby, Value};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::Relaxed;

#[derive(Clone, Educe)]
#[educe(Debug)]
pub struct RubyProcessor {
    processor: Arc<ThreadSafeValue>,
    is_shutdown: Arc<AtomicBool>,
}

impl RubyProcessor {
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

    pub fn submit<F>(
        &self,
        ruby: &Ruby,
        task_id: &str,
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

        let callback = result_receiver.into_proc(ruby);

        let mut maybe_function = Some(function);
        let block = Proc::from_fn(move |ruby, _, _| {
            if let Some(function) = maybe_function.take() {
                function(ruby)
            } else {
                Ok(())
            }
        });

        // def submit(task_id, callback, &task_block)
        let token = CancellationToken::new(self.processor.get(ruby).funcall_with_block(
            id!("submit"),
            (task_id, callback),
            block,
        )?);

        Ok(token)
    }

    pub fn stop(&self, ruby: &Ruby) -> Result<(), Error> {
        if !self.is_shutdown.fetch_or(true, Relaxed) {
            let _: Value = self.processor.get(ruby).funcall(id!("stop"), ())?;
        }

        Ok(())
    }
}
