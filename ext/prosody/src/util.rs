use magnus::value::BoxValue;
use magnus::{Ruby, Value};

#[macro_export]
macro_rules! id {
    ($str:expr) => {{
        static VAL: magnus::value::LazyId = magnus::value::LazyId::new($str);
        *VAL
    }};
}

#[derive(Debug)]
pub struct ThreadSafeValue(BoxValue<Value>);

// SAFETY: The underlying value can only be accessed from a Ruby thread.
unsafe impl Send for ThreadSafeValue {}

// SAFETY: The underlying value can only be accessed from a Ruby thread.
unsafe impl Sync for ThreadSafeValue {}

impl ThreadSafeValue {
    pub fn new(value: Value) -> Self {
        Self(BoxValue::new(value))
    }

    pub fn get(&self, _ruby: &Ruby) -> &Value {
        &self.0
    }
}
