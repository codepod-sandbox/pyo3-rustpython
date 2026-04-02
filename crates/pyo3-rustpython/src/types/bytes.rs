use rustpython_vm::{
    builtins::PyBytes as RpBytes,
    PyObjectRef,
};

use crate::{
    instance::Bound,
    python::Python,
};

/// Marker type for a Python `bytes` object.
pub struct PyBytes;

impl<'py> Bound<'py, PyBytes> {
    /// Create a new Python `bytes` object from a byte slice.
    pub fn new(py: Python<'py>, data: &[u8]) -> Bound<'py, PyBytes> {
        let obj: PyObjectRef = py.vm.ctx.new_bytes(data.to_vec()).into();
        Bound::from_object(py, obj)
    }

    /// Access the underlying byte data.
    pub fn as_bytes(&self) -> &[u8] {
        let pybytes = self
            .obj
            .downcast_ref::<RpBytes>()
            .expect("Bound<PyBytes> must wrap a bytes");
        pybytes.as_bytes()
    }
}
