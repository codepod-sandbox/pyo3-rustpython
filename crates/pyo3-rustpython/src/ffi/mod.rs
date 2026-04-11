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
    _list: *mut PyObject,
    _low: Py_ssize_t,
    _high: Py_ssize_t,
) -> *mut PyObject {
    let vm = vm();
    let slice = vm.ctx.new_list(vec![]);
    pyobject_ref_to_ptr(slice.into())
}
