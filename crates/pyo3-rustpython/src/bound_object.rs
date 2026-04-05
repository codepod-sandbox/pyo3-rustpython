/// Trait that `Bound<'py, T>` and `Borrowed<'_, 'py, T>` implement.
///
/// Provides `.into_any()` and `.unbind()` in a generic way,
/// matching pyo3's `BoundObject` trait.
pub trait BoundObject<'py, T> {
    /// Erase the type parameter, returning a `Bound<'py, PyAny>`.
    fn into_any(self) -> crate::Bound<'py, crate::types::PyAny>;

    /// Detach from the `Python<'py>` lifetime, returning a `Py<T>`.
    fn unbind(self) -> crate::Py<crate::types::PyAny>;
}

impl<'py, T> BoundObject<'py, T> for crate::Bound<'py, T> {
    fn into_any(self) -> crate::Bound<'py, crate::types::PyAny> {
        crate::Bound::from_object(self.py, self.obj)
    }

    fn unbind(self) -> crate::Py<crate::types::PyAny> {
        crate::Py::from_object(self.obj)
    }
}

impl<'a, 'py, T> BoundObject<'py, T> for crate::Borrowed<'a, 'py, T> {
    fn into_any(self) -> crate::Bound<'py, crate::types::PyAny> {
        crate::Bound::from_object(self.py, self.obj)
    }

    fn unbind(self) -> crate::Py<crate::types::PyAny> {
        crate::Py::from_object(self.obj)
    }
}
