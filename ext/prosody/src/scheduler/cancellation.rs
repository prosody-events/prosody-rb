use crate::bridge::Bridge;
use crate::id;
use crate::scheduler::SchedulerError;
use crate::util::ThreadSafeValue;
use magnus::value::ReprValue;
use magnus::{Ruby, Value};

#[derive(Debug)]
pub struct CancellationToken {
    token: ThreadSafeValue,
}

impl CancellationToken {
    pub fn new(token: Value) -> Self {
        Self {
            token: ThreadSafeValue::new(token),
        }
    }

    pub async fn cancel(self, bridge: &Bridge) -> Result<(), SchedulerError> {
        bridge
            .run(move |ruby: &Ruby| {
                self.token
                    .get(ruby)
                    .funcall::<_, _, Value>(id!("cancel"), ())
                    .map_err(|error| SchedulerError::Cancel(error.to_string()))?;

                Ok(())
            })
            .await?
    }
}
