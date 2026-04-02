use rustpython_vm::{builtins::PyBaseException, AsObject, PyRef};

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

    /// Create a new exception of type `T` with arguments.
    ///
    /// Usage: `PyErr::new::<PyValueError, _>("bad value")`
    pub fn new<T, A>(args: A) -> Self
    where
        T: crate::exceptions::PyExceptionType,
        A: Into<String>,
    {
        T::new_err(args)
    }

    /// Create a `ValueError` with the given message.
    pub fn new_value_error(py: crate::Python<'_>, msg: impl Into<String>) -> Self {
        PyErr { inner: py.vm.new_value_error(msg.into()) }
    }

    /// Create a `TypeError` with the given message.
    pub fn new_type_error(py: crate::Python<'_>, msg: impl Into<String>) -> Self {
        PyErr { inner: py.vm.new_type_error(msg.into()) }
    }

    /// Get the exception value as a `Bound<'py, PyAny>`.
    pub fn value<'py>(&self, py: crate::Python<'py>) -> crate::Bound<'py, crate::types::PyAny> {
        let obj: rustpython_vm::PyObjectRef = self.inner.clone().into();
        crate::Bound::from_object(py, obj)
    }

    /// Check if this exception is an instance of type `T`.
    pub fn is_instance_of<T: crate::exceptions::PyExceptionType>(&self, py: crate::Python<'_>) -> bool {
        let exc_type = T::type_object_raw(py);
        let obj: &rustpython_vm::PyObject = self.inner.as_ref();
        obj.fast_isinstance(exc_type)
    }

    /// Check if this exception matches a given type.
    pub fn matches<T: crate::exceptions::PyExceptionType>(&self, py: crate::Python<'_>) -> bool {
        self.is_instance_of::<T>(py)
    }
}

impl std::fmt::Display for PyErr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.inner.as_object())
    }
}

impl std::fmt::Debug for PyErr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "PyErr({:?})", self.inner.as_object())
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
