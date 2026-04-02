use pyo3::prelude::*;

#[pyclass]
#[derive(Clone)]
pub struct Point {
    #[pyo3(get, set)]
    pub x: f64,
    #[pyo3(get, set)]
    pub y: f64,
}

#[pymethods]
impl Point {
    #[new]
    fn new(x: f64, y: f64) -> Self {
        Point { x, y }
    }

    fn distance(&self) -> f64 {
        (self.x * self.x + self.y * self.y).sqrt()
    }

    fn translate(&self, dx: f64, dy: f64) -> Point {
        Point {
            x: self.x + dx,
            y: self.y + dy,
        }
    }

    fn __repr__(&self) -> String {
        format!("Point({}, {})", self.x, self.y)
    }

    fn __str__(&self) -> String {
        format!("({}, {})", self.x, self.y)
    }
}

#[pymodule]
fn point(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Point>()?;
    Ok(())
}
