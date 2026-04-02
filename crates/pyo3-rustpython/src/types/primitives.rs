use rustpython_vm::builtins::{PyFloat as RpFloat, PyInt as RpInt};
use rustpython_vm::{AsObject, PyObjectRef};

use crate::{
    instance::Bound,
    python::Python,
};

/// Marker type for a Python `bool` object.
pub struct PyBool;

/// Marker type for a Python `float` object.
pub struct PyFloat;

/// Marker type for a Python `int` object.
pub struct PyInt;

/// Alias matching pyo3's `PyLong`.
pub type PyLong = PyInt;

impl<'py> Bound<'py, PyBool> {
    /// Create a new Python bool.
    pub fn new(py: Python<'py>, val: bool) -> Bound<'py, PyBool> {
        let obj: PyObjectRef = py.vm.ctx.new_bool(val).into();
        Bound::from_object(py, obj)
    }

    /// Return `true` if this Python bool is `True`.
    pub fn is_true(&self) -> bool {
        let vm = self.py.vm;
        self.obj.is(&vm.ctx.true_value)
    }
}

impl<'py> Bound<'py, PyFloat> {
    /// Create a new Python float.
    pub fn new(py: Python<'py>, val: f64) -> Bound<'py, PyFloat> {
        let obj: PyObjectRef = py.vm.ctx.new_float(val).into();
        Bound::from_object(py, obj)
    }

    /// Extract the `f64` value.
    pub fn value(&self) -> f64 {
        let pyfloat = self
            .obj
            .downcast_ref::<RpFloat>()
            .expect("Bound<PyFloat> must wrap a float");
        pyfloat.to_f64()
    }
}

impl<'py> Bound<'py, PyInt> {
    /// Create a new Python int from an `i64`.
    pub fn new(py: Python<'py>, val: i64) -> Bound<'py, PyInt> {
        let obj: PyObjectRef = py.vm.ctx.new_int(val).into();
        Bound::from_object(py, obj)
    }

    /// Try to extract the value as an `i64`.
    ///
    /// Returns `None` if the value does not fit in an `i64`.
    pub fn value(&self) -> Option<i64> {
        let pyint = self
            .obj
            .downcast_ref::<RpInt>()
            .expect("Bound<PyInt> must wrap an int");
        i64::try_from(pyint.as_bigint()).ok()
    }
}
