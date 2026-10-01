//! Native handle for a presence-only ordered set of `String` members.
//!
//! A set stores membership only. Member traversal reuses the map key cursor,
//! since both yield bare `String` keys.

use super::{NativeMapKeyScan, key_query, outcome_symbol, state_error, wrapped_class};
use crate::bridge::Bridge;
use crate::tracing_util::extract_opentelemetry_context;
use magnus::{Error, Module, RHash, RModule, Ruby, StaticSymbol, method};
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

    fn contains(ruby: &Ruby, this: &Self, member: String) -> Result<bool, Error> {
        run_op!(ruby, this, &this.state, contains(member))
    }

    fn contains_many(ruby: &Ruby, this: &Self, members: Vec<String>) -> Result<Vec<bool>, Error> {
        run_op!(ruby, this, &this.state, contains_many(members))
    }

    fn is_empty(ruby: &Ruby, this: &Self) -> Result<bool, Error> {
        run_op!(ruby, this, &this.state, is_empty())
    }

    fn insert(ruby: &Ruby, this: &Self, member: String) -> Result<(), Error> {
        run_op!(ruby, this, &this.state, insert(member))
    }

    fn remove(ruby: &Ruby, this: &Self, member: String) -> Result<(), Error> {
        run_op!(ruby, this, &this.state, remove(member))
    }

    fn clear(ruby: &Ruby, this: &Self) -> Result<(), Error> {
        run_op!(ruby, this, &this.state, clear())
    }

    fn keys(
        ruby: &Ruby,
        this: &Self,
        direction: StaticSymbol,
        options: RHash,
    ) -> Result<NativeMapKeyScan, Error> {
        let query = key_query(ruby, direction, options)?;
        Ok(NativeMapKeyScan::new(
            this.state.keys().with_query(query).stream(),
            this.bridge.clone(),
            Arc::clone(&this.propagator),
        ))
    }

    fn commit(ruby: &Ruby, this: &Self) -> Result<StaticSymbol, Error> {
        let outcome = run_op!(ruby, this, &this.state, commit())?;
        Ok(outcome_symbol(ruby, outcome))
    }

    fn rollback(ruby: &Ruby, this: &Self) -> Result<StaticSymbol, Error> {
        let outcome = run_infallible!(ruby, this, &this.state, rollback());
        Ok(outcome_symbol(ruby, outcome))
    }

    pub(super) fn register(ruby: &Ruby, module: RModule) -> Result<(), Error> {
        let class = wrapped_class(ruby, module, "Prosody::NativeSetState")?;
        class.define_method("contains", method!(NativeSetState::contains, 1))?;
        class.define_method("contains_many", method!(NativeSetState::contains_many, 1))?;
        class.define_method("is_empty", method!(NativeSetState::is_empty, 0))?;
        class.define_method("insert", method!(NativeSetState::insert, 1))?;
        class.define_method("remove", method!(NativeSetState::remove, 1))?;
        class.define_method("clear", method!(NativeSetState::clear, 0))?;
        class.define_method("keys", method!(NativeSetState::keys, 2))?;
        class.define_method("commit", method!(NativeSetState::commit, 0))?;
        class.define_method("rollback", method!(NativeSetState::rollback, 0))?;
        Ok(())
    }
}
