use rustpython_vm::{builtins::PyBytes as RpBytes, PyObjectRef};

use crate::{instance::Bound, python::Python};

/// Marker type for a Python `bytes` object.
pub struct PyBytes;

impl PyBytes {
    pub fn new<'py>(py: Python<'py>, data: &[u8]) -> Bound<'py, PyBytes> {
        let obj: PyObjectRef = py.vm.ctx.new_bytes(data.to_vec()).into();
        Bound::from_object(py, obj)
    }

    /// Create a new Python `bytes` object of `len` bytes, initialized by a
    /// callback. This mirrors pyo3's `PyBytes::new_with`.
    pub fn new_with<'py>(
        py: Python<'py>,
        len: usize,
        init: impl FnOnce(&mut [u8]) -> crate::PyResult<()>,
    ) -> crate::PyResult<Bound<'py, PyBytes>> {
        let mut data = vec![0u8; len];
        init(&mut data)?;
        Ok(PyBytes::new(py, &data))
    }
}

impl<'py> Bound<'py, PyBytes> {
    /// Access the underlying byte data.
    pub fn as_bytes(&self) -> &[u8] {
        let pybytes = self
            .obj
            .downcast_ref::<RpBytes>()
            .expect("Bound<PyBytes> must wrap a bytes");
        pybytes.as_bytes()
    }
}
