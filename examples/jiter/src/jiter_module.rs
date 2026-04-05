use pyo3::prelude::*;

use crate::python::{map_json_error, PythonParse};

#[pyfunction]
fn from_json(py: Python<'_>, json_data: Vec<u8>) -> PyResult<Py<PyAny>> {
    let parse = PythonParse::default();
    parse
        .python_parse(py, &json_data)
        .map(|bound| bound.unbind())
        .map_err(|e| map_json_error(&json_data, &e))
}

#[pyfunction]
fn cache_clear() {
    crate::py_string_cache::cache_clear();
}

#[pyfunction]
fn cache_usage() -> usize {
    crate::py_string_cache::cache_usage()
}

#[pymodule]
fn jiter_mod(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(from_json, m)?)?;
    m.add_function(wrap_pyfunction!(cache_clear, m)?)?;
    m.add_function(wrap_pyfunction!(cache_usage, m)?)?;
    Ok(())
}
