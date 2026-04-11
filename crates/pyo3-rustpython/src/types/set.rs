use rustpython_vm::{builtins::PySet as RpSet, convert::ToPyObject, PyObjectRef, PyPayload};

use crate::{
    err::{from_vm_result, PyResult},
    instance::Bound,
    python::Python,
};

/// Marker type for a Python `set` object.
pub struct PySet;

/// Marker type for a Python `frozenset` object.
pub struct PyFrozenSet;

impl<'py> Bound<'py, PySet> {
    /// Create a new empty Python set.
    pub fn new(py: Python<'py>) -> Bound<'py, PySet> {
        let vm = py.vm;
        let obj: PyObjectRef = RpSet::default().into_ref(&vm.ctx).into();
        Bound::from_object(py, obj)
    }

    /// Add a value to the set.
    pub fn add(&self, value: impl ToPyObject) -> PyResult<()> {
        let vm = self.py.vm;
        let val_obj = value.to_pyobject(vm);
        let set = self
            .obj
            .downcast_ref::<RpSet>()
            .expect("Bound<PySet> must wrap a set");
        from_vm_result(set.add(val_obj, vm))
    }

    /// Return `true` if the set contains the given value.
    pub fn contains(&self, value: impl ToPyObject) -> PyResult<bool> {
        let vm = self.py.vm;
        let val_obj = value.to_pyobject(vm);
        from_vm_result(
            vm.call_method(&self.obj, "__contains__", (val_obj,))
                .and_then(|r| r.try_to_bool(vm)),
        )
    }

    /// Return the number of items in the set.
    pub fn len(&self) -> usize {
        // Use the object protocol length
        let vm = self.py.vm;
        self.obj.length(vm).unwrap_or(0)
    }

    /// Return `true` if the set is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
