use crate::id;
use crate::util::ThreadSafeValue;
use magnus::value::ReprValue;
use magnus::{Error, Ruby, Value};

#[derive(Debug)]
pub struct CancellationToken(ThreadSafeValue);

impl CancellationToken {
    pub fn new(token: Value) -> Self {
        Self(ThreadSafeValue::new(token))
    }

    pub fn cancel(self, ruby: &Ruby) -> Result<(), Error> {
        let _: Value = self.0.get(ruby).funcall(id!("cancel"), ())?;
        Ok(())
    }
}
