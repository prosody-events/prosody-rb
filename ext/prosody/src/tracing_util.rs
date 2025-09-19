//! OpenTelemetry tracing utilities for Ruby-Rust integration.
//!
//! This module provides shared utilities for extracting and propagating
//! OpenTelemetry context across the Ruby-Rust boundary for distributed tracing.
use crate::id;
use magnus::value::ReprValue;
use magnus::{Error, Module, RHash, RModule, Ruby, Value};
use opentelemetry::Context;
use opentelemetry::propagation::{TextMapCompositePropagator, TextMapPropagator};
use std::collections::HashMap;

/// Extracts OpenTelemetry context from Ruby's current tracing environment.
///
/// This function extracts the current OpenTelemetry context from Ruby and
/// converts it to a Rust context that can be used for distributed tracing.
/// The context is extracted using Ruby's OpenTelemetry propagation mechanism
/// and then parsed using the provided propagator.
///
/// # Arguments
///
/// * `ruby` - Reference to the Ruby VM
/// * `propagator` - The OpenTelemetry propagator to use for context extraction
///
/// # Returns
///
/// The extracted OpenTelemetry context that can be used as a parent for new
/// spans.
///
/// # Errors
///
/// Returns an error if:
/// - OpenTelemetry module is not available in Ruby
/// - Context extraction or propagation fails
/// - Ruby-to-Rust type conversion fails
///
/// # Example
///
/// ```rust
/// let context = extract_opentelemetry_context(ruby, &propagator)?;
/// let span = info_span!("operation");
/// span.set_parent(context);
/// ```
pub fn extract_opentelemetry_context(
    ruby: &Ruby,
    propagator: &TextMapCompositePropagator,
) -> Result<Context, Error> {
    let carrier = RHash::new();
    let otel_class: RModule = ruby.class_module().const_get(id!(ruby, "OpenTelemetry"))?;
    let propagator_obj: Value = otel_class.funcall(id!(ruby, "propagation"), ())?;
    let _: Value = propagator_obj.funcall(id!(ruby, "inject"), (carrier,))?;

    let carrier: HashMap<String, String> = carrier.to_hash_map()?;
    let context = propagator.extract(&carrier);

    Ok(context)
}
