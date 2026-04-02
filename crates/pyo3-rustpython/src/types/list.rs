use rustpython_vm::{
    builtins::PyList as RpList,
    convert::ToPyObject,
    PyObjectRef,
};

use crate::{
    err::PyResult,
    instance::Bound,
    python::Python,
    types::PyAny,
};

/// Marker type for a Python `list` object.
pub struct PyList;

impl<'py> Bound<'py, PyList> {
    /// Create a new list from a slice of `Bound<'py, PyAny>` elements.
    pub fn new(py: Python<'py>, elements: &[Bound<'py, PyAny>]) -> Bound<'py, PyList> {
        let vm = py.vm;
        let elems: Vec<PyObjectRef> = elements.iter().map(|e| e.obj.clone()).collect();
        let obj: PyObjectRef = vm.ctx.new_list(elems).into();
        Bound::from_object(py, obj)
    }

    /// Create an empty list.
    pub fn empty(py: Python<'py>) -> Bound<'py, PyList> {
        let obj: PyObjectRef = py.vm.ctx.new_list(vec![]).into();
        Bound::from_object(py, obj)
    }

    /// Get the item at the given index.
    pub fn get_item(&self, index: usize) -> PyResult<Bound<'py, PyAny>> {
        let vm = self.py.vm;
        let list = self.obj.downcast_ref::<RpList>().expect("Bound<PyList> must wrap a list");
        let elements = list.borrow_vec();
        if index < elements.len() {
            Ok(Bound::from_object(self.py, elements[index].clone()))
        } else {
            Err(crate::PyErr::from_vm_err(
                vm.new_index_error("list index out of range"),
            ))
        }
    }

    /// Set the item at the given index.
    pub fn set_item(&self, index: usize, value: impl ToPyObject) -> PyResult<()> {
        let vm = self.py.vm;
        let val_obj = value.to_pyobject(vm);
        let list = self.obj.downcast_ref::<RpList>().expect("Bound<PyList> must wrap a list");
        let mut elements = list.borrow_vec_mut();
        if index < elements.len() {
            elements[index] = val_obj;
            Ok(())
        } else {
            Err(crate::PyErr::from_vm_err(
                vm.new_index_error("list assignment index out of range"),
            ))
        }
    }

    /// Append a value to the end of the list.
    pub fn append(&self, value: impl ToPyObject) -> PyResult<()> {
        let vm = self.py.vm;
        let val_obj = value.to_pyobject(vm);
        let list = self.obj.downcast_ref::<RpList>().expect("Bound<PyList> must wrap a list");
        list.borrow_vec_mut().push(val_obj);
        Ok(())
    }

    /// Return the number of items in the list.
    pub fn len(&self) -> usize {
        let list = self.obj.downcast_ref::<RpList>().expect("Bound<PyList> must wrap a list");
        list.borrow_vec().len()
    }

    /// Return `true` if the list is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
