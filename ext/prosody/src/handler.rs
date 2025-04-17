use crate::id;
use magnus::value::Lazy;
use magnus::{Module, RClass};
use prosody::consumer::failure::{ClassifyError, ErrorCategory, FallibleHandler};
use prosody::consumer::message::{ConsumerMessage, MessageContext};
use thiserror::Error;

#[allow(clippy::expect_used)]
static TASK_PROCESSOR: Lazy<RClass> = Lazy::new(|ruby| {
    ruby.class_object()
        .const_get(id!("Prosody::AsyncTaskProcessor"))
        .expect("Failed to load Prosody::AsyncTaskProcessor class")
});

#[derive(Clone, Debug)]
pub struct RubyHandler {}

impl FallibleHandler for RubyHandler {
    type Error = RubyHandlerError;

    async fn on_message(
        &self,
        _context: MessageContext,
        _message: ConsumerMessage,
    ) -> Result<(), Self::Error> {
        todo!()
    }
}

impl ClassifyError for RubyHandlerError {
    fn classify_error(&self) -> ErrorCategory {
        todo!()
    }
}

#[derive(Debug, Error)]
pub enum RubyHandlerError {}
