//! Error handling FFI functions.
//!
//! NOTE: `PyErr_GetRaisedException` and `PyErr_Fetch` require access to
//! RustPython's internal exception state, which is `pub(crate)` in
//! rustpython-vm. For now, these are stubbed out. They're only needed
//! by ffi-heavy serialization code; the safe PyO3 API paths don't use them.

use super::ffi_object::*;
use rustpython_vm::AsObject;

/// Fetch the currently raised exception (Python 3.12+ API).
///
/// Returns a new reference, or null if no exception is set. Clears the indicator.
///
/// # Safety
/// Must be called with the GIL held.
#[inline]
pub unsafe fn PyErr_GetRaisedException() -> *mut PyObject {
    crate::err::take_current_exception()
        .map(|exc| {
            let obj: rustpython_vm::PyObjectRef = exc.into();
            pyobject_ref_to_ptr(obj)
        })
        .unwrap_or(std::ptr::null_mut())
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
    let current = crate::err::take_current_exception();
    if let Some(exc) = current {
        let exc_type_obj: rustpython_vm::PyObjectRef = exc.class().to_owned().into();
        let exc_value_obj: rustpython_vm::PyObjectRef = exc.into();
        if !exc_type.is_null() {
            *exc_type = pyobject_ref_to_ptr(exc_type_obj);
        }
        if !exc_value.is_null() {
            *exc_value = pyobject_ref_to_ptr(exc_value_obj);
        }
        if !exc_tb.is_null() {
            *exc_tb = std::ptr::null_mut();
        }
    } else {
        if !exc_type.is_null() {
            *exc_type = std::ptr::null_mut();
        }
        if !exc_value.is_null() {
            *exc_value = std::ptr::null_mut();
        }
        if !exc_tb.is_null() {
            *exc_tb = std::ptr::null_mut();
        }
    }
}

/// Clear the current exception indicator.
///
/// # Safety
/// Must be called with the GIL held.
#[inline]
pub unsafe fn PyErr_Clear() {
    crate::err::set_current_exception(None);
}
