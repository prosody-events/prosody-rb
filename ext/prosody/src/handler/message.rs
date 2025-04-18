use crate::{ROOT_MOD, id};
use educe::Educe;
use magnus::value::ReprValue;
use magnus::{Error, Module, RClass, Ruby, Value, method};
use prosody::consumer::Keyed;
use prosody::consumer::message::ConsumerMessage;
use serde_magnus::serialize;

#[derive(Educe, Clone)]
#[educe(Debug)]
#[magnus::wrap(class = "Prosody::Message", free_immediately, frozen_shareable, size)]
pub struct Message {
    #[educe(Debug(ignore))]
    inner: ConsumerMessage,
}

impl Message {
    fn topic(&self) -> &'static str {
        self.inner.topic().as_ref()
    }

    fn partition(&self) -> i32 {
        self.inner.partition()
    }

    fn offset(&self) -> i64 {
        self.inner.offset()
    }

    fn key(&self) -> &str {
        self.inner.key()
    }

    fn timestamp(ruby: &Ruby, this: &Self) -> Result<Value, Error> {
        let epoch_micros = this.inner.timestamp().timestamp_micros();
        ruby.module_kernel()
            .const_get::<_, RClass>(id!("Time"))?
            .funcall(id!("at"), (epoch_micros, ruby.to_symbol("microsecond")))
    }

    fn payload(_ruby: &Ruby, this: &Self) -> Result<Value, Error> {
        serialize(this.inner.payload())
    }
}

impl From<ConsumerMessage> for Message {
    fn from(value: ConsumerMessage) -> Self {
        Self { inner: value }
    }
}

pub fn init(ruby: &Ruby) -> Result<(), Error> {
    let module = ruby.get_inner(&ROOT_MOD);
    let class = module.define_class(id!("Message"), ruby.class_object())?;

    class.define_method(id!("topic"), method!(Message::topic, 0))?;
    class.define_method(id!("partition"), method!(Message::partition, 0))?;
    class.define_method(id!("offset"), method!(Message::offset, 0))?;
    class.define_method(id!("key"), method!(Message::key, 0))?;
    class.define_method(id!("timestamp"), method!(Message::timestamp, 0))?;
    class.define_method(id!("payload"), method!(Message::payload, 0))?;

    Ok(())
}
