use std::{collections::HashSet, sync::{Mutex, OnceLock}};

use rustpython_vm::AsObject;

use crate::{
    err::{from_vm_result, PyResult},
    instance::Bound,
    python::Python,
};

pub struct PySequence;

fn registered_sequences() -> &'static Mutex<HashSet<usize>> {
    static REGISTERED: OnceLock<Mutex<HashSet<usize>>> = OnceLock::new();
    REGISTERED.get_or_init(|| Mutex::new(HashSet::new()))
}

pub(crate) fn is_registered_sequence_obj(obj: &rustpython_vm::PyObject) -> bool {
    registered_sequences()
        .lock()
        .map(|set| set.contains(&(obj.class().as_object() as *const _ as usize)))
        .unwrap_or(false)
}

impl PySequence {
    pub fn register<T: crate::PyTypeObjectExt>(_py: Python<'_>) -> PyResult<()> {
        let mut set = registered_sequences().lock().unwrap();
        let class = T::type_object_raw(&_py.vm.ctx);
        set.insert(class.as_object() as *const _ as usize);
        Ok(())
    }
}

impl<'py> Bound<'py, PySequence> {
    pub fn len(&self) -> PyResult<usize> {
        let len_obj = self.call_method0("__len__")?;
        len_obj.extract()
    }
}
