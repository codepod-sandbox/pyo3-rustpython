use rustpython_vm::protocol::{PyIter, PyIterReturn};

use crate::{
    err::PyResult,
    instance::Bound,
    types::PyAny,
};

/// Marker type for a Python iterator object.
pub struct PyIterator;

impl<'py> Bound<'py, PyIterator> {
    /// Create an iterator from any iterable Python object.
    ///
    /// This calls `iter(obj)` on the Python side to obtain the iterator.
    pub fn from_bound_any(obj: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyIterator>> {
        let vm = obj.py.vm;
        let iter_obj = vm
            .call_method(&obj.obj, "__iter__", ())
            .map_err(crate::PyErr::from_vm_err)?;
        Ok(Bound::from_object(obj.py, iter_obj))
    }
}

impl<'py> Iterator for Bound<'py, PyIterator> {
    type Item = PyResult<Bound<'py, PyAny>>;

    fn next(&mut self) -> Option<Self::Item> {
        let vm = self.py.vm;
        let iter = PyIter::new(&*self.obj);
        match iter.next(vm) {
            Ok(PyIterReturn::Return(obj)) => Some(Ok(Bound::from_object(self.py, obj))),
            Ok(PyIterReturn::StopIteration(_)) => None,
            Err(e) => Some(Err(crate::PyErr::from_vm_err(e))),
        }
    }
}
