use crate::bridge::{Bridge, BridgeError};
use crate::handler::context::Context;
use crate::handler::message::Message;
use crate::id;
use crate::scheduler::result::ProcessingError;
use crate::scheduler::{Scheduler, SchedulerError};
use crate::util::ThreadSafeValue;
use futures::pin_mut;
use magnus::value::ReprValue;
use magnus::{Error, Ruby, Value};
use prosody::consumer::failure::{ClassifyError, ErrorCategory, FallibleHandler};
use prosody::consumer::message::{ConsumerMessage, MessageContext};
use std::sync::Arc;
use thiserror::Error;
use tokio::select;

mod context;
mod message;

#[derive(Clone, Debug)]
pub struct RubyHandler {
    bridge: Bridge,
    scheduler: Scheduler,
    handler: Arc<ThreadSafeValue>,
}

impl RubyHandler {
    pub fn new(bridge: Bridge, ruby: &Ruby, handler_instance: Value) -> Result<Self, Error> {
        Ok(Self {
            bridge: bridge.clone(),
            scheduler: Scheduler::new(ruby, bridge)?,
            handler: Arc::new(ThreadSafeValue::new(handler_instance)),
        })
    }
}

impl FallibleHandler for RubyHandler {
    type Error = RubyHandlerError;

    async fn on_message(
        &self,
        context: MessageContext,
        message: ConsumerMessage,
    ) -> Result<(), Self::Error> {
        let shutdown_future = context.on_shutdown();
        let handler = self.handler.clone();
        let task_id = format!(
            "{}/{}:{}",
            message.topic(),
            message.partition(),
            message.offset()
        );
        let context: Context = context.into();
        let message: Message = message.into();

        let task_handle = self
            .scheduler
            .schedule(task_id, move |ruby| {
                let _: Value = handler
                    .get(ruby)
                    .funcall(id!("on_message"), (context, message))?;

                Ok(())
            })
            .await?;

        let result_future = task_handle.result.receive();
        pin_mut!(result_future);

        select! {
            result = &mut result_future => {
                result?;
            }
            () = shutdown_future => {
                task_handle.cancellation_token.cancel(&self.bridge).await?;
                result_future.await?;
            }
        }

        Ok(())
    }
}

impl ClassifyError for RubyHandlerError {
    fn classify_error(&self) -> ErrorCategory {
        match self {
            RubyHandlerError::Scheduler(_) | RubyHandlerError::Bridge(_) => {
                ErrorCategory::Transient
            }
            RubyHandlerError::Processing(error) => error.classify_error(),
        }
    }
}

#[derive(Debug, Error)]
pub enum RubyHandlerError {
    #[error(transparent)]
    Scheduler(#[from] SchedulerError),

    #[error(transparent)]
    Bridge(#[from] BridgeError),

    #[error(transparent)]
    Processing(#[from] ProcessingError),
}

pub fn init(ruby: &Ruby) -> Result<(), Error> {
    context::init(ruby)?;
    message::init(ruby)?;

    Ok(())
}
