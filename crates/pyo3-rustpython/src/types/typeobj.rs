use rustpython_vm::builtins::PyType as RpType;

use crate::{
    err::PyResult,
    instance::Bound,
};

/// Marker type for a Python `type` object.
pub struct PyType;

impl<'py> Bound<'py, PyType> {
    /// Return the name of this type.
    pub fn name(&self) -> PyResult<String> {
        let pytype = self
            .obj
            .downcast_ref::<RpType>()
            .expect("Bound<PyType> must wrap a type");
        Ok(pytype.name().to_string())
    }
}
