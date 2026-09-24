//! Native handle for a presence-only ordered set of `String` members.
//!
//! A set stores membership only. Member traversal reuses the map key cursor,
//! since both yield bare `String` keys.

use super::{NativeMapKeyScan, key_query, outcome_symbol, scan_arguments, state_error};
use crate::bridge::Bridge;
use crate::tracing_util::extract_opentelemetry_context;
use magnus::value::ReprValue;
use magnus::{Error, Ruby, StaticSymbol, Value};
use opentelemetry::propagation::TextMapCompositePropagator;
use opentelemetry::trace::FutureExt;
use prosody::consumer::event_context::DynSetState;
use std::sync::Arc;
use tracing::Span;

/// Native set handle, wrapped by `Prosody::SetState`.
#[magnus::wrap(class = "Prosody::NativeSetState")]
pub struct NativeSetState {
    state: Arc<dyn DynSetState>,
    bridge: Bridge,
    propagator: Arc<TextMapCompositePropagator>,
}

impl NativeSetState {
    pub(crate) fn new(
        state: Arc<dyn DynSetState>,
        bridge: Bridge,
        propagator: Arc<TextMapCompositePropagator>,
    ) -> Self {
        Self {
            state,
            bridge,
            propagator,
        }
    }

    pub(super) fn contains(ruby: &Ruby, this: &Self, member: String) -> Result<bool, Error> {
        run_op!(ruby, this, &this.state, contains(member))
    }

    pub(super) fn contains_many(
        ruby: &Ruby,
        this: &Self,
        members: Vec<String>,
    ) -> Result<Vec<bool>, Error> {
        run_op!(ruby, this, &this.state, contains_many(members))
    }

    pub(super) fn is_empty(ruby: &Ruby, this: &Self) -> Result<bool, Error> {
        run_op!(ruby, this, &this.state, is_empty())
    }

    pub(super) fn insert(ruby: &Ruby, this: &Self, member: String) -> Result<Value, Error> {
        run_op!(ruby, this, &this.state, insert(member))?;
        Ok(ruby.qnil().as_value())
    }

    pub(super) fn remove(ruby: &Ruby, this: &Self, member: String) -> Result<Value, Error> {
        run_op!(ruby, this, &this.state, remove(member))?;
        Ok(ruby.qnil().as_value())
    }

    pub(super) fn clear(ruby: &Ruby, this: &Self) -> Result<Value, Error> {
        run_op!(ruby, this, &this.state, clear())?;
        Ok(ruby.qnil().as_value())
    }

    pub(super) fn keys(
        ruby: &Ruby,
        this: &Self,
        args: &[Value],
    ) -> Result<NativeMapKeyScan, Error> {
        let (direction, options) = scan_arguments(args)?;
        let query = key_query(ruby, direction, options)?;
        NativeMapKeyScan::new(
            ruby,
            this.state.keys().with_query(query).stream(),
            this.bridge.clone(),
            Arc::clone(&this.propagator),
        )
    }

    pub(super) fn commit(ruby: &Ruby, this: &Self) -> Result<StaticSymbol, Error> {
        let outcome = run_op!(ruby, this, &this.state, commit())?;
        Ok(outcome_symbol(ruby, outcome))
    }

    pub(super) fn rollback(ruby: &Ruby, this: &Self) -> Result<StaticSymbol, Error> {
        let outcome = run_infallible!(ruby, this, &this.state, rollback());
        Ok(outcome_symbol(ruby, outcome))
    }
}
