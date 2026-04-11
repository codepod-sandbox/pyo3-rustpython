use rustpython_vm::builtins::{PyFloat as RpFloat, PyInt as RpInt};
use rustpython_vm::{AsObject, PyObjectRef};

use crate::{instance::Bound, python::Python};

pub struct PyBool;

impl PyBool {
    pub fn new<'py>(py: Python<'py>, val: bool) -> Bound<'py, PyBool> {
        let obj: PyObjectRef = py.vm.ctx.new_bool(val).into();
        Bound::from_object(py, obj)
    }
}

pub struct PyFloat;

impl PyFloat {
    pub fn new<'py>(py: Python<'py>, val: f64) -> Bound<'py, PyFloat> {
        let obj: PyObjectRef = py.vm.ctx.new_float(val).into();
        Bound::from_object(py, obj)
    }
}

pub struct PyInt;

pub type PyLong = PyInt;

impl PyInt {
    pub fn new<'py, V: rustpython_vm::convert::ToPyObject>(
        py: Python<'py>,
        val: V,
    ) -> Bound<'py, PyInt> {
        let obj: PyObjectRef = val.to_pyobject(py.vm);
        Bound::from_object(py, obj)
    }
}

impl<'py> Bound<'py, PyBool> {
    pub fn new(py: Python<'py>, val: bool) -> Bound<'py, PyBool> {
        PyBool::new(py, val)
    }

    pub fn new_bound(py: Python<'py>, val: bool) -> Bound<'py, PyBool> {
        PyBool::new(py, val)
    }

    pub fn is_true(&self) -> bool {
        let vm = self.py.vm;
        self.obj.is(&vm.ctx.true_value)
    }
}

impl<'py> Bound<'py, PyFloat> {
    pub fn new(py: Python<'py>, val: f64) -> Bound<'py, PyFloat> {
        PyFloat::new(py, val)
    }

    pub fn new_bound(py: Python<'py>, val: f64) -> Bound<'py, PyFloat> {
        PyFloat::new(py, val)
    }

    pub fn value(&self) -> f64 {
        let pyfloat = self
            .obj
            .downcast_ref::<RpFloat>()
            .expect("Bound<PyFloat> must wrap a float");
        pyfloat.to_f64()
    }
}

impl<'py> Bound<'py, PyInt> {
    pub fn new_bound(py: Python<'py>, val: i64) -> Bound<'py, PyInt> {
        PyInt::new(py, val)
    }

    pub fn value(&self) -> Option<i64> {
        let pyint = self
            .obj
            .downcast_ref::<RpInt>()
            .expect("Bound<PyInt> must wrap an int");
        i64::try_from(pyint.as_bigint()).ok()
    }
}
