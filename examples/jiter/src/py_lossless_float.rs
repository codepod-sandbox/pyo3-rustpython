//! Simplified FloatMode for RustPython — no LosslessFloat pyclass for now.

use pyo3::prelude::*;
use pyo3::exceptions::PyValueError;

#[derive(Debug, Clone, Copy, Default)]
#[allow(dead_code)]
pub enum FloatMode {
    #[default]
    Float,
    Decimal,
    LosslessFloat,
}

#[allow(dead_code)]
pub fn get_decimal_type<'py>(_py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
    Err(PyValueError::new_err(
        "Decimal type is not supported in RustPython jiter shim",
    ))
}
