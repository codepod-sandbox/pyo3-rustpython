use rustpython_vm::{
    builtins::PyTuple as RpTuple,
    PyObjectRef,
};

use crate::{
    err::PyResult,
    instance::Bound,
    python::Python,
    types::PyAny,
};

/// Marker type for a Python `tuple` object.
pub struct PyTuple;

impl<'py> Bound<'py, PyTuple> {
    /// Create a new tuple from a slice of `Bound<'py, PyAny>` elements.
    pub fn new(py: Python<'py>, elements: &[Bound<'py, PyAny>]) -> Bound<'py, PyTuple> {
        let vm = py.vm;
        let elems: Vec<PyObjectRef> = elements.iter().map(|e| e.obj.clone()).collect();
        let obj: PyObjectRef = vm.ctx.new_tuple(elems).into();
        Bound::from_object(py, obj)
    }

    /// Create an empty tuple.
    pub fn empty(py: Python<'py>) -> Bound<'py, PyTuple> {
        let obj: PyObjectRef = py.vm.ctx.empty_tuple.clone().into();
        Bound::from_object(py, obj)
    }

    /// Get the item at the given index.
    pub fn get_item(&self, index: usize) -> PyResult<Bound<'py, PyAny>> {
        let vm = self.py.vm;
        let tuple = self.obj.downcast_ref::<RpTuple>().expect("Bound<PyTuple> must wrap a tuple");
        let elements = tuple.as_slice();
        if index < elements.len() {
            Ok(Bound::from_object(self.py, elements[index].clone()))
        } else {
            Err(crate::PyErr::from_vm_err(
                vm.new_index_error("tuple index out of range"),
            ))
        }
    }

    /// Return the number of items in the tuple.
    pub fn len(&self) -> usize {
        let tuple = self.obj.downcast_ref::<RpTuple>().expect("Bound<PyTuple> must wrap a tuple");
        tuple.as_slice().len()
    }

    /// Return `true` if the tuple is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
