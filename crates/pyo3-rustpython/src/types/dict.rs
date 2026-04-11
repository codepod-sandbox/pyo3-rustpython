use rustpython_vm::{builtins::PyDict as RpDict, PyObjectRef};

use crate::{
    conversion::ToPyObject as OurToPyObject,
    err::{from_vm_result, PyResult},
    instance::Bound,
    python::Python,
    types::{PyAny, PyList},
};

/// Marker type for a Python `dict` object.
pub struct PyDict;

impl PyDict {
    pub fn new<'py>(py: Python<'py>) -> Bound<'py, PyDict> {
        let vm = py.vm;
        let obj: PyObjectRef = vm.ctx.new_dict().into();
        Bound::from_object(py, obj)
    }
}

impl<'py> Bound<'py, PyDict> {
    pub fn new(py: Python<'py>) -> Bound<'py, PyDict> {
        PyDict::new(py)
    }

    pub fn new_bound(py: Python<'py>) -> Bound<'py, PyDict> {
        PyDict::new(py)
    }

    /// Get the value associated with `key`, or `None` if the key is not present.
    pub fn get_item(&self, key: impl OurToPyObject) -> PyResult<Option<Bound<'py, PyAny>>> {
        let vm = self.py.vm;
        let key_obj = key.to_object(self.py).obj;
        let dict = self
            .obj
            .downcast_ref::<RpDict>()
            .expect("Bound<PyDict> must wrap a dict");
        match dict.get_item_opt(&*key_obj, vm) {
            Ok(Some(val)) => Ok(Some(Bound::from_object(self.py, val))),
            Ok(None) => Ok(None),
            Err(e) => Err(crate::PyErr::from_vm_err(e)),
        }
    }

    /// Set `dict[key] = value`.
    pub fn set_item(&self, key: impl OurToPyObject, value: impl OurToPyObject) -> PyResult<()> {
        let vm = self.py.vm;
        let key_obj = key.to_object(self.py).obj;
        let val_obj = value.to_object(self.py).obj;
        from_vm_result(self.obj.set_item(&*key_obj, val_obj, vm))
    }

    /// Delete `dict[key]`.
    pub fn del_item(&self, key: impl OurToPyObject) -> PyResult<()> {
        let vm = self.py.vm;
        let key_obj = key.to_object(self.py).obj;
        from_vm_result(self.obj.del_item(&*key_obj, vm))
    }

    /// Return the number of items in the dict.
    pub fn len(&self) -> usize {
        let dict = self
            .obj
            .downcast_ref::<RpDict>()
            .expect("Bound<PyDict> must wrap a dict");
        dict.__len__()
    }

    /// Return `true` if the dict has no items.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Return a list of all keys.
    pub fn keys(&self) -> PyResult<Bound<'py, PyList>> {
        let vm = self.py.vm;
        let dict = self
            .obj
            .downcast_ref::<RpDict>()
            .expect("Bound<PyDict> must wrap a dict");
        let keys: Vec<PyObjectRef> = dict.keys_vec();
        let list_obj: PyObjectRef = vm.ctx.new_list(keys).into();
        Ok(Bound::from_object(self.py, list_obj))
    }

    /// Return a list of all values.
    pub fn values(&self) -> PyResult<Bound<'py, PyList>> {
        let vm = self.py.vm;
        let dict = self
            .obj
            .downcast_ref::<RpDict>()
            .expect("Bound<PyDict> must wrap a dict");
        let vals: Vec<PyObjectRef> = dict.values_vec();
        let list_obj: PyObjectRef = vm.ctx.new_list(vals).into();
        Ok(Bound::from_object(self.py, list_obj))
    }

    /// Return a list of `(key, value)` tuples.
    pub fn items(&self) -> PyResult<Bound<'py, PyList>> {
        let vm = self.py.vm;
        let dict = self
            .obj
            .downcast_ref::<RpDict>()
            .expect("Bound<PyDict> must wrap a dict");
        let items: Vec<PyObjectRef> = dict
            .items_vec()
            .into_iter()
            .map(|(k, v)| vm.ctx.new_tuple(vec![k, v]).into())
            .collect();
        let list_obj: PyObjectRef = vm.ctx.new_list(items).into();
        Ok(Bound::from_object(self.py, list_obj))
    }

    /// Return `true` if the dict contains `key`.
    pub fn contains(&self, key: impl OurToPyObject) -> PyResult<bool> {
        let vm = self.py.vm;
        let key_obj = key.to_object(self.py).obj;
        let dict = self
            .obj
            .downcast_ref::<RpDict>()
            .expect("Bound<PyDict> must wrap a dict");
        Ok(dict.contains_key(&*key_obj, vm))
    }

    pub fn iter(&self) -> BoundDictIterator<'py> {
        let dict = self
            .obj
            .downcast_ref::<RpDict>()
            .expect("Bound<PyDict> must wrap a dict");
        let items: Vec<(PyObjectRef, PyObjectRef)> = dict.into_iter().collect();
        BoundDictIterator {
            py: self.py,
            items,
            index: 0,
        }
    }
}

/// Iterator over `(key, value)` pairs of a `Bound<'py, PyDict>`.
pub struct BoundDictIterator<'py> {
    py: Python<'py>,
    items: Vec<(PyObjectRef, PyObjectRef)>,
    index: usize,
}

impl<'py> Iterator for BoundDictIterator<'py> {
    type Item = (Bound<'py, PyAny>, Bound<'py, PyAny>);

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.items.len() {
            let (k, v) = self.items[self.index].clone();
            self.index += 1;
            Some((
                Bound::from_object(self.py, k),
                Bound::from_object(self.py, v),
            ))
        } else {
            None
        }
    }
}

impl<'py> IntoIterator for &Bound<'py, PyDict> {
    type Item = (Bound<'py, PyAny>, Bound<'py, PyAny>);
    type IntoIter = BoundDictIterator<'py>;

    fn into_iter(self) -> Self::IntoIter {
        let dict = self
            .obj
            .downcast_ref::<RpDict>()
            .expect("Bound<PyDict> must wrap a dict");
        let items: Vec<(PyObjectRef, PyObjectRef)> = dict.into_iter().collect();
        BoundDictIterator {
            py: self.py,
            items,
            index: 0,
        }
    }
}
