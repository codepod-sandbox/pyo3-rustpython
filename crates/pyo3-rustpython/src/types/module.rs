use rustpython_vm::{
    builtins::{PyModule as RpModule, PyStrRef},
    convert::ToPyObject,
    PyObjectRef, Py,
};

use crate::{
    err::{from_vm_result, PyResult},
    instance::Bound,
    python::Python,
    types::PyAny,
};

/// Marker type for a Python module object. Analogous to PyO3's `PyModule`.
pub struct PyModule;

impl<'py> Bound<'py, PyModule> {
    /// Construct from a raw `PyObjectRef` known to be a module.
    #[doc(hidden)]
    pub fn from_module_obj(py: Python<'py>, obj: PyObjectRef) -> Self {
        Bound::from_object(py, obj)
    }

    /// Construct from a `&Py<RpModule>` as received by a `ModuleExec` slot.
    #[doc(hidden)]
    pub fn from_exec_ref(py: Python<'py>, module: &Py<RpModule>) -> Self {
        // to_owned increments the refcount giving an owned PyRef<RpModule>,
        // then into() erases the type tag to PyObjectRef.
        let obj: PyObjectRef = module.to_owned().into();
        Bound::from_object(py, obj)
    }

    /// Add a Python callable (from `wrap_pyfunction!`) to this module.
    ///
    /// Uses the function's `__name__` attribute as the attribute name.
    pub fn add_function(&self, func: Bound<'py, PyAny>) -> PyResult<()> {
        let vm = self.py.vm;
        let name_obj = from_vm_result(func.obj.get_attr("__name__", vm))?;
        // Extract as PyStrRef so we have an AsPyStr-compatible value.
        let name_ref: PyStrRef = from_vm_result(name_obj.try_into_value(vm))?;
        from_vm_result(self.obj.set_attr(&name_ref, func.obj, vm))
    }

    /// Add an arbitrary named attribute to this module.
    ///
    /// The name is interned so it satisfies RustPython's `AsPyStr` bound.
    pub fn add(&self, name: &str, value: impl ToPyObject) -> PyResult<()> {
        let vm = self.py.vm;
        let py_val = value.to_pyobject(vm);
        let interned = vm.ctx.intern_str(name);
        from_vm_result(self.obj.set_attr(interned, py_val, vm))
    }

    // add_class<T> requires PyClassImpl + StaticType which are internal traits.
    // Implemented in generated #[pyclass] code via the companion module_def function.
}
