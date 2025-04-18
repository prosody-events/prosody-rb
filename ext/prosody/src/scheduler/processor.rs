use crate::scheduler::cancellation::CancellationToken;
use crate::scheduler::result::ResultSender;
use crate::util::ThreadSafeValue;
use crate::{ROOT_MOD, id};
use educe::Educe;
use magnus::block::Proc;
use magnus::value::ReprValue;
use magnus::{Class, Error, Module, RClass, Ruby, Value};
use std::sync::Arc;

#[derive(Clone, Educe)]
#[educe(Debug)]
pub struct RubyProcessor(Arc<ThreadSafeValue>);

impl RubyProcessor {
    pub fn new(ruby: &Ruby) -> Result<Self, Error> {
        let class: RClass = ruby
            .get_inner(&ROOT_MOD)
            .const_get(id!("AsyncTaskProcessor"))?;

        let instance: Value = class.new_instance(())?; // todo: pass in logger
        let _: Value = instance.funcall(id!("start"), ())?;
        Ok(Self(Arc::new(ThreadSafeValue::new(instance))))
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
        let token = CancellationToken::new(self.0.get(ruby).funcall_with_block(
            id!("submit"),
            (task_id, callback),
            block,
        )?);

        Ok(token)
    }

    pub fn stop(&self, ruby: &Ruby) -> Result<(), Error> {
        let _: Value = self.0.get(ruby).funcall(id!("stop"), ())?;
        Ok(())
    }
}
