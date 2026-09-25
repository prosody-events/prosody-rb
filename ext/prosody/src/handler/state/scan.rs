//! Concrete typed cursors for native state scans.
//!
//! Cancellation can discard an orphaned chunk. The attempt then closes the
//! cursor through the Ruby `ensure`, so no later operation observes that chunk.

use super::state_error;
use crate::bridge::{Bridge, QUEUE_CLASS};
use crate::handler::message::Message;
use crate::id;
use crate::tracing_util::extract_opentelemetry_context;
use crate::util::ThreadSafeValue;
use magnus::value::ReprValue;
use magnus::{Error, IntoValue, Ruby, Value};
use opentelemetry::propagation::TextMapCompositePropagator;
use opentelemetry::trace::FutureExt;
use prosody::consumer::event_context::{ErasedStateError, StateCursor};
use prosody::consumer::message::ConsumerMessage;
use serde_json::Value as JsonValue;
use serde_magnus::serialize;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::num::NonZeroUsize;
use std::sync::Arc;
use tracing::Span;

#[allow(clippy::unwrap_used, reason = "256 is a nonzero literal")]
const SCAN_READY_CHUNK_SIZE: NonZeroUsize = NonZeroUsize::new(256).unwrap();

/// Maps a failed cursor read to a Ruby error.
///
/// Owned handles raise the typed state errors. Published readers raise
/// `RuntimeError`, the same class as their point reads.
type ReadError = fn(&Ruby, &ErasedStateError) -> Error;

struct ScanInner<T> {
    cursor: Arc<StateCursor<T>>,
    buffer: VecDeque<T>,
    done: bool,
}

macro_rules! drive_scan {
    ($ruby:expr, $this:expr, $inner:expr, |$item:ident| $convert:block) => {{
        loop {
            if let Some($item) = $inner.buffer.pop_front() {
                return $convert;
            }
            if $inner.done {
                return Ok($ruby.qnil().as_value());
            }
            let cursor = Arc::clone(&$inner.cursor);
            let context = extract_opentelemetry_context($ruby, &$this.propagator)?;
            let chunk = $this
                .bridge
                .wait_for(
                    $ruby,
                    async move {
                        cursor
                            .next_ready_chunk(SCAN_READY_CHUNK_SIZE)
                            .with_context(context)
                            .await
                    },
                    Span::current(),
                )?
                .map_err(|error| ($this.read_error)($ruby, &error))?;
            match chunk {
                Some(items) => $inner.buffer.extend(items),
                None => {
                    $inner.done = true;
                    let cursor = Arc::clone(&$inner.cursor);
                    $this.bridge.wait_for(
                        $ruby,
                        async move { cursor.close().await },
                        Span::current(),
                    )?;
                    return Ok($ruby.qnil().as_value());
                }
            }
        }
    }};
}

macro_rules! native_scan {
    ($name:ident, $class:literal, $item:ty, |$ruby:ident, $item_name:ident| $convert:block) => {
        /// Native cursor with one item type.
        #[magnus::wrap(class = $class)]
        pub struct $name {
            inner: RefCell<ScanInner<$item>>,
            lock: ThreadSafeValue,
            bridge: Bridge,
            propagator: Arc<TextMapCompositePropagator>,
            read_error: ReadError,
        }

        impl $name {
            /// Wraps an owned-handle cursor. Read failures raise the typed
            /// state errors.
            pub(super) fn new(
                ruby: &Ruby,
                cursor: StateCursor<$item>,
                bridge: Bridge,
                propagator: Arc<TextMapCompositePropagator>,
            ) -> Result<Self, Error> {
                Self::with_read_error(ruby, cursor, bridge, propagator, state_error)
            }

            fn with_read_error(
                ruby: &Ruby,
                cursor: StateCursor<$item>,
                bridge: Bridge,
                propagator: Arc<TextMapCompositePropagator>,
                read_error: ReadError,
            ) -> Result<Self, Error> {
                let queue: Value = ruby.get_inner(&QUEUE_CLASS).funcall(id!(ruby, "new"), ())?;
                let _: Value = queue.funcall(id!(ruby, "push"), (ruby.qnil(),))?;
                Ok(Self {
                    inner: RefCell::new(ScanInner {
                        cursor: Arc::new(cursor),
                        buffer: VecDeque::new(),
                        done: false,
                    }),
                    lock: ThreadSafeValue::new(queue, bridge.clone()),
                    bridge,
                    propagator,
                    read_error,
                })
            }

            fn acquire(&self, ruby: &Ruby) -> Result<(), Error> {
                let _: Value = self.lock.get(ruby).funcall(id!(ruby, "pop"), ())?;
                Ok(())
            }

            fn release(&self, ruby: &Ruby) {
                let _: Result<Value, Error> = self
                    .lock
                    .get(ruby)
                    .funcall(id!(ruby, "push"), (ruby.qnil(),));
            }

            pub(super) fn next(ruby: &Ruby, this: &Self) -> Result<Value, Error> {
                this.acquire(ruby)?;
                let out = Self::next_locked(ruby, this);
                this.release(ruby);
                out
            }

            fn next_locked(ruby: &Ruby, this: &Self) -> Result<Value, Error> {
                let inner = &mut *this.inner.borrow_mut();
                drive_scan!(ruby, this, inner, |$item_name| {
                    let $ruby = ruby;
                    $convert
                })
            }

            pub(super) fn close(ruby: &Ruby, this: &Self) -> Result<Value, Error> {
                this.acquire(ruby)?;
                let out = Self::close_locked(ruby, this);
                this.release(ruby);
                out
            }

            fn close_locked(ruby: &Ruby, this: &Self) -> Result<Value, Error> {
                let inner = &mut *this.inner.borrow_mut();
                inner.done = true;
                inner.buffer.clear();
                let cursor = Arc::clone(&inner.cursor);
                this.bridge
                    .wait_for(ruby, async move { cursor.close().await }, Span::current())?;
                Ok(ruby.qnil().as_value())
            }
        }
    };
}

native_scan!(
    NativeJsonDequeScan,
    "Prosody::NativeJsonDequeScan",
    JsonValue,
    |ruby, item| { serialize(ruby, &item) }
);
native_scan!(
    NativeJsonMapScan,
    "Prosody::NativeJsonMapScan",
    (String, JsonValue),
    |ruby, item| {
        let (key, value) = item;
        let value: Value = serialize(ruby, &value)?;
        Ok((key, value).into_value_with(ruby))
    }
);
native_scan!(
    NativeMessageDequeScan,
    "Prosody::NativeMessageDequeScan",
    ConsumerMessage<JsonValue>,
    |ruby, item| { Ok(Message::from(item).into_value_with(ruby)) }
);
native_scan!(
    NativeMessageMapScan,
    "Prosody::NativeMessageMapScan",
    (String, ConsumerMessage<JsonValue>),
    |ruby, item| {
        let (key, message) = item;
        Ok((key, Message::from(message)).into_value_with(ruby))
    }
);
native_scan!(
    NativeMapKeyScan,
    "Prosody::NativeMapKeyScan",
    String,
    |ruby, item| { Ok(item.into_value_with(ruby)) }
);

pub(crate) fn published_map_scan(
    ruby: &Ruby,
    cursor: StateCursor<(String, JsonValue)>,
    bridge: Bridge,
    propagator: Arc<TextMapCompositePropagator>,
) -> Result<NativeJsonMapScan, Error> {
    NativeJsonMapScan::with_read_error(ruby, cursor, bridge, propagator, published_error)
}

pub(crate) fn published_map_key_scan(
    ruby: &Ruby,
    cursor: StateCursor<String>,
    bridge: Bridge,
    propagator: Arc<TextMapCompositePropagator>,
) -> Result<NativeMapKeyScan, Error> {
    NativeMapKeyScan::with_read_error(ruby, cursor, bridge, propagator, published_error)
}

pub(crate) fn published_deque_scan(
    ruby: &Ruby,
    cursor: StateCursor<JsonValue>,
    bridge: Bridge,
    propagator: Arc<TextMapCompositePropagator>,
) -> Result<NativeJsonDequeScan, Error> {
    NativeJsonDequeScan::with_read_error(ruby, cursor, bridge, propagator, published_error)
}

/// Raises a published-reader read failure as `RuntimeError`.
fn published_error(ruby: &Ruby, error: &ErasedStateError) -> Error {
    Error::new(ruby.exception_runtime_error(), error.message().to_owned())
}
