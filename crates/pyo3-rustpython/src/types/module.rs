use rustpython_vm::{
    builtins::{PyDict as RpDict, PyModule as RpModule, PyStrRef},
    convert::ToPyObject,
    Py, PyObjectRef,
};

use crate::{
    err::{from_vm_result, PyResult},
    instance::Bound,
    python::Python,
    types::{PyAny, PyString},
};

pub trait IntoAddWrapped<'py> {
    fn into_wrapped(self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>>;
}

impl<'py> IntoAddWrapped<'py> for &Bound<'py, PyAny> {
    fn into_wrapped(self, _py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        Ok(self.clone())
    }
}

impl<'py> IntoAddWrapped<'py> for Bound<'py, PyAny> {
    fn into_wrapped(self, _py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        Ok(self)
    }
}

impl<'py, T> IntoAddWrapped<'py> for PyResult<Bound<'py, T>> {
    fn into_wrapped(self, _py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        self.map(|b| b.into_any())
    }
}

impl<'py, F> IntoAddWrapped<'py> for F
where
    F: FnOnce(Python<'py>) -> PyResult<Bound<'py, PyAny>>,
{
    fn into_wrapped(self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        self(py)
    }
}

pub struct WrapPyFn;

impl IntoAddWrapped<'_> for WrapPyFn {
    fn into_wrapped(self, _py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
        unreachable!(
            "WrapPyFn should not be used directly; use wrap_pyfunction!(func, module) instead"
        )
    }
}

/// Marker type for a Python module object. Analogous to PyO3's `PyModule`.
pub struct PyModule;

impl PyModule {
    /// Create a new module object with `__name__` set to `name`.
    pub fn new<'py>(py: Python<'py>, name: &str) -> PyResult<Bound<'py, PyModule>> {
        let vm = py.vm;
        let dict = vm.ctx.new_dict();
        let module = vm.new_module(name, dict, None);
        let obj: PyObjectRef = module.into();
        Ok(Bound::from_object(py, obj))
    }

    /// Import a Python module by name. Equivalent to `import name`.
    /// Handles dotted names like `collections.abc` correctly by traversing submodules.
    pub fn import<'py>(py: Python<'py>, name: &str) -> PyResult<Bound<'py, PyModule>> {
        let vm = py.vm;
        // For dotted names, import the top-level and then traverse submodules.
        // `vm.import("collections.abc", 0)` returns `collections`, not `collections.abc`.
        let parts: Vec<&str> = name.split('.').collect();
        let top_interned = vm.ctx.intern_str(parts[0]);
        let mut module = from_vm_result(vm.import(top_interned, 0))?;
        // Traverse submodule attributes.
        for part in &parts[1..] {
            let attr_interned = vm.ctx.intern_str(*part);
            module = from_vm_result(module.get_attr(attr_interned, vm))?;
        }
        Ok(Bound::from_object(py, module))
    }
}

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

    /// Add a function or (sub)module to this module, using the object's `__name__`
    /// attribute as the name.
    ///
    /// Accepts either:
    /// - A `&Bound<'_, PyAny>` (for `wrap_pyfunction!(fn, module)` form)
    /// - A `PyResult<Bound<'_, PyAny>>` (for `wrap_pyfunction!(fn)` form)
    pub fn add_wrapped(&self, wrapper: impl IntoAddWrapped<'py>) -> PyResult<()> {
        let wrapper = wrapper.into_wrapped(self.py)?;
        let vm = self.py.vm;
        let name_obj = from_vm_result(wrapper.obj.get_attr("__name__", vm))?;
        let name_ref: PyStrRef = from_vm_result(name_obj.try_into_value(vm))?;
        from_vm_result(self.obj.set_attr(&name_ref, wrapper.obj.clone(), vm))
    }

    /// Add a submodule to this module.
    ///
    /// The submodule's `__name__` attribute is used as the attribute name.
    pub fn add_submodule(&self, module: &Bound<'py, PyModule>) -> PyResult<()> {
        let vm = self.py.vm;
        let name_obj = from_vm_result(module.obj.get_attr("__name__", vm))?;
        let name_ref: PyStrRef = from_vm_result(name_obj.try_into_value(vm))?;
        from_vm_result(self.obj.set_attr(&name_ref, module.obj.clone(), vm))
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

    /// Register a `#[pyclass]` type with this module.
    ///
    /// The class is created via `T::make_class` and attached as an attribute
    /// using its Python-side name. Property accessors generated by
    /// `#[pyo3(get)]` / `#[pyo3(set)]` are registered separately.
    pub fn add_class<T>(&self) -> crate::err::PyResult<()>
    where
        T: ::rustpython_vm::PyPayload
            + ::rustpython_vm::class::PyClassImpl
            + ::rustpython_vm::class::StaticType
            + crate::Pyo3Accessors,
    {
        let vm = self.py.vm;
        let name = <T as ::rustpython_vm::class::PyClassDef>::NAME;

        // Use PyPayload::class() instead of make_class() directly so that
        // extends= base initialization and fixup_dunder_slots run.
        let class_static = <T as ::rustpython_vm::PyPayload>::class(&vm.ctx);
        let class: ::rustpython_vm::PyRef<::rustpython_vm::builtins::PyType> =
            class_static.to_owned();

        // Register property accessors for #[pyo3(get)] / #[pyo3(set)] fields.
        T::__pyo3_register_accessors(&vm.ctx, class_static);

        let interned = vm.ctx.intern_str(name);
        let class_obj: PyObjectRef = class.into();
        from_vm_result(self.obj.set_attr(interned, class_obj, vm))
    }
}
