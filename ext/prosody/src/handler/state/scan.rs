//! Concrete typed cursors for native state scans.
//!
//! Cancellation can discard an orphaned chunk. The attempt then closes the
//! cursor through the Ruby `ensure`, so no later operation observes that chunk.

use super::state_error;
use crate::bridge::Bridge;
use crate::handler::message::Message;
use crate::tracing_util::extract_opentelemetry_context;
use magnus::{Error, IntoValue, RArray, Ruby, Value};
use opentelemetry::propagation::TextMapCompositePropagator;
use opentelemetry::trace::FutureExt;
use prosody::consumer::event_context::StateCursor;
use prosody::consumer::message::ConsumerMessage;
use serde_json::Value as JsonValue;
use serde_magnus::serialize;
use std::num::NonZeroUsize;
use std::sync::Arc;
use tracing::Span;

const SCAN_READY_CHUNK_SIZE: NonZeroUsize = match NonZeroUsize::new(256) {
    Some(size) => size,
    None => NonZeroUsize::MIN,
};

macro_rules! native_scan {
    ($name:ident, $class:literal, $item:ty, |$ruby:ident, $item_name:ident| $convert:block) => {
        /// Native cursor with one item type.
        #[magnus::wrap(class = $class)]
        pub struct $name {
            cursor: Arc<StateCursor<$item>>,
            bridge: Bridge,
            propagator: Arc<TextMapCompositePropagator>,
        }

        impl $name {
            pub(crate) fn new(
                cursor: StateCursor<$item>,
                bridge: Bridge,
                propagator: Arc<TextMapCompositePropagator>,
            ) -> Self {
                Self {
                    cursor: Arc::new(cursor),
                    bridge,
                    propagator,
                }
            }

            /// Returns the next ready chunk as an Array, or `nil` after the
            /// end. Read failures raise the typed state errors.
            pub(super) fn next_chunk($ruby: &Ruby, this: &Self) -> Result<Option<RArray>, Error> {
                let cursor = Arc::clone(&this.cursor);
                let context = extract_opentelemetry_context($ruby, &this.propagator)?;
                let pull = async move {
                    cursor
                        .next_ready_chunk(SCAN_READY_CHUNK_SIZE)
                        .with_context(context)
                        .await
                };
                this.bridge
                    .wait_for($ruby, pull, Span::current())?
                    .map_err(|error| state_error($ruby, &error))?
                    .map(|items| {
                        $ruby.ary_try_from_iter(items.into_iter().map(|$item_name| $convert))
                    })
                    .transpose()
            }

            pub(super) fn close(ruby: &Ruby, this: &Self) -> Result<(), Error> {
                let cursor = Arc::clone(&this.cursor);
                this.bridge
                    .wait_for(ruby, async move { cursor.close().await }, Span::current())
            }
        }
    };
}

native_scan!(
    NativeJsonDequeScan,
    "Prosody::NativeJsonDequeScan",
    JsonValue,
    |ruby, item| { serialize::<_, Value>(ruby, &item) }
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
