//! Registration for concrete native state classes.

use super::{
    NativeJsonDequeScan, NativeJsonDequeState, NativeJsonMapScan, NativeJsonMapState,
    NativeJsonValueState, NativeMapKeyScan, NativeMessageDequeScan, NativeMessageDequeState,
    NativeMessageMapScan, NativeMessageMapState, NativeMessageValueState,
};
use crate::{ROOT_MOD, id};
use magnus::{Error, Module, Ruby, method};

macro_rules! register_value {
    ($ruby:expr, $module:expr, $name:literal, $type:ty) => {{
        let class = $module.define_class(id!($ruby, $name), $ruby.class_object())?;
        class.define_method(id!($ruby, "get"), method!(<$type>::get, 0))?;
        class.define_method(id!($ruby, "set"), method!(<$type>::set, 1))?;
        class.define_method(id!($ruby, "clear"), method!(<$type>::clear, 0))?;
        class.define_method(id!($ruby, "commit"), method!(<$type>::commit, 0))?;
        class.define_method(id!($ruby, "rollback"), method!(<$type>::rollback, 0))?;
    }};
}

macro_rules! register_map {
    ($ruby:expr, $module:expr, $name:literal, $type:ty) => {{
        let class = $module.define_class(id!($ruby, $name), $ruby.class_object())?;
        class.define_method(id!($ruby, "get"), method!(<$type>::get, 1))?;
        class.define_method(
            id!($ruby, "contains_key"),
            method!(<$type>::contains_key, 1),
        )?;
        class.define_method(id!($ruby, "get_many"), method!(<$type>::get_many, 1))?;
        class.define_method(id!($ruby, "set"), method!(<$type>::set, 2))?;
        class.define_method(id!($ruby, "remove"), method!(<$type>::remove, 1))?;
        class.define_method(id!($ruby, "clear"), method!(<$type>::clear, 0))?;
        class.define_method(id!($ruby, "scan"), method!(<$type>::scan, 1))?;
        class.define_method(id!($ruby, "keys"), method!(<$type>::keys, 1))?;
        class.define_method(id!($ruby, "commit"), method!(<$type>::commit, 0))?;
        class.define_method(id!($ruby, "rollback"), method!(<$type>::rollback, 0))?;
    }};
}

macro_rules! register_deque {
    ($ruby:expr, $module:expr, $name:literal, $type:ty) => {{
        let class = $module.define_class(id!($ruby, $name), $ruby.class_object())?;
        class.define_method(id!($ruby, "len"), method!(<$type>::len, 0))?;
        class.define_method(id!($ruby, "is_empty"), method!(<$type>::is_empty, 0))?;
        class.define_method(id!($ruby, "get"), method!(<$type>::get, 1))?;
        class.define_method(id!($ruby, "peek_front"), method!(<$type>::peek_front, 0))?;
        class.define_method(id!($ruby, "peek_back"), method!(<$type>::peek_back, 0))?;
        class.define_method(id!($ruby, "push_back"), method!(<$type>::push_back, 1))?;
        class.define_method(id!($ruby, "push_front"), method!(<$type>::push_front, 1))?;
        class.define_method(id!($ruby, "pop_front"), method!(<$type>::pop_front, 0))?;
        class.define_method(id!($ruby, "pop_back"), method!(<$type>::pop_back, 0))?;
        class.define_method(id!($ruby, "clear"), method!(<$type>::clear, 0))?;
        class.define_method(id!($ruby, "scan"), method!(<$type>::scan, 1))?;
        class.define_method(id!($ruby, "commit"), method!(<$type>::commit, 0))?;
        class.define_method(id!($ruby, "rollback"), method!(<$type>::rollback, 0))?;
    }};
}

macro_rules! register_scan {
    ($ruby:expr, $module:expr, $name:literal, $type:ty) => {{
        let class = $module.define_class(id!($ruby, $name), $ruby.class_object())?;
        class.define_method(id!($ruby, "next"), method!(<$type>::next, 0))?;
        class.define_method(id!($ruby, "close"), method!(<$type>::close, 0))?;
    }};
}

/// Registers the concrete native state classes.
///
/// # Errors
///
/// Returns a Magnus error if class or method definition fails.
pub(crate) fn register(ruby: &Ruby) -> Result<(), Error> {
    let module = ruby.get_inner(&ROOT_MOD);

    register_value!(ruby, module, "NativeJsonValueState", NativeJsonValueState);
    register_value!(
        ruby,
        module,
        "NativeMessageValueState",
        NativeMessageValueState
    );
    register_map!(ruby, module, "NativeJsonMapState", NativeJsonMapState);
    register_map!(ruby, module, "NativeMessageMapState", NativeMessageMapState);
    register_deque!(ruby, module, "NativeJsonDequeState", NativeJsonDequeState);
    register_deque!(
        ruby,
        module,
        "NativeMessageDequeState",
        NativeMessageDequeState
    );
    register_scan!(ruby, module, "NativeJsonDequeScan", NativeJsonDequeScan);
    register_scan!(ruby, module, "NativeJsonMapScan", NativeJsonMapScan);
    register_scan!(
        ruby,
        module,
        "NativeMessageDequeScan",
        NativeMessageDequeScan
    );
    register_scan!(ruby, module, "NativeMessageMapScan", NativeMessageMapScan);
    register_scan!(ruby, module, "NativeMapKeyScan", NativeMapKeyScan);

    Ok(())
}
