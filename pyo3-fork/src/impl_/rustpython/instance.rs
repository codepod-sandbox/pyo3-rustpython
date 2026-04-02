use std::marker::PhantomData;
use rustpython_vm::PyObjectRef;
use super::marker::Python;

pub struct Py<T> {
    pub(crate) obj: PyObjectRef,
    _marker: PhantomData<T>,
}

impl<T> Py<T> {
    pub fn from_object(obj: PyObjectRef) -> Self {
        Py { obj, _marker: PhantomData }
    }

    pub fn into_object(self) -> PyObjectRef { self.obj }

    /// Returns &Bound via pointer cast — safe because Python<'py> is ZST,
    /// so Bound and Py have the same layout.
    pub fn bind<'py>(&self, _py: Python<'py>) -> &Bound<'py, T> {
        unsafe { &*(self as *const Py<T> as *const Bound<'py, T>) }
    }

    pub fn clone_ref(&self, _py: Python<'_>) -> Self {
        Py { obj: self.obj.clone(), _marker: PhantomData }
    }
}

impl<T> Clone for Py<T> {
    fn clone(&self) -> Self {
        Py { obj: self.obj.clone(), _marker: PhantomData }
    }
}

// RUSTPYTHON-ASSUMPTION: single-threaded
unsafe impl<T> Send for Py<T> {}
unsafe impl<T> Sync for Py<T> {}

pub struct Bound<'py, T> {
    py: Python<'py>,       // ZST — zero bytes
    pub(crate) obj: PyObjectRef,
    _marker: PhantomData<T>,
}

impl<'py, T> Bound<'py, T> {
    pub fn from_object(py: Python<'py>, obj: PyObjectRef) -> Self {
        Bound { py, obj, _marker: PhantomData }
    }

    pub fn py(&self) -> Python<'py> { self.py }

    pub fn as_pyobject(&self) -> &PyObjectRef { &self.obj }

    pub fn into_pyobject_ref(self) -> PyObjectRef { self.obj }

    pub fn as_any(&self) -> &Bound<'py, crate::types::PyAny> {
        unsafe { &*(self as *const Bound<'py, T> as *const Bound<'py, crate::types::PyAny>) }
    }

    pub fn into_any(self) -> Bound<'py, crate::types::PyAny> {
        Bound { py: self.py, obj: self.obj, _marker: PhantomData }
    }

    pub fn unbind(self) -> Py<T> { Py::from_object(self.obj) }
}

impl<'py, T> Clone for Bound<'py, T> {
    fn clone(&self) -> Self {
        Bound { py: self.py, obj: self.obj.clone(), _marker: PhantomData }
    }
}

pub type PyObject = Py<crate::types::PyAny>;
