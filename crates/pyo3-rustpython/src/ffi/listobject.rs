//! List object FFI functions.

use rustpython_vm::builtins::PyList;
use rustpython_vm::PyPayload;

use super::ffi_object::*;
use super::pyport::Py_ssize_t;

/// Create a new Python list of the given size.
///
/// Returns a new reference. Elements are set to `None`.
///
/// # Safety
/// Caller must eventually `Py_DECREF` the returned pointer.
#[inline]
pub unsafe fn PyList_New(size: Py_ssize_t) -> *mut PyObject {
    let vm = vm();
    let list_ref = PyList::default().into_ref(&vm.ctx);
    if size > 0 {
        let none = vm.ctx.none();
        let mut elements = list_ref.borrow_vec_mut();
        for _ in 0..size {
            elements.push(none.clone());
        }
    }
    let obj: rustpython_vm::PyObjectRef = list_ref.into();
    pyobject_ref_to_ptr(obj)
}

/// Set item at position `i` in the list. **Steals** the reference to `item`.
///
/// Returns 0 on success, -1 on failure.
///
/// # Safety
/// `list` must be a valid list. `item` ownership is transferred. `i` must be valid.
#[inline]
pub unsafe fn PyList_SetItem(
    list: *mut PyObject,
    i: Py_ssize_t,
    item: *mut PyObject,
) -> std::os::raw::c_int {
    if list.is_null() {
        return -1;
    }
    let list_ref = ptr_to_pyobject_ref_borrowed(list);
    let list_inner = match list_ref.downcast_ref::<PyList>() {
        Some(l) => l,
        None => return -1,
    };
    let item_ref = if item.is_null() {
        vm().ctx.none()
    } else {
        ptr_to_pyobject_ref_owned(item)
    };
    let mut elements = list_inner.borrow_vec_mut();
    if i < 0 || (i as usize) >= elements.len() {
        return -1;
    }
    elements[i as usize] = item_ref;
    0
}

/// Get item at position `i` (returns a NEW reference).
///
/// # Safety
/// `list` must be a valid list. `i` must be a valid index.
#[inline]
pub unsafe fn PyList_GET_ITEM(list: *mut PyObject, i: Py_ssize_t) -> *mut PyObject {
    if list.is_null() {
        return std::ptr::null_mut();
    }
    let list_ref = ptr_to_pyobject_ref_borrowed(list);
    let list_inner = match list_ref.downcast_ref::<PyList>() {
        Some(l) => l,
        None => return std::ptr::null_mut(),
    };
    let elements = list_inner.borrow_vec();
    if i < 0 || (i as usize) >= elements.len() {
        return std::ptr::null_mut();
    }
    pyobject_ref_to_ptr(elements[i as usize].clone())
}

/// Get the length of the list.
///
/// # Safety
/// `list` must be a valid list object.
#[inline]
pub unsafe fn PyList_GET_SIZE(list: *mut PyObject) -> Py_ssize_t {
    if list.is_null() {
        return 0;
    }
    let list_ref = ptr_to_pyobject_ref_borrowed(list);
    match list_ref.downcast_ref::<PyList>() {
        Some(l) => l.__len__() as Py_ssize_t,
        None => 0,
    }
}

/// Get the length of the list (safe version, returns -1 on error).
///
/// # Safety
/// `list` must be a valid list object.
#[inline]
pub unsafe fn PyList_Size(list: *mut PyObject) -> Py_ssize_t {
    if list.is_null() {
        return -1;
    }
    let list_ref = ptr_to_pyobject_ref_borrowed(list);
    match list_ref.downcast_ref::<PyList>() {
        Some(l) => l.__len__() as Py_ssize_t,
        None => -1,
    }
}
