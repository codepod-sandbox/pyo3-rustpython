pub trait BoundObject<'py, T> {
    fn into_any(self) -> crate::Bound<'py, crate::types::PyAny>;
    fn unbind(self) -> crate::Py<T>;
}

impl<'py, T> BoundObject<'py, T> for crate::Bound<'py, T> {
    fn into_any(self) -> crate::Bound<'py, crate::types::PyAny> {
        crate::Bound::from_object(self.py, self.obj)
    }

    fn unbind(self) -> crate::Py<T> {
        crate::Py::from_object(self.obj)
    }
}

impl<'a, 'py, T> BoundObject<'py, T> for &'a crate::Bound<'py, T> {
    fn into_any(self) -> crate::Bound<'py, crate::types::PyAny> {
        crate::Bound::from_object(self.py(), self.as_pyobject().clone())
    }

    fn unbind(self) -> crate::Py<T> {
        crate::Py::from_object(self.as_pyobject().clone())
    }
}

impl<'a, 'py, T> BoundObject<'py, T> for crate::Borrowed<'a, 'py, T> {
    fn into_any(self) -> crate::Bound<'py, crate::types::PyAny> {
        crate::Bound::from_object(self.py, self.obj)
    }

    fn unbind(self) -> crate::Py<T> {
        crate::Py::from_object(self.obj)
    }
}
