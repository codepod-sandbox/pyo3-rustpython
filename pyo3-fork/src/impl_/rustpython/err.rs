use rustpython_vm::{builtins::PyBaseException, PyRef};
use super::marker::Python;

pub struct PyErr {
    pub(crate) inner: PyRef<PyBaseException>,
}

pub type PyResult<T = ()> = Result<T, PyErr>;

impl PyErr {
    pub fn from_vm_err(e: PyRef<PyBaseException>) -> Self { PyErr { inner: e } }
    pub fn into_vm_err(self) -> PyRef<PyBaseException> { self.inner }

    pub fn new_value_error(py: Python<'_>, msg: impl Into<String>) -> Self {
        PyErr { inner: py.vm().new_value_error(msg.into()) }
    }

    pub fn new_type_error(py: Python<'_>, msg: impl Into<String>) -> Self {
        PyErr { inner: py.vm().new_type_error(msg.into()) }
    }
}

impl From<PyRef<PyBaseException>> for PyErr {
    fn from(e: PyRef<PyBaseException>) -> Self { PyErr { inner: e } }
}

pub fn from_vm_result<T>(r: rustpython_vm::PyResult<T>) -> PyResult<T> {
    r.map_err(PyErr::from_vm_err)
}

#[doc(hidden)]
pub fn into_vm_err(e: PyErr) -> PyRef<PyBaseException> { e.inner }
