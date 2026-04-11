//! Tuple object FFI functions.

use rustpython_vm::builtins::PyTuple;

use super::ffi_object::*;
use super::pyport::Py_ssize_t;

/// Get item at position `i`. Returns a NEW reference.
///
/// # Safety
/// `tuple` must be a valid tuple. `i` must be a valid index.
#[inline]
pub unsafe fn PyTuple_GET_ITEM(tuple: *mut PyObject, i: Py_ssize_t) -> *mut PyObject {
    if tuple.is_null() {
        return std::ptr::null_mut();
    }
    let tuple_ref = ptr_to_pyobject_ref_borrowed(tuple);
    let tuple_inner = match tuple_ref.downcast_ref::<PyTuple>() {
        Some(t) => t,
        None => return std::ptr::null_mut(),
    };
    let elements = tuple_inner.as_slice();
    if i < 0 || (i as usize) >= elements.len() {
        return std::ptr::null_mut();
    }
    pyobject_ref_to_ptr(elements[i as usize].clone())
}

/// Get the length of the tuple.
///
/// # Safety
/// `tuple` must be a valid tuple object.
#[inline]
pub unsafe fn PyTuple_GET_SIZE(tuple: *mut PyObject) -> Py_ssize_t {
    if tuple.is_null() {
        return 0;
    }
    let tuple_ref = ptr_to_pyobject_ref_borrowed(tuple);
    match tuple_ref.downcast_ref::<PyTuple>() {
        Some(t) => t.__len__() as Py_ssize_t,
        None => 0,
    }
}

/// Get the length of the tuple (safe version).
///
/// # Safety
/// `tuple` must be a valid tuple object.
#[inline]
pub unsafe fn PyTuple_Size(tuple: *mut PyObject) -> Py_ssize_t {
    PyTuple_GET_SIZE(tuple)
}
