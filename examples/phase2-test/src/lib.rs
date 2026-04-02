use pyo3::prelude::*;
use pyo3::exceptions::{PyValueError, PyTypeError};

/// A class that exercises Phase 2 features: conversions, type wrappers,
/// PyAnyMethods, and exceptions.
#[pyclass]
#[derive(Clone)]
pub struct Converter {
    #[pyo3(get)]
    pub label: String,
}

#[pymethods]
impl Converter {
    #[new]
    fn new(label: String) -> Self {
        Converter { label }
    }

    /// Test FromPyObject: extract various types from Python objects
    fn extract_int(&self, val: i64) -> i64 {
        val * 2
    }

    fn extract_float(&self, val: f64) -> f64 {
        val + 0.5
    }

    fn extract_bool(&self, val: bool) -> bool {
        !val
    }

    fn extract_string(&self, val: String) -> String {
        format!("{}:{}", self.label, val)
    }

    /// Test returning i64 (basic IntoPyObject)
    fn double(&self, val: i64) -> i64 {
        val * 2
    }

    /// Test error handling with exceptions
    fn validate(&self, value: i64) -> PyResult<String> {
        if value < 0 {
            Err(PyValueError::new_err("value must be non-negative"))
        } else if value > 100 {
            Err(PyTypeError::new_err("value must be <= 100"))
        } else {
            Ok(format!("valid: {}", value))
        }
    }

    fn __repr__(&self) -> String {
        format!("Converter('{}')", self.label)
    }
}

#[pymodule]
fn phase2(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Converter>()?;
    Ok(())
}
