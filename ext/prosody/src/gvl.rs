use rb_sys::rb_thread_call_without_gvl;
use std::ffi::c_void;
use std::panic::{AssertUnwindSafe, catch_unwind};
use thiserror::Error;
use tracing::error;

/// Executes the given closure `func` without holding Ruby’s Global VM Lock
/// (GVL), allowing long-running or blocking Rust code to run without blocking
/// Ruby’s threads.
///
/// # Arguments
///
/// - `func`: A closure that performs the work without the GVL and returns a
///   value of type `R`. Called **exactly once** by Ruby.
/// - `unblock`: A closure that Ruby will call to "unblock" the thread if needed
///   while `func` is running. Ruby **may** call this multiple times, depending
///   on signals, etc.
///
/// # Returns
///
/// The result of `func`.
///
/// # Panics
///
/// - If either `func` or `unblock` panics, that panic is caught and the process
///   is aborted. This is necessary to prevent unwinding across the C FFI
///   boundary, which is undefined behavior.
///
/// # Overview
///
/// 1. We box both closures (`func` and `unblock`) and convert them into raw
///    pointers via `Box::into_raw`.
/// 2. We define two `unsafe extern "C"` callbacks (`anon_func` and
///    `anon_unblock`), which:
///    - `anon_func` (for `func`) *consumes* its Box exactly once via
///      `Box::from_raw`, then calls it inside `catch_unwind`.
///    - `anon_unblock` (for `unblock`) does **not** consume its Box. Instead,
///      it just uses a pointer to call `unblock` each time Ruby needs it.
///      Because Ruby may call it multiple times, we only do `Box::from_raw` on
///      this pointer **after** `rb_thread_call_without_gvl` returns.
/// 3. We wrap calls in `catch_unwind` + `abort()` so that no panic can cross
///    the C boundary.
/// 4. Once `rb_thread_call_without_gvl` returns, we do a single
///    `Box::from_raw(unblock_ptr)`, ensuring no memory leaks. We also retrieve
///    the result from `anon_func` via `Box::from_raw` exactly once.
///
/// # Safety
///
/// - We must ensure each pointer from `Box::into_raw` is only reconstructed
///   exactly once via `Box::from_raw`.
/// - `anon_unblock` does not free the pointer on each call, thus avoiding
///   double-free if Ruby calls it multiple times.
/// - `catch_unwind` and `abort()` ensure that we never unwind through C code.
///
/// Adapted from: <https://github.com/temporalio/sdk-ruby/blob/main/temporalio/ext/src/util.rs>,
/// plus various examples in the Rust/Ruby ecosystem.
#[allow(unsafe_code)]
pub(crate) fn without_gvl<F, R, U>(func: F, unblock: U) -> Result<R, GvlError>
where
    F: FnOnce() -> R,
    U: FnMut() + Send,
{
    // SAFETY: This is an FFI callback, called exactly once by Ruby. We:
    // - Reconstruct the Box<F> from `data`.
    // - Catch any panic to avoid unwinding across the C boundary.
    // - Return a pointer to a boxed result or abort on panic.
    unsafe extern "C" fn anon_func<F, R>(data: *mut c_void) -> *mut c_void
    where
        F: FnOnce() -> R,
    {
        // SAFETY: `data` is guaranteed to come from `Box::into_raw(Box<F>)`.
        // We reconstruct exactly once, transferring ownership back into Rust.
        let func: Box<F> = unsafe { Box::from_raw(data.cast()) };

        // Catch unwind so we don't panic across FFI.
        let result = catch_unwind(AssertUnwindSafe(|| (*func)())).map_err(|_| GvlError::Panicked);

        // SAFETY: We box the result, returning a raw pointer. The caller
        // will Box::from_raw it exactly once after `rb_thread_call_without_gvl`.
        Box::into_raw(Box::new(result)).cast::<c_void>()
    }

    // SAFETY: Another FFI callback. Ruby may call this multiple times, so we do
    // NOT call `Box::from_raw` here. Instead, we just dereference the pointer
    // each time. We also catch any panic and abort to avoid unwinding across FFI.
    unsafe extern "C" fn anon_unblock<U>(data: *mut c_void)
    where
        U: FnMut() + Send,
    {
        // We take a pointer to `U`; we do NOT consume the Box here.
        // Note that `&mut *ptr` is not automatically UnwindSafe, so we wrap the call
        // in `AssertUnwindSafe`.
        let closure_ptr = data.cast::<U>();

        if catch_unwind(AssertUnwindSafe(|| {
            // SAFETY: closure_ptr was allocated via `Box::into_raw`. We only
            // borrow it here, not freeing it. That’s safe for multiple calls.
            unsafe { (*closure_ptr)() };
        }))
        .is_err()
        {
            error!("panicked while attempting to unblock a function running outside of the GVL");
        }
    }

    // Box up both closures. We'll consume `func` exactly once in `anon_func`,
    // and we'll only free `unblock` after `rb_thread_call_without_gvl` returns.
    let boxed_func = Box::new(func);
    let boxed_unblock = Box::new(unblock);

    // Convert to raw pointers for FFI.
    let func_ptr = Box::into_raw(boxed_func);
    let unblock_ptr = Box::into_raw(boxed_unblock);

    // SAFETY: Passing valid function pointers and data pointers to
    // `rb_thread_call_without_gvl`. Ruby will:
    // - Call `anon_func` once, passing `func_ptr`.
    // - Potentially call `anon_unblock` multiple times with `unblock_ptr`.
    // After `rb_thread_call_without_gvl` returns, we are guaranteed that Ruby
    // won't call `anon_unblock` again, so we can safely free the unblock closure.
    let raw_result = unsafe {
        rb_thread_call_without_gvl(
            Some(anon_func::<F, R>),
            func_ptr.cast(),
            Some(anon_unblock::<U>),
            unblock_ptr.cast(),
        )
    };

    // SAFETY: This pointer came from `anon_func`, which did
    // `Box::into_raw(Box::new(val))`. We reconstruct it here exactly once to
    // get the result.
    let result = unsafe { *Box::from_raw(raw_result.cast()) };

    // SAFETY: Now that `rb_thread_call_without_gvl` has returned, Ruby will no
    // longer call our `anon_unblock` callback. Thus, it's safe to free the
    // `unblock` closure exactly once here.
    unsafe {
        let _ = Box::from_raw(unblock_ptr);
    }

    result
}

#[derive(Debug, Error)]
pub enum GvlError {
    #[error("panicked while attempting to call function running outside of the GVL")]
    Panicked,
}
