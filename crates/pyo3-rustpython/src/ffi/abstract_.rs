//! Abstract object protocol FFI functions.

use super::ffi_object::*;

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
