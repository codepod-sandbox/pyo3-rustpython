//! Long (integer) object FFI functions.

use super::ffi_object::*;
use super::pyport::Py_ssize_t;

/// Convert a Python integer to `i64`.
///
/// Returns -1 on failure. Note: -1 might also be valid; check `PyErr_Occurred`.
///
/// # Safety
/// `obj` must be a valid Python integer object.
#[inline]
pub unsafe fn PyLong_AsLongLong(obj: *mut PyObject) -> i64 {
    if obj.is_null() {
        return -1;
    }
    let obj_ref = ptr_to_pyobject_ref_borrowed(obj);
    let vm = vm();
    match obj_ref.try_into_value::<i64>(vm) {
        Ok(v) => v,
        Err(_) => -1,
    }
}

/// Convert a Python integer to `isize`.
///
/// # Safety
/// `obj` must be a valid Python integer object.
#[inline]
pub unsafe fn PyLong_AsSsize_t(obj: *mut PyObject) -> Py_ssize_t {
    PyLong_AsLongLong(obj) as Py_ssize_t
}
