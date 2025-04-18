use magnus::{Error, Ruby};
use prosody::consumer::failure::{ClassifyError, ErrorCategory, FallibleHandler};
use prosody::consumer::message::{ConsumerMessage, MessageContext};
use thiserror::Error;

mod context;
mod message;

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

pub fn init(ruby: &Ruby) -> Result<(), Error> {
    context::init(ruby)?;
    message::init(ruby)?;

    Ok(())
}
