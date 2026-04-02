use rustpython_vm::{builtins::PyBaseException, PyRef};

/// A Python exception. Analogous to PyO3's `PyErr`.
pub struct PyErr {
    pub(crate) inner: PyRef<PyBaseException>,
}

pub type PyResult<T = ()> = Result<T, PyErr>;

impl PyErr {
    /// Construct from a RustPython base exception reference.
    #[doc(hidden)]
    pub fn from_vm_err(e: PyRef<PyBaseException>) -> Self {
        PyErr { inner: e }
    }

    /// Convert back to a RustPython error (for returning from exec slots).
    #[doc(hidden)]
    pub fn into_vm_err(self) -> PyRef<PyBaseException> {
        self.inner
    }

    /// Create a `ValueError` with the given message.
    pub fn new_value_error(py: crate::Python<'_>, msg: impl Into<String>) -> Self {
        PyErr { inner: py.vm.new_value_error(msg.into()) }
    }

    /// Create a `TypeError` with the given message.
    pub fn new_type_error(py: crate::Python<'_>, msg: impl Into<String>) -> Self {
        PyErr { inner: py.vm.new_type_error(msg.into()) }
    }
}

impl From<PyRef<PyBaseException>> for PyErr {
    fn from(e: PyRef<PyBaseException>) -> Self {
        PyErr { inner: e }
    }
}

/// Convert a `rustpython_vm::PyResult<T>` into our `PyResult<T>`.
pub(crate) fn from_vm_result<T>(
    r: rustpython_vm::PyResult<T>,
) -> PyResult<T> {
    r.map_err(PyErr::from_vm_err)
}

/// Helper used by generated exec-slot code.
/// Takes our `PyErr` and produces a `rustpython_vm::PyBaseExceptionRef`
/// so it can be returned from a `ModuleExec` function.
#[doc(hidden)]
pub fn into_vm_err(e: PyErr) -> PyRef<PyBaseException> {
    e.inner
}

/// Extension trait so generated code can write `.map_err(PyErr::vm_into_err)`.
pub trait IntoVmError {
    fn into_vm_err(self) -> PyRef<PyBaseException>;
}

impl IntoVmError for PyErr {
    fn into_vm_err(self) -> PyRef<PyBaseException> {
        self.inner
    }
}
