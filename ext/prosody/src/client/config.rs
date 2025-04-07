use crate::id;
use crate::ROOT_MOD;
use magnus::block::Proc;
use magnus::method::Block;
use magnus::value::ReprValue;
use magnus::{
    exception, method, Class, Error, IntoValue, Module, RHash, RString, Ruby, Symbol, TryConvert,
    Value,
};
use parking_lot::Mutex;
use std::sync::Arc;

/// The enum representing the three states of the probe port.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Default)]
pub enum ProbePort {
    #[default]
    Unset,
    Disabled,
    Port(u16),
}

/// Ruby wrapper for the configuration.
///
/// Holds an Arc<Mutex<ConfigurationInner>> as its internal state.
#[magnus::wrap(
    class = "Prosody::Configuration",
    free_immediately,
    frozen_shareable,
    size
)]
#[derive(Clone, Debug, Default)]
pub struct Configuration {
    inner: Arc<Mutex<ConfigurationInner>>,
}

/// The actual configuration data.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct ConfigurationInner {
    pub bootstrap_servers: Option<Vec<String>>,
    pub mock: Option<bool>,
    pub send_timeout_ms: Option<u32>,
    pub group_id: Option<String>,
    pub idempotence_cache_size: Option<u32>,
    pub subscribed_topics: Option<Vec<String>>,
    pub allowed_events: Option<Vec<String>>,
    pub source_system: Option<String>,
    pub max_concurrency: Option<u32>,
    pub max_uncommitted: Option<u16>,
    pub max_enqueued_per_key: Option<u16>,
    pub stall_threshold_ms: Option<u32>,
    pub shutdown_timeout_ms: Option<u32>,
    pub poll_interval_ms: Option<u32>,
    pub commit_interval_ms: Option<u32>,
    pub mode: Option<Mode>,
    pub retry_base_ms: Option<u32>,
    pub max_retries: Option<u32>,
    pub max_retry_delay_ms: Option<u32>,
    pub failure_topic: Option<String>,
    pub probe_port: ProbePort,
}

/// Generic accessor macro for defining getter and setter methods.
/// Accepts a field name, its type, and a conversion closure.
macro_rules! define_accessor {
    ($(#[$attr:meta])* $field:ident: $ty:ty, $conv:expr) => {
         paste::paste! {
             $(#[$attr])*
             pub fn $field(&self) -> Option<$ty> {
                 let inner = self.inner.lock();
                 inner.$field.clone()
             }
             $(#[$attr])*
             pub fn [<set_ $field>](&self, value: Value) -> Result<Value, Error> {
                 let mut inner = self.inner.lock();
                 if value.is_nil() {
                     println!("Setting field {} to None", stringify!($field));
                     inner.$field = None;
                 } else {
                     let converted = $conv(value)?;
                     println!("Setting field {} to {:?}", stringify!($field), &converted);
                     inner.$field = Some(converted);
                 }
                 Ok(value)
             }
         }
    }
}

/// Define an accessor using the default TryConvert conversion.
macro_rules! define_field {
    ($field:ident: $ty:ty) => {
        define_accessor!(
            $field: $ty,
            (|v: Value| -> Result<$ty, Error> { < $ty as TryConvert >::try_convert(v) })
        );
    }
}

/// Define an accessor for Vec<String> fields using `convert_to_string_vec`.
macro_rules! define_vec_field {
    ($field:ident) => {
        define_accessor!(
            $field: Vec<String>,
            (|v: Value| -> Result<Vec<String>, Error> { Self::convert_to_string_vec(v) })
        );
    }
}

/// Macro to set each field from the keyword hash.
///
/// This macro accepts the keyword hash as `$kwargs` (passed in from the caller)
/// and creates the keys using our id! macro and RString::new. Debug logging is added
/// to trace the lookups.
#[macro_export]
macro_rules! set_from_kwargs {
    ($ruby:expr, $config:expr, $kwargs:expr, $($field:ident),+ $(,)?) => {
        paste::paste! {
            $(
                println!("Looking up key: {}", stringify!($field));
                let key_sym = id!(stringify!($field));
                let key_str = RString::new(stringify!($field)).into_value();
                let value_opt = $kwargs.get(key_sym)
                    .or_else(|| $kwargs.get(key_str));
                match value_opt {
                    Some(value) => {
                        println!("Found value for field {}: {:?}", stringify!($field), value);
                        // Properly handle the result value
                        let _ = $config.[<set_ $field>](value)?;
                    },
                    None => {
                        println!("No value found for field {}", stringify!($field));
                    },
                }
            )+
        }
    }
}

impl Configuration {
    // We need this constructor for initializing our Ruby objects
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(ConfigurationInner::default())),
        }
    }

    /// Converts a Ruby value to a Vec<String>.
    /// Accepts either a single string or an array of strings.
    pub fn convert_to_string_vec(value: Value) -> Result<Vec<String>, Error> {
        if let Ok(result) = String::try_convert(value) {
            Ok(vec![result])
        } else {
            Ok(Vec::try_convert(value)?)
        }
    }

    /// Converts a Ruby value to a ProbePort.
    pub fn convert_probe_port(value: Value) -> Result<ProbePort, Error> {
        if value.is_nil() {
            return Ok(ProbePort::Unset);
        }
        if bool::try_convert(value).ok() == Some(false) {
            return Ok(ProbePort::Disabled);
        }
        if value.is_kind_of(magnus::class::symbol()) {
            let sym: Symbol = Symbol::try_convert(value)?;
            let s = sym.to_string().to_lowercase();
            if s == "disabled" {
                return Ok(ProbePort::Disabled);
            }
            let inspected: String = value.funcall("inspect", ())?;
            return Err(Error::new(
                exception::type_error(),
                format!("expected :disabled or a port number, got: {inspected}"),
            ));
        }
        if let Ok(port) = u16::try_convert(value) {
            return Ok(ProbePort::Port(port));
        }
        let inspected: String = value.funcall("inspect", ())?;
        Err(Error::new(
            exception::type_error(),
            format!("expected :disabled, a port number, false, or nil, got: {inspected}"),
        ))
    }

    // Define vector fields.
    define_vec_field!(bootstrap_servers);
    define_vec_field!(subscribed_topics);
    define_vec_field!(allowed_events);

    // Define regular fields.
    define_field!(mock: bool);
    define_field!(send_timeout_ms: u32);
    define_field!(group_id: String);
    define_field!(idempotence_cache_size: u32);
    define_field!(source_system: String);
    define_field!(max_concurrency: u32);
    define_field!(max_uncommitted: u16);
    define_field!(max_enqueued_per_key: u16);
    define_field!(stall_threshold_ms: u32);
    define_field!(shutdown_timeout_ms: u32);
    define_field!(poll_interval_ms: u32);
    define_field!(commit_interval_ms: u32);
    define_field!(retry_base_ms: u32);
    define_field!(max_retries: u32);
    define_field!(max_retry_delay_ms: u32);
    define_field!(failure_topic: String);
    // Remove generic field for mode; handle it separately.

    // Specialized accessor for mode.
    pub fn mode(ruby: &Ruby, this: &Self) -> Value {
        let inner = this.inner.lock();
        if let Some(mode) = inner.mode {
            Symbol::new(mode.to_s()).into_value()
        } else {
            ruby.qnil().into_value()
        }
    }
    pub fn set_mode(&self, value: Value) -> Result<Value, Error> {
        let mut inner = self.inner.lock();
        if value.is_nil() {
            inner.mode = None;
        } else {
            inner.mode = Some(Mode::try_convert(value)?);
        }
        Ok(value)
    }

    // Specialized accessor for probe_port.
    pub fn probe_port(ruby: &Ruby, this: &Self) -> Value {
        let inner = this.inner.lock();
        match inner.probe_port {
            ProbePort::Unset => ruby.qnil().into_value(),
            ProbePort::Disabled => false.into_value(),
            ProbePort::Port(port) => i64::from(port).into_value(),
        }
    }
    pub fn set_probe_port(&self, value: Value) -> Result<Value, Error> {
        let port = Self::convert_probe_port(value)?;
        let mut inner = self.inner.lock();
        inner.probe_port = port;
        Ok(value)
    }

    /// Ruby initializer.
    ///
    /// Accepts a keyword hash (or nil) as the first argument and an optional block.
    /// Idiomatic usage in Ruby:
    ///
    ///   config = Prosody::Configuration.new(bootstrap_servers: [...]) do |config|
    ///     config.mock = true
    ///   end
    pub fn initialize(ruby: &Ruby, args: &[Value], block: Option<Proc>) -> Result<Self, Error> {
        let val = args
            .first()
            .copied()
            .unwrap_or_else(|| ruby.qnil().into_value());

        let kwargs = if val.is_nil() {
            RHash::new()
        } else {
            RHash::try_convert(val)?
        };

        println!("Kwargs content: {:?}", kwargs);

        // Create a new configuration
        let config = Self::new();

        // Process the keyword arguments using this instance
        set_from_kwargs!(
            ruby,
            &config,
            kwargs,
            bootstrap_servers,
            mock,
            send_timeout_ms,
            group_id,
            idempotence_cache_size,
            subscribed_topics,
            allowed_events,
            source_system,
            max_concurrency,
            max_uncommitted,
            max_enqueued_per_key,
            stall_threshold_ms,
            shutdown_timeout_ms,
            poll_interval_ms,
            commit_interval_ms,
            mode,
            retry_base_ms,
            max_retries,
            max_retry_delay_ms,
            failure_topic,
            probe_port
        );

        // Handle the block if provided
        if let Some(b) = block {
            println!("Yielding configuration to block...");
            // Use the same configuration object
            let _ = b.call::<_, Value>((config.clone(),))?;
        } else {
            println!("No block provided.");
        }

        // Create a return value that uses the same inner state
        Ok(config)
    }
}

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
#[magnus::wrap(class = "Prosody::Mode", free_immediately, frozen_shareable, size)]
pub enum Mode {
    #[default]
    Pipeline,
    LowLatency,
    BestEffort,
}

impl Mode {
    pub fn to_s(self) -> String {
        match self {
            Mode::Pipeline => "pipeline".to_owned(),
            Mode::LowLatency => "low_latency".to_owned(),
            Mode::BestEffort => "best_effort".to_owned(),
        }
    }
}

impl TryConvert for Mode {
    fn try_convert(val: Value) -> Result<Self, Error> {
        if !val.is_kind_of(magnus::class::symbol()) {
            let inspected: String = val.funcall("inspect", ())?;
            return Err(Error::new(
                exception::type_error(),
                format!("expected mode symbol, got: {inspected}"),
            ));
        }
        let sym: Symbol = Symbol::try_convert(val)?;
        let s = sym.to_string().to_lowercase();
        match s.as_str() {
            "pipeline" => Ok(Mode::Pipeline),
            "low_latency" | "lowlatency" => Ok(Mode::LowLatency),
            "best_effort" | "besteffort" => Ok(Mode::BestEffort),
            _ => Err(Error::new(
                exception::arg_error(),
                format!("invalid mode: {s}"),
            )),
        }
    }
}

macro_rules! register_field {
    ($class:expr, $field:ident) => {
        paste::paste! {
            $class.define_method(stringify!($field), method!(Configuration::$field, 0))?;
            $class.define_method(
                concat!(stringify!($field), "="),
                method!(|this: &Configuration, value: Value| -> Result<Value, Error> {
                    Configuration::[<set_ $field>](this, value)
                }, 1)
            )?;
        }
    };
}
macro_rules! register_fields {
    ($class:expr, $($field:ident),+ $(,)?) => {
        $( register_field!($class, $field); )+
    }
}

pub fn init(ruby: &Ruby) -> Result<(), Error> {
    let module = ruby.get_inner(&ROOT_MOD);

    let mode_class = module.define_class("Mode", ruby.class_object())?;
    mode_class.define_method("to_s", method!(Mode::to_s, 0))?;
    mode_class.const_set("PIPELINE", Mode::Pipeline)?;
    mode_class.const_set("LOW_LATENCY", Mode::LowLatency)?;
    mode_class.const_set("BEST_EFFORT", Mode::BestEffort)?;

    let config_class = module.define_class("Configuration", ruby.class_object())?;
    config_class.define_alloc_func::<Configuration>();
    config_class.define_method("initialize", method!(Configuration::initialize, -1))?;

    register_fields!(
        config_class,
        bootstrap_servers,
        mock,
        send_timeout_ms,
        group_id,
        idempotence_cache_size,
        subscribed_topics,
        allowed_events,
        source_system,
        max_concurrency,
        max_uncommitted,
        max_enqueued_per_key,
        stall_threshold_ms,
        shutdown_timeout_ms,
        poll_interval_ms,
        commit_interval_ms,
        retry_base_ms,
        max_retries,
        max_retry_delay_ms,
        failure_topic
    );
    config_class.define_method("probe_port", method!(Configuration::probe_port, 0))?;
    config_class.define_method("probe_port=", method!(Configuration::set_probe_port, 1))?;
    config_class.define_method("mode", method!(Configuration::mode, 0))?;
    config_class.define_method("mode=", method!(Configuration::set_mode, 1))?;
    config_class.define_method("mock?", method!(Configuration::mock, 0))?;
    Ok(())
}
