use rustpython_vm::{builtins::PyStr as RpStr, PyObjectRef};

use crate::{err::PyResult, instance::Bound, python::Python};

/// Marker type for a Python `str` object.
pub struct PyString;

impl PyString {
    /// Create a new Python string from a Rust `&str`.
    pub fn new<'py>(py: Python<'py>, s: &str) -> Bound<'py, PyString> {
        let obj: PyObjectRef = py.vm.ctx.new_str(s).into();
        Bound::from_object(py, obj)
    }
}

impl<'py> Bound<'py, PyString> {
    /// Create a new Python string from a Rust `&str`.
    pub fn new(py: Python<'py>, s: &str) -> Bound<'py, PyString> {
        let obj: PyObjectRef = py.vm.ctx.new_str(s).into();
        Bound::from_object(py, obj)
    }

    /// Extract the string value as a `&str`.
    ///
    /// Returns an error if the underlying object is not a valid Python `str`.
    pub fn to_str(&self) -> PyResult<&str> {
        let pystr = self
            .obj
            .downcast_ref::<RpStr>()
            .expect("Bound<PyString> must wrap a str");
        Ok(pystr.as_str())
    }

    /// Extract the string value, replacing invalid data with the Unicode
    /// replacement character.
    pub fn to_string_lossy(&self) -> String {
        match self.obj.downcast_ref::<RpStr>() {
            Some(pystr) => pystr.as_str().to_owned(),
            None => String::from("<invalid str>"),
        }
    }

    // extract() is defined generically on Bound<'py, T> in instance.rs
}
