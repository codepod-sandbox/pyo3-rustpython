use rustpython_vm::{
    builtins::PyList as RpList,
    convert::ToPyObject,
    protocol::{PyIter, PyIterReturn},
    PyObjectRef,
};

use crate::{err::PyResult, instance::Bound, python::Python, types::PyAny};

/// Marker type for a Python `list` object.
pub struct PyList;

impl PyList {
    pub fn new<'py, T: crate::conversion::ToPyObject>(
        py: Python<'py>,
        elements: impl IntoIterator<Item = T>,
    ) -> PyResult<Bound<'py, PyList>> {
        let vm = py.vm;
        let elems: Vec<PyObjectRef> = elements
            .into_iter()
            .map(|e| crate::conversion::ToPyObject::to_object(&e, py).obj)
            .collect();
        let obj: PyObjectRef = vm.ctx.new_list(elems).into();
        Ok(Bound::from_object(py, obj))
    }

    pub fn empty<'py>(py: Python<'py>) -> Bound<'py, PyList> {
        let obj: PyObjectRef = py.vm.ctx.new_list(vec![]).into();
        Bound::from_object(py, obj)
    }
}

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
        let list = self
            .obj
            .downcast_ref::<RpList>()
            .expect("Bound<PyList> must wrap a list");
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
        let list = self
            .obj
            .downcast_ref::<RpList>()
            .expect("Bound<PyList> must wrap a list");
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
        let list = self
            .obj
            .downcast_ref::<RpList>()
            .expect("Bound<PyList> must wrap a list");
        list.borrow_vec_mut().push(val_obj);
        Ok(())
    }

    /// Return the number of items in the list.
    pub fn len(&self) -> usize {
        let list = self
            .obj
            .downcast_ref::<RpList>()
            .expect("Bound<PyList> must wrap a list");
        list.borrow_vec().len()
    }

    /// Return `true` if the list is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Iterate over the elements of this list (or list-like iterable).
    pub fn iter(&self) -> BoundListIterator<'py> {
        BoundListIterator::new(self.py, &self.obj)
    }
}

/// Iterator over elements of a `Bound<'py, PyList>` (or any iterable via Python protocol).
pub struct BoundListIterator<'py> {
    py: crate::python::Python<'py>,
    items: Vec<PyObjectRef>,
    index: usize,
}

impl<'py> BoundListIterator<'py> {
    fn new(py: crate::python::Python<'py>, obj: &PyObjectRef) -> Self {
        let vm = py.vm;
        // Try to collect eagerly from the Python iterator.
        let mut items = Vec::new();
        // Use the Python iteration protocol: get __iter__ then __next__
        if let Ok(iter_obj) = vm.call_method(obj, "__iter__", ()) {
            let iter = PyIter::new(&*iter_obj);
            loop {
                match iter.next(vm) {
                    Ok(PyIterReturn::Return(item)) => items.push(item),
                    Ok(PyIterReturn::StopIteration(_)) => break,
                    Err(_) => break,
                }
            }
        } else {
            // Already is an iterator (no __iter__ returning new iter), try direct iteration
            let iter = PyIter::new(&**obj);
            loop {
                match iter.next(vm) {
                    Ok(PyIterReturn::Return(item)) => items.push(item),
                    Ok(PyIterReturn::StopIteration(_)) => break,
                    Err(_) => break,
                }
            }
        }
        BoundListIterator {
            py,
            items,
            index: 0,
        }
    }
}

impl<'py> Iterator for BoundListIterator<'py> {
    type Item = Bound<'py, PyAny>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.items.len() {
            let item = self.items[self.index].clone();
            self.index += 1;
            Some(Bound::from_object(self.py, item))
        } else {
            None
        }
    }
}
