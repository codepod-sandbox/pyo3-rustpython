use rustpython_vm::PyObjectRef;

use crate::{
    instance::Bound,
    python::Python,
};

/// Marker type for Python's `None` singleton.
pub struct PyNone;

impl<'py> Bound<'py, PyNone> {
    /// Get the Python `None` singleton.
    pub fn get(py: Python<'py>) -> Bound<'py, PyNone> {
        let obj: PyObjectRef = py.vm.ctx.none();
        Bound::from_object(py, obj)
    }
}
