//! RustPython-backed implementation of `pyo3::ffi`.
//!
//! This module provides CPython C API-compatible functions that delegate
//! to RustPython internals. Pointer semantics match CPython: `*mut PyObject`
//! is an owned reference that must be released with `Py_DECREF`.

#![allow(non_camel_case_types, non_snake_case)]

pub mod abstract_;
pub mod dictobject;
pub mod ffi_object;
pub mod floatobject;
pub mod listobject;
pub mod longobject;
pub mod modsupport;
pub mod objimpl;
pub mod pyerrors;
pub mod pyport;
pub mod tupleobject;
pub mod unicodeobject;

pub use abstract_::*;
pub use dictobject::*;
pub use ffi_object::*;
pub use floatobject::*;
pub use listobject::*;
pub use longobject::*;
pub use modsupport::*;
pub use objimpl::*;
pub use pyerrors::*;
pub use pyport::*;
pub use tupleobject::*;
pub use unicodeobject::*;

#[inline]
pub unsafe fn Py_True() -> *mut PyObject {
    let vm = vm();
    let true_obj: rustpython_vm::PyObjectRef = vm.ctx.true_value.clone().into();
    pyobject_ref_to_ptr(true_obj)
}

#[inline]
pub unsafe fn Py_False() -> *mut PyObject {
    let vm = vm();
    let false_obj: rustpython_vm::PyObjectRef = vm.ctx.false_value.clone().into();
    pyobject_ref_to_ptr(false_obj)
}

pub unsafe fn PyList_GetSlice(
    list: *mut PyObject,
    low: Py_ssize_t,
    high: Py_ssize_t,
) -> *mut PyObject {
    if list.is_null() {
        return std::ptr::null_mut();
    }
    let vm = vm();
    let list_ref = ptr_to_pyobject_ref_borrowed(list);
    let Some(list_inner) = list_ref.downcast_ref::<rustpython_vm::builtins::PyList>() else {
        return std::ptr::null_mut();
    };
    let elements = list_inner.borrow_vec();
    let len = elements.len() as Py_ssize_t;
    let start = low.clamp(0, len) as usize;
    let end = high.clamp(low.max(0), len) as usize;
    let slice = vm.ctx.new_list(elements[start..end].to_vec());
    pyobject_ref_to_ptr(slice.into())
}
