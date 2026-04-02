use rustpython_vm::VirtualMachine;

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
        Python { vm }
    }

    /// Access the underlying `VirtualMachine`.
    pub fn vm(self) -> &'py VirtualMachine {
        self.vm
    }
}
