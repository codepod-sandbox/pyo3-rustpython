use std::marker::PhantomData;

use rustpython_vm::PyObjectRef;

use crate::python::Python;

/// An owned Python object reference with a type tag. Analogous to PyO3's `Py<T>`.
///
/// The type parameter `T` is a marker only; the underlying object is a
/// `PyObjectRef` (ref-counted `PyObject`).
pub struct Py<T> {
    pub(crate) obj: PyObjectRef,
    _marker: PhantomData<T>,
}

impl<T> Py<T> {
    #[doc(hidden)]
    pub fn from_object(obj: PyObjectRef) -> Self {
        Py { obj, _marker: PhantomData }
    }

    pub fn into_object(self) -> PyObjectRef {
        self.obj
    }
}

impl<T> Clone for Py<T> {
    fn clone(&self) -> Self {
        Py { obj: self.obj.clone(), _marker: PhantomData }
    }
}

/// A borrowed Python object reference tied to a `Python<'py>` lifetime token.
/// Analogous to PyO3's `Bound<'py, T>`.
pub struct Bound<'py, T> {
    pub(crate) py: Python<'py>,
    pub(crate) obj: PyObjectRef,
    _marker: PhantomData<T>,
}

impl<'py, T> Bound<'py, T> {
    /// Construct from a raw `PyObjectRef`.
    #[doc(hidden)]
    pub fn from_object(py: Python<'py>, obj: PyObjectRef) -> Self {
        Bound { py, obj, _marker: PhantomData }
    }

    /// Return the `Python<'py>` token this reference is tied to.
    pub fn py(&self) -> Python<'py> {
        self.py
    }

    /// Access the inner `PyObjectRef`.
    pub fn as_pyobject(&self) -> &PyObjectRef {
        &self.obj
    }

    /// Erase the type tag, returning `Bound<'py, PyAny>`.
    pub fn as_any(&self) -> &Bound<'py, crate::types::PyAny> {
        // SAFETY: Bound is #[repr(C)] with identical layout for all T.
        unsafe { &*(self as *const Bound<'py, T> as *const Bound<'py, crate::types::PyAny>) }
    }

    /// Convert to an owned, untyped `Bound<'py, PyAny>`.
    pub fn into_any(self) -> Bound<'py, crate::types::PyAny> {
        Bound { py: self.py, obj: self.obj, _marker: PhantomData }
    }
}

impl<'py, T> Clone for Bound<'py, T> {
    fn clone(&self) -> Self {
        Bound { py: self.py, obj: self.obj.clone(), _marker: PhantomData }
    }
}
