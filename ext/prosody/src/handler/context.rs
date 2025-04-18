use crate::{ROOT_MOD, id};
use educe::Educe;
use magnus::{Error, Module, Ruby};
use prosody::consumer::message::MessageContext;

#[derive(Educe, Clone)]
#[educe(Debug)]
#[magnus::wrap(class = "Prosody::Context", free_immediately, frozen_shareable, size)]
pub struct Context {
    #[educe(Debug(ignore))]
    inner: MessageContext,
}

impl From<MessageContext> for Context {
    fn from(value: MessageContext) -> Self {
        Self { inner: value }
    }
}

pub fn init(ruby: &Ruby) -> Result<(), Error> {
    let module = ruby.get_inner(&ROOT_MOD);
    let class = module.define_class(id!("Context"), ruby.class_object())?;

    Ok(())
}
