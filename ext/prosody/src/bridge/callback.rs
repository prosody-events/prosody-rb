use crate::id;
use crate::util::ThreadSafeValue;
use magnus::value::ReprValue;
use magnus::{Error, IntoValue, Ruby, Value};

pub struct AsyncCallback {
    queue: ThreadSafeValue,
}

impl AsyncCallback {
    pub fn from_queue(queue: Value) -> Self {
        Self {
            queue: ThreadSafeValue::new(queue),
        }
    }

    pub fn complete<V>(self, ruby: &Ruby, value: V) -> Result<(), Error>
    where
        V: IntoValue,
    {
        self.queue
            .get(ruby)
            .funcall(id!("push"), (value.into_value_with(ruby),))
            .map(|_: Value| ())
    }
}
