use super::{
    Arc, Class, Client, Duration, ErasedReadCache, Error, FutureExt, HANDLER_METHODS, Module,
    Object, RClass, RModule, ROOT_MOD, ReprValue, ResponseError, Ruby, RubyHandler,
    SharedHighLevelClient, Shutdown, Value, function, id, kwargs, method, request,
};

pub(super) fn validate_handler(ruby: &Ruby, handler: Value) -> Result<(), Error> {
    let event_handler: RClass = ruby
        .get_inner(&ROOT_MOD)
        .const_get(id!(ruby, "EventHandler"))?;
    for method_name in HANDLER_METHODS {
        if !handler.respond_to(method_name, false)? {
            return Err(Error::new(
                ruby.exception_arg_error(),
                format!("handler must implement #{method_name}"),
            ));
        }
        let method: Value = handler.funcall(id!(ruby, "method"), (method_name,))?;
        let owner: Value = method.funcall(id!(ruby, "owner"), ())?;
        if owner.equal(event_handler)? {
            return Err(Error::new(
                ruby.exception_arg_error(),
                format!("handler must implement #{method_name}"),
            ));
        }
    }
    Ok(())
}

pub(super) fn shutdown(client: &SharedHighLevelClient<RubyHandler>) -> Shutdown {
    let client = client.clone();
    async move {
        client
            .shutdown()
            .await
            .map_err(|error| Arc::from(error.to_string()))
    }
    .boxed()
    .shared()
}

pub(super) fn read_cache(
    ruby: &Ruby,
    seconds: Option<f64>,
    disabled: bool,
) -> Result<ErasedReadCache, Error> {
    match (seconds, disabled) {
        (None, false) => Ok(ErasedReadCache::Inherit),
        (None, true) => Ok(ErasedReadCache::Disabled),
        (Some(seconds), false) => Duration::try_from_secs_f64(seconds)
            .map(ErasedReadCache::Ttl)
            .map_err(|_| {
                Error::new(
                    ruby.exception_arg_error(),
                    "read_cache must be finite and non-negative",
                )
            }),
        (Some(_), true) => Err(Error::new(
            ruby.exception_arg_error(),
            "read_cache cannot specify a TTL and be disabled",
        )),
    }
}

/// Initializes the client module in Ruby.
///
/// Defines the `Prosody::Client` class and its methods, making the client
/// functionality available to Ruby code.
///
/// # Arguments
///
/// * `ruby` - The Ruby VM context
///
/// # Errors
///
/// Returns an error if Ruby class or method definition fails.
pub fn init(ruby: &Ruby) -> Result<(), Error> {
    let module = ruby.get_inner(&ROOT_MOD);
    let class = module.define_class(id!(ruby, "Client"), ruby.class_object())?;

    class.define_singleton_method("new", function!(Client::new, 1))?;
    class.define_method(
        id!(ruby, "consumer_state"),
        method!(Client::consumer_state, 0),
    )?;
    class.define_method(id!(ruby, "send_message"), method!(Client::send, 3))?;
    class.define_method(id!(ruby, "excise"), method!(Client::excise, 2))?;
    class.define_method(id!(ruby, "native_request"), method!(request::request, 1))?;
    class.define_method(
        id!(ruby, "native_request_excise"),
        method!(request::request_excise, 1),
    )?;
    class.define_method(id!(ruby, "subscribe"), method!(Client::subscribe, 1))?;
    class.define_method(
        id!(ruby, "assigned_partitions"),
        method!(Client::assigned_partitions, 0),
    )?;
    class.define_method(id!(ruby, "is_stalled?"), method!(Client::is_stalled, 0))?;
    class.define_method(id!(ruby, "unsubscribe"), method!(Client::unsubscribe, 0))?;
    class.define_method(id!(ruby, "shutdown"), method!(Client::shutdown, 0))?;
    class.define_method(
        id!(ruby, "source_system"),
        method!(Client::source_system, 0),
    )?;
    class.define_method(
        id!(ruby, "published_value"),
        method!(Client::published_value, 4),
    )?;
    class.define_method(
        id!(ruby, "published_map"),
        method!(Client::published_map, 4),
    )?;
    class.define_method(
        id!(ruby, "published_deque"),
        method!(Client::published_deque, 4),
    )?;

    Ok(())
}

pub(super) fn response_error(
    ruby: &Ruby,
    module: RModule,
    error: ResponseError,
) -> Result<Value, Error> {
    let (name, message) = match error {
        ResponseError::Handler { message } => ("HandlerError", Some(message)),
        ResponseError::Timeout => ("Timeout", None),
        ResponseError::FormatMismatch => ("FormatMismatch", None),
        ResponseError::Malformed => ("MalformedResponse", None),
    };
    let class: RClass = module.const_get(name)?;
    match message {
        Some(message) => class.new_instance((kwargs!(ruby, "message" => message),)),
        None => class.new_instance(()),
    }
}
