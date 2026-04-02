// A minimal PyO3 extension written against `pyo3-rustpython`.
//
// This is exactly the same code a real PyO3 extension author would write.
// The only difference is the Cargo.toml dependency alias:
//   `pyo3 = { package = "pyo3-rustpython", ... }`

use pyo3::prelude::*;

#[pyfunction]
fn greet(name: &str) -> String {
    format!("hello, {}!", name)
}

#[pymodule]
fn hello(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(greet, m)?)?;
    Ok(())
}
