//! # Logging Bridge
//!
//! This module provides a bridge between Rust's tracing infrastructure and
//! Ruby's logging system. It captures tracing events from Rust code and
//! forwards them to a Ruby logger, ensuring logs from the native extension are
//! properly integrated into the Ruby application's logging.
//!
//! It uses bump allocation for efficient string formatting and concurrent
//! processing for high-throughput logging scenarios.

#![allow(clippy::print_stderr)]

use crate::bridge::Bridge;
use crate::id;
use crate::util::ThreadSafeValue;
use bumpalo::Bump;
use bumpalo::collections::string::String as BumpString;
use educe::Educe;
use futures::StreamExt;
use magnus::value::ReprValue;
use magnus::{Class, Module, RClass, Ruby, Value};
use std::cell::RefCell;
use std::error::Error;
use std::fmt;
use std::fmt::{Debug, Display, Write};
use std::sync::Arc;
use tokio::spawn;
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};
use tokio_stream::wrappers::UnboundedReceiverStream;
use tracing::field::{Field, Visit};
use tracing::{Event, Level, Metadata, Subscriber};
use tracing_subscriber::Layer;
use tracing_subscriber::layer::Context;

/// Maximum number of log requests to process concurrently.
///
/// Setting this to `None` allows unlimited concurrency, while `Some(n)` limits
/// to n concurrent log requests.
const CONCURRENT_LOG_REQUESTS: Option<usize> = Some(16);

/// A tracing layer that forwards log events to Ruby's logging system.
///
/// This struct captures tracing events from Rust and sends them to a Ruby
/// Logger instance, ensuring consistent logging across language boundaries.
#[derive(Clone, Educe)]
#[educe(Debug)]
pub struct Logger {
    /// Channel sender for log events
    #[educe(Debug(ignore))]
    tx: UnboundedSender<(Level, String)>,
}

impl Logger {
    /// Creates a new logger that forwards events to Ruby's logging system.
    ///
    /// This function:
    /// 1. Creates a channel for passing log events
    /// 2. Sets up a Ruby Logger instance directed to STDOUT
    /// 3. Spawns a tokio task to handle log events asynchronously
    ///
    /// # Arguments
    ///
    /// * `ruby` - Reference to the Ruby VM
    /// * `bridge` - Bridge for communication between Rust and Ruby
    ///
    /// # Returns
    ///
    /// A new `Logger` instance or an error if Ruby logger creation fails.
    ///
    /// # Errors
    ///
    /// Returns a `magnus::Error` if:
    /// - The Ruby `Logger` class cannot be accessed
    /// - STDOUT cannot be accessed
    /// - Logger instantiation fails
    pub fn new(ruby: &Ruby, bridge: Bridge) -> Result<Self, magnus::Error> {
        let (tx, rx) = unbounded_channel::<(Level, String)>();

        let logger_class: RClass = ruby.module_kernel().const_get(id!("Logger"))?;
        let stdout: Value = ruby.module_kernel().const_get(id!("STDOUT"))?;
        let logger = logger_class.new_instance((stdout,))?;
        let logger = Arc::new(ThreadSafeValue::new(logger));

        spawn(async move {
            UnboundedReceiverStream::new(rx)
                .for_each_concurrent(CONCURRENT_LOG_REQUESTS, move |evt| {
                    log_event(bridge.clone(), logger.clone(), evt)
                })
                .await;
        });

        Ok(Self { tx })
    }
}

/// Sends a log event to Ruby.
///
/// This function maps Rust tracing levels to Ruby Logger methods and forwards
/// the message to the Ruby logger.
///
/// # Arguments
///
/// * `bridge` - Bridge for communication between Rust and Ruby
/// * `logger` - Thread-safe reference to a Ruby Logger instance
/// * `(level, msg)` - The log level and formatted message to send
async fn log_event(bridge: Bridge, logger: Arc<ThreadSafeValue>, (level, msg): (Level, String)) {
    if let Err(error) = bridge
        .run(move |ruby| {
            let method = match level {
                Level::ERROR => id!("error"),
                Level::WARN => id!("warn"),
                Level::INFO => id!("info"),
                Level::DEBUG | Level::TRACE => id!("debug"),
            };

            if let Err(error) = logger.get(ruby).funcall::<_, _, Value>(method, (msg,)) {
                eprintln!("failed to log to Ruby: {error:#}");
            }
        })
        .await
    {
        eprintln!("failed to log to Ruby: {error:#}");
    }
}

impl<S: Subscriber> Layer<S> for Logger {
    /// Processes a tracing event and forwards it to Ruby.
    ///
    /// This method captures the event, formats it using a `Visitor`, and sends
    /// it to the Ruby logger.
    ///
    /// # Arguments
    ///
    /// * `event` - The tracing event to process
    /// * `_ctx` - The subscriber context (unused)
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        thread_local! {
            static LOG_BUMP: RefCell<Bump> = RefCell::new(Bump::with_capacity(1024));
        }

        LOG_BUMP.with(|cell| {
            let mut bump = cell.borrow_mut();
            bump.reset();

            let mut visitor = Visitor::new(&bump, event.metadata());
            event.record(&mut visitor);

            if let Err(error) = self
                .tx
                .send((*event.metadata().level(), visitor.to_string()))
            {
                eprintln!("failed to send log message: {error:#}; message: {visitor}");
            }
        });
    }
}

/// A visitor that accumulates recorded field values into a bump-allocated
/// string.
///
/// This struct processes tracing event fields and formats them into a
/// human-readable log message using bump allocation for efficient memory
/// management.
pub struct Visitor<'a> {
    /// Reference to the bump allocator
    bump: &'a Bump,

    /// Accumulated metadata as a string
    metadata: BumpString<'a>,

    /// Optional message field, if present
    maybe_message: Option<BumpString<'a>>,
}

impl<'a> Visitor<'a> {
    /// Creates a new visitor for processing event fields.
    ///
    /// Initializes the visitor with metadata from the event, including module
    /// path and source line number.
    ///
    /// # Arguments
    ///
    /// * `bump` - Reference to a bump allocator
    /// * `md` - Metadata from the tracing event
    pub fn new(bump: &'a Bump, md: &'static Metadata<'static>) -> Self {
        let mut visitor = Self {
            bump,
            metadata: BumpString::with_capacity_in(128, bump),
            maybe_message: None,
        };

        if let Some(module) = md.module_path() {
            let v = bump.alloc_str(module);
            visitor.push_kv("module", v);
        }

        if let Some(line) = md.line() {
            let v = bumpalo::format!(in bump, "{}", line);
            visitor.push_kv("line", &v);
        }

        visitor
    }

    /// Adds a key-value pair to the metadata string.
    ///
    /// # Arguments
    ///
    /// * `key` - The field name
    /// * `value` - The field value as a string
    fn push_kv(&mut self, key: &str, value: &str) {
        if self.metadata.is_empty() {
            self.metadata.push_str(key);
            self.metadata.push('=');
            self.metadata.push_str(value);
        } else {
            self.metadata.push(' ');
            self.metadata.push_str(key);
            self.metadata.push('=');
            self.metadata.push_str(value);
        }
    }
}

impl Visit for Visitor<'_> {
    /// Records a f64 value.
    ///
    /// # Arguments
    ///
    /// * `f` - Field descriptor
    /// * `v` - Field value
    fn record_f64(&mut self, f: &Field, v: f64) {
        let s = bumpalo::format!(in self.bump, "{}", v);
        self.push_kv(f.name(), &s);
    }

    /// Records an i64 value.
    ///
    /// # Arguments
    ///
    /// * `f` - Field descriptor
    /// * `v` - Field value
    fn record_i64(&mut self, f: &Field, v: i64) {
        let s = bumpalo::format!(in self.bump, "{}", v);
        self.push_kv(f.name(), &s);
    }

    /// Records a u64 value.
    ///
    /// # Arguments
    ///
    /// * `f` - Field descriptor
    /// * `v` - Field value
    fn record_u64(&mut self, f: &Field, v: u64) {
        let s = bumpalo::format!(in self.bump, "{}", v);
        self.push_kv(f.name(), &s);
    }

    /// Records an i128 value.
    ///
    /// # Arguments
    ///
    /// * `f` - Field descriptor
    /// * `v` - Field value
    fn record_i128(&mut self, f: &Field, v: i128) {
        let s = bumpalo::format!(in self.bump, "{}", v);
        self.push_kv(f.name(), &s);
    }

    /// Records a u128 value.
    ///
    /// # Arguments
    ///
    /// * `f` - Field descriptor
    /// * `v` - Field value
    fn record_u128(&mut self, f: &Field, v: u128) {
        let s = bumpalo::format!(in self.bump, "{}", v);
        self.push_kv(f.name(), &s);
    }

    /// Records a boolean value.
    ///
    /// # Arguments
    ///
    /// * `f` - Field descriptor
    /// * `v` - Field value
    fn record_bool(&mut self, f: &Field, v: bool) {
        let s = bumpalo::format!(in self.bump, "{}", v);
        self.push_kv(f.name(), &s);
    }

    /// Records a string value.
    ///
    /// # Arguments
    ///
    /// * `f` - Field descriptor
    /// * `v` - Field value
    fn record_str(&mut self, f: &Field, v: &str) {
        self.push_kv(f.name(), v);
    }

    /// Records an error value.
    ///
    /// # Arguments
    ///
    /// * `f` - Field descriptor
    /// * `err` - Error value
    fn record_error(&mut self, f: &Field, err: &(dyn Error + 'static)) {
        let s = bumpalo::format!(in self.bump, "{:#}", err);
        self.push_kv(f.name(), &s);
    }

    /// Records a debug-formatted value.
    ///
    /// Special handling is applied to the "message" field, which is stored
    /// separately from other metadata.
    ///
    /// # Arguments
    ///
    /// * `f` - Field descriptor
    /// * `dbg` - Value to format with Debug
    fn record_debug(&mut self, f: &Field, dbg: &dyn Debug) {
        let m = bumpalo::format!(in self.bump, "{:?}", dbg);
        if f.name() == "message" {
            self.maybe_message = Some(m);
        } else {
            self.push_kv(f.name(), &m);
        }
    }
}

impl Display for Visitor<'_> {
    /// Formats the log message.
    ///
    /// If a "message" field was recorded, it's placed at the beginning of the
    /// formatted string, followed by the metadata. Otherwise, only metadata is
    /// included.
    ///
    /// # Arguments
    ///
    /// * `f` - Formatter to write to
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.maybe_message {
            None => f.write_str(&self.metadata),
            Some(message) => {
                f.write_str(message)?;
                f.write_char(' ')?;
                f.write_str(&self.metadata)
            }
        }
    }
}
