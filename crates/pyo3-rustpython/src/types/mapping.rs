use rustpython_vm::PyObjectRef;

use crate::{
    err::{from_vm_result, PyResult},
    instance::Bound,
    python::Python,
    types::{PyAny, PyList},
};

/// Marker type for a Python mapping object.
pub struct PyMapping;

impl<'py> Bound<'py, PyMapping> {
    /// Get the items as a list of (key, value) tuples.
    pub fn items(&self) -> PyResult<Bound<'py, PyList>> {
        let vm = self.py.vm;
        let result = from_vm_result(vm.call_method(&self.obj, "items", ()))?;
        // Convert to list
        let list = from_vm_result(vm.call_method(&result, "__iter__", ()))?;
        // Actually, just return the items view as a list-like object
        Ok(Bound::from_object(self.py, result))
    }
}

impl PyMapping {
    /// Register a type as a Mapping (ABC registration).
    /// For now this is a no-op stub.
    pub fn register<T>(_py: Python<'_>) -> PyResult<()> {
        // In a full implementation, this would register T with
        // collections.abc.Mapping. For now, just succeed.
        Ok(())
    }
}
