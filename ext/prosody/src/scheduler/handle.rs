use crate::scheduler::cancellation::CancellationToken;
use crate::scheduler::result::ResultReceiver;

#[derive(Debug)]
pub struct TaskHandle {
    pub result: ResultReceiver,
    pub cancellation_token: CancellationToken,
}

impl TaskHandle {
    pub fn new(result: ResultReceiver, cancellation_token: CancellationToken) -> TaskHandle {
        Self {
            result,
            cancellation_token,
        }
    }
}
