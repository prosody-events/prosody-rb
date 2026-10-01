//! Registration for concrete native state classes.

use super::{
    NativeJsonDequeScan, NativeJsonDequeState, NativeJsonMapScan, NativeJsonMapState,
    NativeJsonValueState, NativeMapKeyScan, NativeMessageDequeScan, NativeMessageDequeState,
    NativeMessageMapScan, NativeMessageMapState, NativeMessageValueState, NativeSetState,
};
use crate::ROOT_MOD;
use magnus::{Error, Ruby};

/// Registers the concrete native state classes.
///
/// # Errors
///
/// Returns a Magnus error if class or method definition fails.
pub(crate) fn register(ruby: &Ruby) -> Result<(), Error> {
    let module = ruby.get_inner(&ROOT_MOD);

    NativeJsonValueState::register(ruby, module)?;
    NativeMessageValueState::register(ruby, module)?;
    NativeJsonMapState::register(ruby, module)?;
    NativeMessageMapState::register(ruby, module)?;
    NativeSetState::register(ruby, module)?;
    NativeJsonDequeState::register(ruby, module)?;
    NativeMessageDequeState::register(ruby, module)?;
    NativeJsonDequeScan::register(ruby, module)?;
    NativeJsonMapScan::register(ruby, module)?;
    NativeMessageDequeScan::register(ruby, module)?;
    NativeMessageMapScan::register(ruby, module)?;
    NativeMapKeyScan::register(ruby, module)?;

    Ok(())
}
