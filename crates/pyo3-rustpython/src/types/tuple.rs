use rustpython_vm::{builtins::PyTuple as RpTuple, PyObjectRef};

use crate::{err::PyResult, instance::Bound, python::Python, types::PyAny, ToPyObject};

/// Marker trait for PyTuple methods. This is a compatibility shim for
/// `pyo3::types::PyTupleMethods`.
pub trait PyTupleMethods<'py> {}
impl<'py> PyTupleMethods<'py> for Bound<'py, PyTuple> {}

/// Marker type for a Python `tuple` object.
pub struct PyTuple;

impl PyTuple {
    /// Create a new tuple from items that can be converted through PyO3's object protocol.
    /// This is the static method form: `PyTuple::new(py, items)`.
    pub fn new<'py, I>(py: Python<'py>, items: I) -> PyResult<Bound<'py, PyTuple>>
    where
        I: IntoIterator,
        I::Item: ToPyObject,
    {
        let vm = py.vm;
        let elems: Vec<PyObjectRef> = items.into_iter().map(|e| e.to_object(py).obj).collect();
        let obj: PyObjectRef = vm.ctx.new_tuple(elems).into();
        Ok(Bound::from_object(py, obj))
    }
}

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
        let tuple = self
            .obj
            .downcast_ref::<RpTuple>()
            .expect("Bound<PyTuple> must wrap a tuple");
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
        let tuple = self
            .obj
            .downcast_ref::<RpTuple>()
            .expect("Bound<PyTuple> must wrap a tuple");
        tuple.as_slice().len()
    }

    /// Return `true` if the tuple is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Iterator over elements of a `Bound<'py, PyTuple>`.
pub struct BoundTupleIterator<'py> {
    tuple: Bound<'py, PyTuple>,
    index: usize,
    len: usize,
}

impl<'py> Iterator for BoundTupleIterator<'py> {
    type Item = Bound<'py, PyAny>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.len {
            let item = self.tuple.get_item(self.index).ok()?;
            self.index += 1;
            Some(item)
        } else {
            None
        }
    }
}

impl<'py> IntoIterator for &Bound<'py, PyTuple> {
    type Item = Bound<'py, PyAny>;
    type IntoIter = BoundTupleIterator<'py>;

    fn into_iter(self) -> Self::IntoIter {
        BoundTupleIterator {
            len: self.len(),
            tuple: self.clone(),
            index: 0,
        }
    }
}

impl<'py> IntoIterator for Bound<'py, PyTuple> {
    type Item = Bound<'py, PyAny>;
    type IntoIter = BoundTupleIterator<'py>;

    fn into_iter(self) -> Self::IntoIter {
        let len = self.len();
        BoundTupleIterator {
            tuple: self,
            index: 0,
            len,
        }
    }
}
