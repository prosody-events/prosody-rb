use crate::bridge::Bridge;
use crate::result::{ProcessingError, ResultReceiver};
use crate::scheduler::SchedulerError;
use crate::scheduler::cancellation::CancellationToken;
use magnus::Ruby;

#[derive(Debug)]
pub struct TaskHandle {
    bridge: Bridge,
    result: ResultReceiver,
    token: CancellationToken,
}

impl TaskHandle {
    pub fn new(bridge: Bridge, result: ResultReceiver, token: CancellationToken) -> TaskHandle {
        Self {
            bridge,
            result,
            token,
        }
    }

    pub async fn cancel(self) -> Result<(), SchedulerError> {
        self.bridge
            .run_sync(move |ruby: &Ruby| {
                self.token
                    .cancel(ruby)
                    .map_err(|error| SchedulerError::Cancel(error.to_string()))
            })
            .await?
    }

    pub async fn result(self) -> Result<(), ProcessingError> {
        self.result.receive().await
    }
}
