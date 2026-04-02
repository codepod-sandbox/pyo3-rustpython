use rustpython_vm::{
    builtins::PyDict as RpDict,
    convert::ToPyObject,
    PyObjectRef,
};

use crate::{
    err::{from_vm_result, PyResult},
    instance::Bound,
    python::Python,
    types::{PyAny, PyList},
};

/// Marker type for a Python `dict` object.
pub struct PyDict;

impl<'py> Bound<'py, PyDict> {
    /// Create a new empty Python dict.
    pub fn new(py: Python<'py>) -> Bound<'py, PyDict> {
        let vm = py.vm;
        let obj: PyObjectRef = vm.ctx.new_dict().into();
        Bound::from_object(py, obj)
    }

    /// Get the value associated with `key`, or `None` if the key is not present.
    pub fn get_item(&self, key: impl ToPyObject) -> PyResult<Option<Bound<'py, PyAny>>> {
        let vm = self.py.vm;
        let key_obj = key.to_pyobject(vm);
        // Use the dict's own get method to distinguish missing keys from errors.
        let dict = self.obj.downcast_ref::<RpDict>().expect("Bound<PyDict> must wrap a dict");
        match dict.get_item_opt(&*key_obj, vm) {
            Ok(Some(val)) => Ok(Some(Bound::from_object(self.py, val))),
            Ok(None) => Ok(None),
            Err(e) => Err(crate::PyErr::from_vm_err(e)),
        }
    }

    /// Set `dict[key] = value`.
    pub fn set_item(&self, key: impl ToPyObject, value: impl ToPyObject) -> PyResult<()> {
        let vm = self.py.vm;
        let key_obj = key.to_pyobject(vm);
        let val_obj = value.to_pyobject(vm);
        from_vm_result(self.obj.set_item(&*key_obj, val_obj, vm))
    }

    /// Delete `dict[key]`.
    pub fn del_item(&self, key: impl ToPyObject) -> PyResult<()> {
        let vm = self.py.vm;
        let key_obj = key.to_pyobject(vm);
        from_vm_result(self.obj.del_item(&*key_obj, vm))
    }

    /// Return the number of items in the dict.
    pub fn len(&self) -> usize {
        let dict = self.obj.downcast_ref::<RpDict>().expect("Bound<PyDict> must wrap a dict");
        dict.__len__()
    }

    /// Return `true` if the dict has no items.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Return a list of all keys.
    pub fn keys(&self) -> PyResult<Bound<'py, PyList>> {
        let vm = self.py.vm;
        let dict = self.obj.downcast_ref::<RpDict>().expect("Bound<PyDict> must wrap a dict");
        let keys: Vec<PyObjectRef> = dict.keys_vec();
        let list_obj: PyObjectRef = vm.ctx.new_list(keys).into();
        Ok(Bound::from_object(self.py, list_obj))
    }

    /// Return a list of all values.
    pub fn values(&self) -> PyResult<Bound<'py, PyList>> {
        let vm = self.py.vm;
        let dict = self.obj.downcast_ref::<RpDict>().expect("Bound<PyDict> must wrap a dict");
        let vals: Vec<PyObjectRef> = dict.values_vec();
        let list_obj: PyObjectRef = vm.ctx.new_list(vals).into();
        Ok(Bound::from_object(self.py, list_obj))
    }

    /// Return a list of `(key, value)` tuples.
    pub fn items(&self) -> PyResult<Bound<'py, PyList>> {
        let vm = self.py.vm;
        let dict = self.obj.downcast_ref::<RpDict>().expect("Bound<PyDict> must wrap a dict");
        let items: Vec<PyObjectRef> = dict
            .items_vec()
            .into_iter()
            .map(|(k, v)| vm.ctx.new_tuple(vec![k, v]).into())
            .collect();
        let list_obj: PyObjectRef = vm.ctx.new_list(items).into();
        Ok(Bound::from_object(self.py, list_obj))
    }

    /// Return `true` if the dict contains `key`.
    pub fn contains(&self, key: impl ToPyObject) -> PyResult<bool> {
        let vm = self.py.vm;
        let key_obj = key.to_pyobject(vm);
        let dict = self.obj.downcast_ref::<RpDict>().expect("Bound<PyDict> must wrap a dict");
        Ok(dict.contains_key(&*key_obj, vm))
    }
}
