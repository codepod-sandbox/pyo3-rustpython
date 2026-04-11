//! Unicode/string object FFI functions.

use std::os::raw::c_char;

use rustpython_vm::convert::ToPyObject;

use super::ffi_object::*;
use super::pyport::Py_ssize_t;

/// Get the UTF-8 representation. Returns a pointer into the string buffer.
///
/// # Safety
/// `obj` must be a valid Python string. The returned pointer is valid as long
/// as the string is alive (no reference to keep it alive is taken).
#[inline]
pub unsafe fn PyUnicode_AsUTF8AndSize(obj: *mut PyObject, size: *mut Py_ssize_t) -> *const c_char {
    if obj.is_null() {
        return std::ptr::null();
    }
    let obj_ref = ptr_to_pyobject_ref_borrowed(obj);
    match obj_ref.downcast_ref::<rustpython_vm::builtins::PyStr>() {
        Some(s) => {
            let st = s.as_str();
            if !size.is_null() {
                *size = st.len() as Py_ssize_t;
            }
            st.as_ptr() as *const c_char
        }
        None => std::ptr::null(),
    }
}

/// Intern a string. Returns a new reference.
///
/// # Safety
/// `s` must be a null-terminated C string with valid UTF-8.
#[inline]
pub unsafe fn PyUnicode_InternFromString(s: *const c_char) -> *mut PyObject {
    if s.is_null() {
        return std::ptr::null_mut();
    }
    let cstr = std::ffi::CStr::from_ptr(s);
    let rust_str = match cstr.to_str() {
        Ok(v) => v,
        Err(_) => return std::ptr::null_mut(),
    };
    let vm = vm();
    let interned = vm.ctx.intern_str(rust_str);
    let obj: rustpython_vm::PyObjectRef = interned.to_pyobject(vm);
    pyobject_ref_to_ptr(obj)
}

/// Create a Python string from UTF-8 data and size. Returns a new reference.
///
/// # Safety
/// `u` must point to valid UTF-8 of at least `size` bytes.
#[inline]
pub unsafe fn PyUnicode_FromStringAndSize(u: *const c_char, size: Py_ssize_t) -> *mut PyObject {
    if u.is_null() {
        return std::ptr::null_mut();
    }
    let bytes = std::slice::from_raw_parts(u as *const u8, size as usize);
    let rust_str = match std::str::from_utf8(bytes) {
        Ok(v) => v,
        Err(_) => return std::ptr::null_mut(),
    };
    let vm = vm();
    let obj: rustpython_vm::PyObjectRef = vm.ctx.new_str(rust_str).into();
    pyobject_ref_to_ptr(obj)
}

/// Create a Python string from a null-terminated C string. Returns a new reference.
///
/// # Safety
/// `u` must be null-terminated valid UTF-8.
#[inline]
pub unsafe fn PyUnicode_FromString(u: *const c_char) -> *mut PyObject {
    if u.is_null() {
        return std::ptr::null_mut();
    }
    let cstr = std::ffi::CStr::from_ptr(u);
    let rust_str = match cstr.to_str() {
        Ok(v) => v,
        Err(_) => return std::ptr::null_mut(),
    };
    let vm = vm();
    let obj: rustpython_vm::PyObjectRef = vm.ctx.new_str(rust_str).into();
    pyobject_ref_to_ptr(obj)
}
