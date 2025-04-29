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

const BUMP_CAP: usize = 256;
const META_CAP: usize = 128;
const CONCURRENT_LOG_REQUESTS: Option<usize> = Some(16);

thread_local! {
    static LOG_BUMP: RefCell<Bump> = RefCell::new(Bump::with_capacity(BUMP_CAP));
}

#[derive(Clone, Educe)]
#[educe(Debug)]
pub struct Logger {
    #[educe(Debug(ignore))]
    tx: UnboundedSender<(Level, String)>,
}

impl Logger {
    pub fn new(ruby: &Ruby, bridge: Bridge) -> Result<Self, magnus::Error> {
        let (tx, rx) = unbounded_channel::<(Level, String)>();
        let logger_class: RClass = ruby.module_kernel().const_get(id!("Logger"))?;
        let stdout: Value = ruby.module_kernel().const_get(id!("STDOUT"))?;
        let logger = logger_class.new_instance((stdout,))?;
        let logger = Arc::new(ThreadSafeValue::new(logger));

        spawn(async move {
            UnboundedReceiverStream::new(rx)
                .for_each_concurrent(CONCURRENT_LOG_REQUESTS, |(level, message)| {
                    let bridge = bridge.clone();
                    let logger = logger.clone();

                    async move {
                        let result = bridge
                            .run_sync(move |ruby| {
                                let method = match level {
                                    Level::ERROR => id!("error"),
                                    Level::WARN => id!("warn"),
                                    Level::INFO => id!("info"),
                                    Level::DEBUG => id!("debug"),
                                    Level::TRACE => id!("debug"),
                                };

                                if let Err(error) =
                                    logger.get(ruby).funcall::<_, _, Value>(method, (message,))
                                {
                                    eprintln!("failed to call logger: {error:#}");
                                }
                            })
                            .await;

                        if let Err(error) = result {
                            eprintln!("failed to send log message to Ruby bridge: {error:#}");
                        }
                    }
                })
                .await;
        });

        Ok(Self { tx })
    }
}

impl<S: Subscriber> Layer<S> for Logger {
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        LOG_BUMP.with(|cell| {
            let mut bump = cell.borrow_mut();
            bump.reset();

            let mut visitor = Visitor::new(&bump, event.metadata());
            event.record(&mut visitor);

            if let Err(error) = self
                .tx
                .send((*event.metadata().level(), visitor.to_string()))
            {
                eprintln!("failed to send log message: {error:#}");
            }
        });
    }
}

/// A visitor that accumulates recorded field values into a bump-allocated
/// string.
pub struct Visitor<'a> {
    bump: &'a Bump,
    metadata: BumpString<'a>,
    maybe_message: Option<BumpString<'a>>,
}

impl<'a> Visitor<'a> {
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

    /// Append "key=value" or ", key=value" in one bump-allocated fragment.
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
    fn record_f64(&mut self, f: &Field, v: f64) {
        let s = bumpalo::format!(in self.bump, "{}", v);
        self.push_kv(f.name(), &s);
    }

    fn record_i64(&mut self, f: &Field, v: i64) {
        let s = bumpalo::format!(in self.bump, "{}", v);
        self.push_kv(f.name(), &s);
    }

    fn record_u64(&mut self, f: &Field, v: u64) {
        let s = bumpalo::format!(in self.bump, "{}", v);
        self.push_kv(f.name(), &s);
    }

    fn record_i128(&mut self, f: &Field, v: i128) {
        let s = bumpalo::format!(in self.bump, "{}", v);
        self.push_kv(f.name(), &s);
    }

    fn record_u128(&mut self, f: &Field, v: u128) {
        let s = bumpalo::format!(in self.bump, "{}", v);
        self.push_kv(f.name(), &s);
    }

    fn record_bool(&mut self, f: &Field, v: bool) {
        let s = bumpalo::format!(in self.bump, "{}", v);
        self.push_kv(f.name(), &s);
    }

    fn record_str(&mut self, f: &Field, v: &str) {
        let s = self.bump.alloc_str(v);
        self.push_kv(f.name(), s);
    }

    fn record_error(&mut self, f: &Field, err: &(dyn Error + 'static)) {
        let s = bumpalo::format!(in self.bump, "{:#}", err);
        self.push_kv(f.name(), &s);
    }

    fn record_debug(&mut self, f: &Field, dbg: &dyn Debug) {
        if f.name() == "message" {
            let m = bumpalo::format!(in self.bump, "{:?}", dbg);
            self.maybe_message = Some(m);
        } else {
            let s = bumpalo::format!(in self.bump, "{:?}", dbg);
            self.push_kv(f.name(), &s);
        }
    }
}

impl Display for Visitor<'_> {
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
