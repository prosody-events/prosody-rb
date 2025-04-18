use crate::id;
use atomic_take::AtomicTake;
use educe::Educe;
use magnus::block::Proc;
use magnus::value::ReprValue;
use magnus::{Error, Ruby, TryConvert, Value, kwargs};
use prosody::consumer::failure::{ClassifyError, ErrorCategory};
use thiserror::Error;
use tokio::sync::oneshot;

pub fn result_channel() -> (ResultSender, ResultReceiver) {
    let (result_tx, result_rx) = oneshot::channel();
    let result_tx = AtomicTake::new(result_tx);
    (ResultSender { result_tx }, ResultReceiver { result_rx })
}

#[derive(Educe)]
#[educe(Debug)]
pub struct ResultSender {
    #[educe(Debug(ignore))]
    result_tx: AtomicTake<oneshot::Sender<Result<(), ProcessingError>>>,
}

#[derive(Educe)]
#[educe(Debug)]
pub struct ResultReceiver {
    #[educe(Debug(ignore))]
    result_rx: oneshot::Receiver<Result<(), ProcessingError>>,
}

impl ResultSender {
    pub fn send(&self, ruby: &Ruby, is_success: bool, result: Value) -> Result<(), Error> {
        let Some(result_tx) = self.result_tx.take() else {
            return Err(Error::new(
                ruby.exception_runtime_error(),
                "result was already sent",
            ));
        };

        if is_success {
            result_tx.send(Ok(())).map_err(|_| closed_error(ruby))?;
            return Ok(());
        }

        let is_permanent = result.funcall(id!("permanent?"), ()).unwrap_or(false);

        let error_string: String = result
            .funcall(id!("full_message"), (kwargs!("highlight" => false),))
            .or_else(|_| result.funcall(id!("inspect"), ()))
            .unwrap_or_else(|_| result.to_string());

        let error = if is_permanent {
            ProcessingError::Permanent(error_string)
        } else {
            ProcessingError::Transient(error_string)
        };

        result_tx.send(Err(error)).map_err(|_| closed_error(ruby))?;
        Ok(())
    }

    pub fn into_proc(self, ruby: &Ruby) -> Proc {
        ruby.proc_from_fn(move |ruby, args, _block| match args {
            [is_success, result, ..] => self.send(ruby, bool::try_convert(*is_success)?, *result),
            _ => Err(Error::new(
                ruby.exception_arg_error(),
                format!("Expected two arguments but received {}", args.len()),
            )),
        })
    }
}

impl ResultReceiver {
    pub async fn receive(self) -> Result<(), ProcessingError> {
        self.result_rx.await.map_err(|_| ProcessingError::Closed)?
    }
}

#[derive(Clone, Debug, Error)]
pub enum ProcessingError {
    #[error("transient error: {0}")]
    Transient(String),

    #[error("permanent error: {0}")]
    Permanent(String),

    #[error("result channel has been closed")]
    Closed,
}

impl ClassifyError for ProcessingError {
    fn classify_error(&self) -> ErrorCategory {
        match self {
            ProcessingError::Transient(_) | ProcessingError::Closed => ErrorCategory::Transient,
            ProcessingError::Permanent(_) => ErrorCategory::Permanent,
        }
    }
}

fn closed_error(ruby: &Ruby) -> Error {
    Error::new(
        ruby.exception_runtime_error(),
        "result channel has been closed",
    )
}
