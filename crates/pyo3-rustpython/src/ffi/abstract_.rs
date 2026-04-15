//! Abstract object protocol FFI functions.

use super::ffi_object::*;
use super::pyport::Py_ssize_t;

/// Convert `obj` to int (as if calling `int(obj)`). Returns a new reference.
///
/// # Safety
/// `obj` must be a valid Python object.
#[inline]
pub unsafe fn PyNumber_Long(obj: *mut PyObject) -> *mut PyObject {
    if obj.is_null() {
        return std::ptr::null_mut();
    }
    let obj_ref = ptr_to_pyobject_ref_borrowed(obj);
    let vm = vm();
    let int_type = vm.ctx.types.int_type.to_owned();
    match vm.invoke(&int_type, (obj_ref.clone(),)) {
        Ok(result) => pyobject_ref_to_ptr(result),
        Err(_) => std::ptr::null_mut(),
    }
}

#[inline]
pub unsafe fn PySequence_Length(obj: *mut PyObject) -> Py_ssize_t {
    if obj.is_null() {
        return -1;
    }
    let vm = vm();
    let obj_ref = ptr_to_pyobject_ref_borrowed(obj);
    match obj_ref.length(vm) {
        Ok(len) => len as Py_ssize_t,
        Err(_) => -1,
    }
}

#[inline]
pub unsafe fn PyMapping_Length(obj: *mut PyObject) -> Py_ssize_t {
    if obj.is_null() {
        return -1;
    }
    let vm = vm();
    let obj_ref = ptr_to_pyobject_ref_borrowed(obj);
    match obj_ref.length(vm) {
        Ok(len) => len as Py_ssize_t,
        Err(_) => -1,
    }
}
