use std::fmt;
use std::sync::{Arc, Mutex};

use rustpython_vm::{PyObjectRef, PyPayload};

use crate::{instance::Bound, python::Python, PyResult};

pub struct PyCapsule {
    destructor: Arc<Mutex<Option<Box<dyn FnOnce() + Send + 'static>>>>,
}

impl fmt::Debug for PyCapsule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PyCapsule")
    }
}

impl Drop for PyCapsule {
    fn drop(&mut self) {
        if let Some(destructor) = self.destructor.lock().unwrap().take() {
            destructor();
        }
    }
}

impl rustpython_vm::object::MaybeTraverse for PyCapsule {
    fn try_traverse(&self, _traverse_fn: &mut rustpython_vm::object::TraverseFn<'_>) {}
}

impl rustpython_vm::PyPayload for PyCapsule {
    fn class(
        ctx: &rustpython_vm::Context,
    ) -> &'static rustpython_vm::Py<rustpython_vm::builtins::PyType> {
        ctx.types.capsule_type
    }
}

impl PyCapsule {
    pub fn new_with_value_and_destructor<'py, T, F>(
        py: Python<'py>,
        _value: T,
        _name: &std::ffi::CStr,
        destructor: F,
    ) -> PyResult<Bound<'py, PyCapsule>>
    where
        F: FnOnce(Python<'_>, &T) + Send + 'static,
        T: Send + Sync + 'static,
    {
        let value = Arc::new(_value);
        let value_for_drop = value.clone();
        let payload = PyCapsule {
            destructor: Arc::new(Mutex::new(Some(Box::new(move || {
                Python::attach(|py2| destructor(py2, &value_for_drop));
            })))),
        };
        let obj: PyObjectRef = payload.into_ref(&py.vm.ctx).into();
        Ok(Bound::from_object(py, obj))
    }
}
