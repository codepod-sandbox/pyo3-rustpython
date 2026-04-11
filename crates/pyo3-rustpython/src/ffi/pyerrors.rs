//! Error handling FFI functions.
//!
//! NOTE: `PyErr_GetRaisedException` and `PyErr_Fetch` require access to
//! RustPython's internal exception state, which is `pub(crate)` in
//! rustpython-vm. For now, these are stubbed out. They're only needed
//! by ffi-heavy serialization code; the safe PyO3 API paths don't use them.

use super::ffi_object::*;

/// Fetch the currently raised exception (Python 3.12+ API).
///
/// Returns a new reference, or null if no exception is set. Clears the indicator.
///
/// # Safety
/// Must be called with the GIL held.
#[inline]
pub unsafe fn PyErr_GetRaisedException() -> *mut PyObject {
    // TODO: implement via rustpython_vm internal access or sys.exc_info()
    // The exception state is pub(crate) in rustpython_vm.
    // For now, return null (no exception).
    std::ptr::null_mut()
}

/// Fetch the current exception as (type, value, traceback).
///
/// Clears the indicator. Any pointer may be null.
///
/// # Safety
/// Must be called with the GIL held. Caller must `Py_DECREF` each non-null ptr.
#[inline]
pub unsafe fn PyErr_Fetch(
    exc_type: *mut *mut PyObject,
    exc_value: *mut *mut PyObject,
    exc_tb: *mut *mut PyObject,
) {
    if !exc_type.is_null() {
        *exc_type = std::ptr::null_mut();
    }
    if !exc_value.is_null() {
        *exc_value = std::ptr::null_mut();
    }
    if !exc_tb.is_null() {
        *exc_tb = std::ptr::null_mut();
    }
    // TODO: implement via rustpython_vm internal access
}

/// Clear the current exception indicator.
///
/// # Safety
/// Must be called with the GIL held.
#[inline]
pub unsafe fn PyErr_Clear() {
    // TODO: implement via rustpython_vm internal access
}
