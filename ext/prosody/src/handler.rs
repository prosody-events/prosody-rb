use prosody::consumer::failure::{ClassifyError, ErrorCategory, FallibleHandler};
use prosody::consumer::message::{ConsumerMessage, MessageContext};
use thiserror::Error;

#[derive(Clone, Debug)]
pub struct RubyHandler {}

impl FallibleHandler for RubyHandler {
    type Error = RubyHandlerError;

    async fn on_message(
        &self,
        context: MessageContext,
        message: ConsumerMessage,
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
