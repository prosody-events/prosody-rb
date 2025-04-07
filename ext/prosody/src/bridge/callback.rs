use crate::id;
use magnus::value::{BoxValue, ReprValue};
use magnus::{Error, IntoValue, Ruby, Value};

pub struct AsyncCallback {
    queue: BoxValue<Value>,
}

// SAFETY: The underlying queue can only be pushed to from a Ruby thread.
unsafe impl Send for AsyncCallback {}

// SAFETY: The underlying queue can only be pushed to from a Ruby thread.
unsafe impl Sync for AsyncCallback {}

impl AsyncCallback {
    pub fn from_queue(queue: Value) -> Self {
        Self {
            queue: BoxValue::new(queue),
        }
    }

    pub fn complete<V>(self, ruby: &Ruby, value: V) -> Result<(), Error>
    where
        V: IntoValue,
    {
        self.queue
            .funcall(id!("push"), (value.into_value_with(ruby),))
            .map(|_: Value| ())
    }
}
