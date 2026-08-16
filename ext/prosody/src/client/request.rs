use super::{
    Class, Client, Deserialize, Duration, Error, Module, OpenTelemetrySpanExt, RClass, ROOT_MOD,
    ReprValue, Ruby, SubsystemName, Value, debug, deserialize, ensure_runtime_context,
    extract_opentelemetry_context, id, info_span, kwargs, response_error, serialize,
};

#[derive(Deserialize)]
struct NativeRequest {
    topic: String,
    key: String,
    payload: serde_json::Value,
    subsystems: Vec<String>,
    timeout: f64,
}

#[derive(Deserialize)]
struct NativeExciseRequest {
    topic: String,
    key: String,
    subsystems: Vec<String>,
    timeout: f64,
}

pub(super) fn request(ruby: &Ruby, this: &Client, request: Value) -> Result<Value, Error> {
    Client::check_fork(ruby, this)?;
    let _guard = ensure_runtime_context(ruby);
    let request: NativeRequest = deserialize(ruby, request)?;
    let subsystems = request
        .subsystems
        .into_iter()
        .map(SubsystemName::try_new)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| Error::new(ruby.exception_arg_error(), error.to_string()))?;
    let timeout = Duration::try_from_secs_f64(request.timeout).map_err(|_| {
        Error::new(
            ruby.exception_arg_error(),
            "timeout must be a finite, non-negative duration",
        )
    })?;
    let topic = prosody::Topic::from(request.topic.as_str());
    let context = extract_opentelemetry_context(ruby, &this.propagator)?;
    let span = info_span!("ruby-request", topic = %request.topic, key = %request.key);
    if let Err(error) = span.set_parent(context) {
        debug!("failed to set parent span: {error:#}");
    }
    let inner = this.inner.clone();
    let results = this
        .bridge
        .wait_for(
            ruby,
            async move {
                inner
                    .request(
                        Vec::new(),
                        topic,
                        request.key,
                        request.payload,
                        subsystems,
                        timeout,
                    )
                    .await
            },
            span,
        )?
        .map_err(|error| Error::new(ruby.exception_runtime_error(), error.to_string()))?;

    let module = ruby.get_inner(&ROOT_MOD);
    let outcomes = ruby.hash_new();
    let success: RClass = module.const_get(id!(ruby, "Success"))?;
    let failure: RClass = module.const_get(id!(ruby, "Failure"))?;
    for (subsystem, result) in results {
        let outcome = match result {
            Ok(value) => {
                let value: Value = serialize(ruby, &value)?;
                success.new_instance((kwargs!(ruby, "value" => value),))?
            }
            Err(error) => failure.new_instance((kwargs!(
                ruby,
                "error" => response_error(ruby, module, error)?
            ),))?,
        };
        outcomes.aset(subsystem.as_str(), outcome)?;
    }
    Ok(outcomes.as_value())
}

pub(super) fn request_excise(ruby: &Ruby, this: &Client, request: Value) -> Result<Value, Error> {
    Client::check_fork(ruby, this)?;
    let _guard = ensure_runtime_context(ruby);
    let request: NativeExciseRequest = deserialize(ruby, request)?;
    let subsystems = request
        .subsystems
        .into_iter()
        .map(SubsystemName::try_new)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| Error::new(ruby.exception_arg_error(), error.to_string()))?;
    let timeout = Duration::try_from_secs_f64(request.timeout).map_err(|_| {
        Error::new(
            ruby.exception_arg_error(),
            "timeout must be a finite, non-negative duration",
        )
    })?;
    let topic = prosody::Topic::from(request.topic.as_str());
    let context = extract_opentelemetry_context(ruby, &this.propagator)?;
    let span = info_span!("ruby-request-excise", topic = %request.topic, key = %request.key);
    if let Err(error) = span.set_parent(context) {
        debug!("failed to set parent span: {error:#}");
    }
    let inner = this.inner.clone();
    let results = this
        .bridge
        .wait_for(
            ruby,
            async move {
                inner
                    .request_excise(Vec::new(), topic, request.key, subsystems, timeout)
                    .await
            },
            span,
        )?
        .map_err(|error| Error::new(ruby.exception_runtime_error(), error.to_string()))?;

    let module = ruby.get_inner(&ROOT_MOD);
    let outcomes = ruby.hash_new();
    let success: RClass = module.const_get(id!(ruby, "Success"))?;
    let failure: RClass = module.const_get(id!(ruby, "Failure"))?;
    for (subsystem, result) in results {
        let outcome = match result {
            Ok(value) => {
                let value: Value = serialize(ruby, &value)?;
                success.new_instance((kwargs!(ruby, "value" => value),))?
            }
            Err(error) => failure.new_instance((kwargs!(
                ruby, "error" => response_error(ruby, module, error)?
            ),))?,
        };
        outcomes.aset(subsystem.as_str(), outcome)?;
    }
    Ok(outcomes.as_value())
}
