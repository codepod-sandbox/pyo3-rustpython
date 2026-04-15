use std::{collections::HashSet, sync::{Mutex, OnceLock}};

use rustpython_vm::AsObject;

use crate::{
    err::{from_vm_result, PyResult},
    instance::Bound,
    python::Python,
    types::PyList,
};

/// Marker type for a Python mapping object.
pub struct PyMapping;

fn registered_mappings() -> &'static Mutex<HashSet<usize>> {
    static REGISTERED: OnceLock<Mutex<HashSet<usize>>> = OnceLock::new();
    REGISTERED.get_or_init(|| Mutex::new(HashSet::new()))
}

pub(crate) fn is_registered_mapping_obj(obj: &rustpython_vm::PyObject) -> bool {
    registered_mappings()
        .lock()
        .map(|set| set.contains(&(obj.class().as_object() as *const _ as usize)))
        .unwrap_or(false)
}

impl<'py> Bound<'py, PyMapping> {
    /// Get the items as a list of (key, value) tuples.
    pub fn items(&self) -> PyResult<Bound<'py, PyList>> {
        let vm = self.py.vm;
        let result = from_vm_result(vm.call_method(&self.obj, "items", ()))?;
        // Convert to list
        let _list = from_vm_result(vm.call_method(&result, "__iter__", ()))?;

        Ok(Bound::from_object(self.py, result))
    }
}

impl PyMapping {
    /// Register a type as a Mapping (ABC registration).
    pub fn register<T: crate::PyTypeObjectExt>(_py: Python<'_>) -> PyResult<()> {
        let mut set = registered_mappings().lock().unwrap();
        let class = T::type_object_raw(&_py.vm.ctx);
        set.insert(class.as_object() as *const _ as usize);
        Ok(())
    }
}
