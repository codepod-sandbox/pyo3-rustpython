use rustpython_vm::VirtualMachine;
use std::borrow::Cow;
use std::cell::Cell;

thread_local! {
    static CURRENT_VM: Cell<Option<*const VirtualMachine>> = const { Cell::new(None) };
}

#[derive(Copy, Clone)]
pub struct Python<'py> {
    pub(crate) vm: &'py VirtualMachine,
}

impl<'py> Python<'py> {
    #[doc(hidden)]
    pub fn from_vm(vm: &'py VirtualMachine) -> Self {
        CURRENT_VM.with(|cell| cell.set(Some(vm as *const VirtualMachine)));
        Python { vm }
    }

    pub fn vm(self) -> &'py VirtualMachine {
        self.vm
    }

    pub fn py(self) -> Self {
        self
    }

    pub fn with_gil<F, R>(f: F) -> R
    where
        F: for<'p> FnOnce(Python<'p>) -> R,
    {
        CURRENT_VM.with(|cell| {
            let ptr = cell
                .get()
                .expect("Python::with_gil called outside RustPython interpreter context");
            let vm = unsafe { &*ptr };
            f(Python { vm })
        })
    }

    pub fn attach<F, R>(f: F) -> R
    where
        F: for<'p> FnOnce(Python<'p>) -> R,
    {
        Self::with_gil(f)
    }

    /// Get a `Python` token assuming the GIL is already held.
    /// This is the 0-argument version matching real pyo3's API:
    /// `let py = unsafe { Python::assume_attached() };`
    ///
    /// # Safety: Must be called from a context where the VM is active.
    pub unsafe fn assume_attached() -> Python<'py> {
        CURRENT_VM.with(|cell| {
            let ptr = cell
                .get()
                .expect("Python::assume_attached called outside RustPython interpreter context");
            Python { vm: &*ptr }
        })
    }

    #[allow(non_snake_case)]
    pub fn None(self) -> crate::Py<crate::types::PyAny> {
        let none: rustpython_vm::PyObjectRef = self.vm.ctx.none();
        crate::Py::from_object(none)
    }

    pub fn detach<F, R>(self, f: F) -> R
    where
        F: FnOnce() -> R,
    {
        f()
    }

    #[allow(non_snake_case)]
    pub fn NotImplemented(self) -> crate::Py<crate::types::PyAny> {
        let not_impl: rustpython_vm::PyObjectRef = self.vm.ctx.not_implemented();
        crate::Py::from_object(not_impl)
    }

    /// Get the Python type object for `T`.
    pub fn get_type<T: crate::PyTypeObjectExt>(self) -> crate::Bound<'py, crate::types::PyType> {
        T::type_object(self)
    }

    /// Import a Python module by name.
    pub fn import(self, name: &str) -> crate::PyResult<crate::Bound<'py, crate::types::PyModule>> {
        let vm = self.vm;
        let name_interned = vm.ctx.intern_str(name);
        let module = vm.import(name_interned, 0)?;
        Ok(crate::Bound::from_object(self, module))
    }

    pub fn run(
        self,
        code: impl PyCodeInput,
        globals: Option<&crate::Bound<'py, crate::types::PyDict>>,
        locals: Option<&crate::Bound<'py, crate::types::PyDict>>,
    ) -> crate::PyResult<()> {
        let vm = self.vm;
        let source = code.to_source();
        let globals_dict = globals
            .map(|d| {
                d.obj.clone()
                    .try_into_value::<rustpython_vm::PyRef<rustpython_vm::builtins::PyDict>>(vm)
                    .expect("Borrowed<PyDict> must wrap a dict")
            })
            .unwrap_or_else(|| vm.ctx.new_dict());
        let locals_mapping = locals
            .map(|d| {
                d.obj.clone()
                    .try_into_value::<rustpython_vm::PyRef<rustpython_vm::builtins::PyDict>>(vm)
                    .expect("Bound<PyDict> must wrap a dict")
            })
            .map(rustpython_vm::function::ArgMapping::from_dict_exact);
        let scope = rustpython_vm::scope::Scope::with_builtins(locals_mapping, globals_dict, vm);
        crate::err::from_vm_result(vm.run_string(scope, &source, "<embedded>".to_owned()))
            .map(|_| ())
    }

    /// Check if a Python object is truthy.
    #[doc(hidden)]
    pub fn is_truthy(self, obj: &crate::Bound<'py, crate::types::PyAny>) -> crate::PyResult<bool> {
        let vm = self.vm;
        crate::err::from_vm_result(obj.obj.clone().try_to_bool(vm))
    }
}

pub trait PyCodeInput {
    fn to_source(&self) -> Cow<'_, str>;
}

impl PyCodeInput for str {
    fn to_source(&self) -> Cow<'_, str> {
        Cow::Borrowed(self)
    }
}

impl PyCodeInput for String {
    fn to_source(&self) -> Cow<'_, str> {
        Cow::Borrowed(self.as_str())
    }
}

impl PyCodeInput for std::ffi::CStr {
    fn to_source(&self) -> Cow<'_, str> {
        self.to_string_lossy()
    }
}

impl PyCodeInput for std::ffi::CString {
    fn to_source(&self) -> Cow<'_, str> {
        self.as_c_str().to_source()
    }
}

impl<T: PyCodeInput + ?Sized> PyCodeInput for &T {
    fn to_source(&self) -> Cow<'_, str> {
        (*self).to_source()
    }
}
