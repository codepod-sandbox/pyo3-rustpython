//! Dict object FFI functions.

/// Opaque dict type for ffi code that references `PyDictObject`.
/// In RustPython, dicts are just `PyObject` instances.
#[repr(C)]
pub struct PyDictObject {
    _opaque: [usize; 0],
    pub ma_used: crate::ffi::pyport::Py_ssize_t,
}

use std::os::raw::c_int;

use rustpython_vm::builtins::PyDict;
use rustpython_vm::PyPayload;

use super::ffi_object::*;
use super::pyport::Py_ssize_t;

/// Iterate over dict items. `*pos` starts at 0; returned pointers are NEW refs.
///
/// Returns 1 if another item was found, 0 if exhausted.
///
/// # Safety
/// `dict` must be a valid dict. `pos` must be a valid pointer.
#[inline]
pub unsafe fn PyDict_Next(
    dict: *mut PyObject,
    pos: *mut Py_ssize_t,
    key: *mut *mut PyObject,
    value: *mut *mut PyObject,
) -> c_int {
    if dict.is_null() || pos.is_null() {
        return 0;
    }
    let dict_ref = ptr_to_pyobject_ref_borrowed(dict);
    let dict_inner = match dict_ref.downcast_ref::<PyDict>() {
        Some(d) => d,
        None => return 0,
    };

    let items: Vec<(rustpython_vm::PyObjectRef, rustpython_vm::PyObjectRef)> =
        dict_inner.items_vec();

    let current_pos = *pos as usize;
    if current_pos >= items.len() {
        return 0;
    }

    let (k, v) = &items[current_pos];

    if !key.is_null() {
        *key = pyobject_ref_to_ptr(k.clone());
    }
    if !value.is_null() {
        *value = pyobject_ref_to_ptr(v.clone());
    }
    *pos = (current_pos + 1) as Py_ssize_t;
    1
}

/// Get the number of items in the dict.
///
/// # Safety
/// `dict` must be a valid dict object.
#[inline]
pub unsafe fn PyDict_Size(dict: *mut PyObject) -> Py_ssize_t {
    if dict.is_null() {
        return -1;
    }
    let dict_ref = ptr_to_pyobject_ref_borrowed(dict);
    match dict_ref.downcast_ref::<PyDict>() {
        Some(d) => d.__len__() as Py_ssize_t,
        None => -1,
    }
}

/// Create a shallow copy. Returns a new reference.
///
/// # Safety
/// `dict` must be a valid dict object.
#[inline]
pub unsafe fn PyDict_Copy(dict: *mut PyObject) -> *mut PyObject {
    if dict.is_null() {
        return std::ptr::null_mut();
    }
    let dict_ref = ptr_to_pyobject_ref_borrowed(dict);
    let vm = vm();
    let copy = PyDict::default().into_ref(&vm.ctx);
    let dict_inner = match dict_ref.downcast_ref::<PyDict>() {
        Some(d) => d,
        None => return std::ptr::null_mut(),
    };
    for (k, v) in dict_inner.items_vec() {
        copy.set_item(&*k, v, vm).ok();
    }
    let obj: rustpython_vm::PyObjectRef = copy.into();
    pyobject_ref_to_ptr(obj)
}

/// Set an item in the dict. Does NOT steal references.
///
/// Returns 0 on success, -1 on failure.
///
/// # Safety
/// `dict`, `key`, `value` must be valid Python objects.
#[inline]
pub unsafe fn PyDict_SetItem(
    dict: *mut PyObject,
    key: *mut PyObject,
    value: *mut PyObject,
) -> c_int {
    if dict.is_null() || key.is_null() || value.is_null() {
        return -1;
    }
    let dict_ref = ptr_to_pyobject_ref_borrowed(dict);
    let key_ref = ptr_to_pyobject_ref_borrowed(key);
    let value_ref = ptr_to_pyobject_ref_borrowed(value);
    let vm = vm();
    match dict_ref.set_item(&*key_ref, value_ref, vm) {
        Ok(()) => 0,
        Err(_) => -1,
    }
}
