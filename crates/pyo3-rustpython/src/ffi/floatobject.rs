//! Float object FFI functions.

use super::ffi_object::*;

/// Get the `double` value (fast path — direct struct access in CPython).
///
/// # Safety
/// `obj` must be a valid Python float object.
#[inline]
pub unsafe fn PyFloat_AS_DOUBLE(obj: *mut PyObject) -> f64 {
    if obj.is_null() {
        return 0.0;
    }
    let obj_ref = ptr_to_pyobject_ref_borrowed(obj);
    match obj_ref.downcast_ref::<rustpython_vm::builtins::PyFloat>() {
        Some(f) => f.to_f64(),
        None => 0.0,
    }
}

/// Get the `double` value (safe version).
///
/// Returns -1.0 on failure.
///
/// # Safety
/// `obj` must be a valid Python object (float or convertible).
#[inline]
pub unsafe fn PyFloat_AsDouble(obj: *mut PyObject) -> f64 {
    if obj.is_null() {
        return -1.0;
    }
    let obj_ref = ptr_to_pyobject_ref_borrowed(obj);
    let vm = vm();
    match obj_ref.try_into_value::<f64>(vm) {
        Ok(v) => v,
        Err(_) => -1.0,
    }
}

/// Create a Python float from a C `double`. Returns a new reference.
#[inline]
pub fn PyFloat_FromDouble(v: f64) -> *mut PyObject {
    let vm = vm();
    let obj: rustpython_vm::PyObjectRef = vm.ctx.new_float(v).into();
    unsafe { pyobject_ref_to_ptr(obj) }
}
