use std::cell::Cell;
use rustpython_vm::VirtualMachine;

thread_local! {
    static CURRENT_VM: Cell<Option<*const VirtualMachine>> = const { Cell::new(None) };
}

/// Represents access to the Python interpreter. Analogous to PyO3's `Python<'py>`.
///
/// In RustPython this is a thin wrapper around `&VirtualMachine`. The lifetime
/// `'py` ties borrows to the duration of the Python call frame.
#[derive(Copy, Clone)]
pub struct Python<'py> {
    pub(crate) vm: &'py VirtualMachine,
}

impl<'py> Python<'py> {
    /// Construct from a raw VM reference. Used in generated exec-slot code.
    #[doc(hidden)]
    pub fn from_vm(vm: &'py VirtualMachine) -> Self {
        // Stash in thread-local so with_gil can find it
        CURRENT_VM.with(|cell| cell.set(Some(vm as *const VirtualMachine)));
        Python { vm }
    }

    /// Access the underlying `VirtualMachine`.
    pub fn vm(self) -> &'py VirtualMachine {
        self.vm
    }

    /// Run a closure with access to the Python interpreter.
    ///
    /// Retrieves the VM from a thread-local set during interpreter entry.
    /// Panics if called outside a RustPython interpreter context.
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

    /// Alias for `with_gil`. pyo3 0.28 renamed `with_gil` to `attach`.
    pub fn attach<F, R>(f: F) -> R
    where
        F: for<'p> FnOnce(Python<'p>) -> R,
    {
        Self::with_gil(f)
    }

    /// Return a Python `None` value as a `Py<PyAny>`.
    #[allow(non_snake_case)]
    pub fn None(self) -> crate::Py<crate::types::PyAny> {
        let none: rustpython_vm::PyObjectRef = self.vm.ctx.none();
        crate::Py::from_object(none)
    }

    /// Return a Python `NotImplemented` value as a `Py<PyAny>`.
    #[allow(non_snake_case)]
    pub fn NotImplemented(self) -> crate::Py<crate::types::PyAny> {
        let not_impl: rustpython_vm::PyObjectRef = self.vm.ctx.not_implemented();
        crate::Py::from_object(not_impl)
    }
}
