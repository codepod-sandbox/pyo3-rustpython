//! Core `PyObject` type and pointer conversion infrastructure.
//!
//! ## Design
//!
//! RustPython's `PyObjectRef` is `NonNull<rustpython_vm::PyObject>` with manual
//! reference counting (`into_raw` / `from_raw`). This maps 1:1 to CPython's
//! `PyObject*` model, so we can freely convert between `*mut ffi::PyObject` and
//! `PyObjectRef`:
//!
//! - `pyobject_ref_to_ptr(obj)` → owned `*mut PyObject` (caller must `Py_DECREF`)
//! - `pyobject_ref_as_ptr(&obj)` → borrowed `*mut PyObject` (no refcount change)
//! - `ptr_to_pyobject_ref_owned(ptr)` → `PyObjectRef` taking ownership
//! - `ptr_to_pyobject_ref_borrowed(ptr)` → `PyObjectRef` incrementing refcount

use std::mem;
use std::ptr::NonNull;

use rustpython_vm::{PyObjectRef, VirtualMachine};

use crate::python::Python;

/// Opaque `PyObject` type. In CPython this has `ob_refcnt` and `ob_type` fields.
/// Here it's opaque — all access is through function calls.
#[repr(C)]
pub struct PyObject {
    _opaque: [usize; 0],
}

/// Opaque type representing `PyTypeObject*` (CPython type object pointer).
#[repr(C)]
pub struct PyTypeObject([u8; 0]);

// ---------------------------------------------------------------------------
// Pointer conversion helpers
// ---------------------------------------------------------------------------

/// Convert a `PyObjectRef` to a raw `*mut PyObject` (owned reference).
///
/// The refcount is NOT incremented; ownership is transferred to the caller.
/// The caller must eventually call `Py_DECREF`.
#[inline]
pub fn pyobject_ref_to_ptr(obj: PyObjectRef) -> *mut PyObject {
    obj.into_raw().as_ptr() as *mut PyObject
}

/// Get a raw `*mut PyObject` from a `&PyObjectRef` (borrowed reference).
///
/// Does NOT affect the refcount. The pointer is valid as long as the
/// `PyObjectRef` is alive.
#[inline]
pub fn pyobject_ref_as_ptr(obj: &PyObjectRef) -> *mut PyObject {
    let ptr: *const rustpython_vm::PyObject = &**obj;
    ptr as *mut PyObject
}

/// Convert a raw `*mut PyObject` to a `PyObjectRef` (takes ownership).
///
/// The refcount is NOT incremented; the returned `PyObjectRef` adopts the
/// existing reference. When it is dropped, the refcount is decremented.
///
/// # Safety
/// `ptr` must be a valid owned reference (from `pyobject_ref_to_ptr` etc.).
#[inline]
pub unsafe fn ptr_to_pyobject_ref_owned(ptr: *mut PyObject) -> PyObjectRef {
    let nn = NonNull::new_unchecked(ptr as *mut rustpython_vm::PyObject);
    PyObjectRef::from_raw(nn)
}

/// Alias for `ptr_to_pyobject_ref_owned` — used by `Bound::from_owned_ptr`.
#[inline]
pub unsafe fn owned_ptr_to_pyobject_ref(ptr: *mut PyObject) -> PyObjectRef {
    ptr_to_pyobject_ref_owned(ptr)
}

/// Convert a raw `*mut PyObject` to a `PyObjectRef` (borrowed, increments refcount).
///
/// # Safety
/// `ptr` must point to a valid, live Python object.
#[inline]
pub unsafe fn ptr_to_pyobject_ref_borrowed(ptr: *mut PyObject) -> PyObjectRef {
    let nn = NonNull::new_unchecked(ptr as *mut rustpython_vm::PyObject);
    let objref = PyObjectRef::from_raw(nn);
    let cloned = objref.clone();
    mem::forget(objref);
    cloned
}

/// Get the `VirtualMachine` from the current thread-local context.
///
/// Panics if called outside a RustPython interpreter context.
#[inline]
pub(super) fn vm() -> &'static VirtualMachine {
    Python::with_gil(|py| {
        let vm_ptr = py.vm as *const VirtualMachine;
        unsafe { &*vm_ptr }
    })
}

// ---------------------------------------------------------------------------
// Reference counting
// ---------------------------------------------------------------------------

/// Decrement the reference count. If it reaches zero, the object is freed.
///
/// # Safety
/// `obj` must be a valid owned reference.
#[inline]
pub unsafe fn Py_DECREF(obj: *mut PyObject) {
    if obj.is_null() {
        return;
    }
    let _ = ptr_to_pyobject_ref_owned(obj);
}

/// Increment the reference count.
///
/// # Safety
/// `obj` must point to a valid, live Python object.
#[inline]
pub unsafe fn Py_IncRef(obj: *mut PyObject) {
    if obj.is_null() {
        return;
    }
    let objref = ptr_to_pyobject_ref_borrowed(obj);
    mem::forget(objref);
}

// ---------------------------------------------------------------------------
// Type access
// ---------------------------------------------------------------------------

/// Return the type object of `obj` as a `*mut PyTypeObject`.
///
/// # Safety
/// `obj` must be a valid Python object.
#[inline]
pub unsafe fn Py_TYPE(obj: *mut PyObject) -> *mut PyTypeObject {
    if obj.is_null() {
        return std::ptr::null_mut();
    }
    let objref = ptr_to_pyobject_ref_borrowed(obj);
    let type_obj: PyObjectRef = objref.class().to_owned().into();
    pyobject_ref_to_ptr(type_obj) as *mut PyTypeObject
}

/// Check if `subtype` is a subclass of `supertype`.
///
/// # Safety
/// Both must be valid type objects.
#[inline]
pub unsafe fn PyType_IsSubtype(
    subtype: *mut PyTypeObject,
    supertype: *mut PyTypeObject,
) -> std::os::raw::c_int {
    if subtype.is_null() || supertype.is_null() {
        return 0;
    }
    let sub_ref = ptr_to_pyobject_ref_borrowed(subtype as *mut PyObject);
    let super_ref = ptr_to_pyobject_ref_borrowed(supertype as *mut PyObject);
    let vm = vm();
    match sub_ref.real_is_subclass(&super_ref, vm) {
        Ok(true) => 1,
        _ => 0,
    }
}

/// Check if `obj` is an instance of `typ`.
///
/// # Safety
/// `obj` must be a valid Python object. `typ` must be a valid type or tuple.
#[inline]
pub unsafe fn PyObject_IsInstance(obj: *mut PyObject, typ: *mut PyObject) -> std::os::raw::c_int {
    if obj.is_null() || typ.is_null() {
        return -1;
    }
    let obj_ref = ptr_to_pyobject_ref_borrowed(obj);
    let typ_ref = ptr_to_pyobject_ref_borrowed(typ);
    let vm = vm();
    match obj_ref.is_instance(&typ_ref, vm) {
        Ok(true) => 1,
        Ok(false) => 0,
        Err(_) => -1,
    }
}

/// Get `str(obj)`. Returns a new reference, or null on failure.
///
/// # Safety
/// `obj` must be a valid Python object.
#[inline]
pub unsafe fn PyObject_Str(obj: *mut PyObject) -> *mut PyObject {
    if obj.is_null() {
        return std::ptr::null_mut();
    }
    let obj_ref = ptr_to_pyobject_ref_borrowed(obj);
    let vm = vm();
    match obj_ref.str(vm) {
        Ok(s) => {
            let str_obj: PyObjectRef = s.into();
            pyobject_ref_to_ptr(str_obj)
        }
        Err(_) => std::ptr::null_mut(),
    }
}

/// Get an attribute by Python string name. Returns a new reference, or null.
///
/// # Safety
/// `obj` and `attr_name` must be valid Python objects.
#[inline]
pub unsafe fn PyObject_GetAttr(obj: *mut PyObject, attr_name: *mut PyObject) -> *mut PyObject {
    if obj.is_null() || attr_name.is_null() {
        return std::ptr::null_mut();
    }
    let obj_ref = ptr_to_pyobject_ref_borrowed(obj);
    let name_ref = ptr_to_pyobject_ref_borrowed(attr_name);
    let vm = vm();
    match vm.call_method(&obj_ref, "__getattr__", (name_ref.clone(),)) {
        Ok(val) => pyobject_ref_to_ptr(val),
        Err(_) => std::ptr::null_mut(),
    }
}

/// Get an attribute by C string name. Returns a new reference, or null.
///
/// # Safety
/// `obj` must be a valid Python object. `name` must be null-terminated.
#[inline]
pub unsafe fn PyObject_GetAttrString(
    obj: *mut PyObject,
    name: *const std::os::raw::c_char,
) -> *mut PyObject {
    if obj.is_null() || name.is_null() {
        return std::ptr::null_mut();
    }
    let cname = std::ffi::CStr::from_ptr(name);
    let rust_name = match cname.to_str() {
        Ok(s) => s,
        Err(_) => return std::ptr::null_mut(),
    };
    let obj_ref = ptr_to_pyobject_ref_borrowed(obj);
    let vm = vm();
    match obj_ref.get_attr(rust_name, vm) {
        Ok(val) => pyobject_ref_to_ptr(val),
        Err(_) => std::ptr::null_mut(),
    }
}

// ---------------------------------------------------------------------------
// Bound / Py pointer methods
// ---------------------------------------------------------------------------

impl<'py, T> crate::Bound<'py, T> {
    /// Return a raw pointer (borrowed reference). Valid while this `Bound` lives.
    #[inline]
    pub fn as_ptr(&self) -> *mut PyObject {
        pyobject_ref_as_ptr(&self.obj)
    }

    /// Convert into a raw pointer (owned reference). Caller must `Py_DECREF`.
    #[inline]
    pub fn into_ptr(self) -> *mut PyObject {
        pyobject_ref_to_ptr(self.obj)
    }

    /// Reconstruct from a borrowed pointer. Increments refcount. Returns None if null.
    ///
    /// # Safety
    /// `ptr` must point to a valid, live Python object (or be null).
    #[inline]
    pub unsafe fn from_borrowed_ptr(py: Python<'py>, ptr: *mut PyObject) -> Option<Self> {
        if ptr.is_null() {
            return None;
        }
        let objref = ptr_to_pyobject_ref_borrowed(ptr);
        Some(crate::Bound::from_object(py, objref))
    }
}

impl<'py> crate::Bound<'py, crate::types::PyAny> {
    /// Reconstruct from an owned pointer. Takes ownership (no refcount increment).
    ///
    /// Defined on `Bound<'py, PyAny>` specifically so that type inference works
    /// without a turbofish: `Bound::from_owned_ptr(py, ptr)`.
    ///
    /// # Safety
    /// `ptr` must be a valid owned reference (from `into_ptr` etc.).
    #[inline]
    pub unsafe fn from_owned_ptr(py: Python<'py>, ptr: *mut PyObject) -> Self {
        let objref = ptr_to_pyobject_ref_owned(ptr);
        crate::Bound::from_object(py, objref)
    }
}

impl<T> crate::Py<T> {
    /// Return a raw pointer (borrowed reference).
    #[inline]
    pub fn as_ptr(&self) -> *mut PyObject {
        pyobject_ref_as_ptr(&self.obj)
    }

    /// Convert into a raw pointer (owned reference). Caller must `Py_DECREF`.
    #[inline]
    pub fn into_ptr(self) -> *mut PyObject {
        pyobject_ref_to_ptr(self.obj)
    }

    /// Reconstruct from an owned pointer.
    ///
    /// # Safety
    /// Same as `Bound::from_owned_ptr`.
    #[inline]
    pub unsafe fn from_owned_ptr(_py: crate::Python<'_>, ptr: *mut PyObject) -> Self {
        let objref = ptr_to_pyobject_ref_owned(ptr);
        crate::Py::from_object(objref)
    }

    /// Reconstruct from a borrowed pointer.
    ///
    /// # Safety
    /// Same as `Bound::from_borrowed_ptr`.
    #[inline]
    pub unsafe fn from_borrowed_ptr(_py: crate::Python<'_>, ptr: *mut PyObject) -> Option<Self> {
        if ptr.is_null() {
            return None;
        }
        let objref = ptr_to_pyobject_ref_borrowed(ptr);
        Some(crate::Py::from_object(objref))
    }
}
