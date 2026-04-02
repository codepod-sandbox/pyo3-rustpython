use std::cell::Cell;
use rustpython_vm::VirtualMachine;

thread_local! {
    static CURRENT_VM: Cell<Option<*const VirtualMachine>> = const { Cell::new(None) };
}

pub unsafe fn set_current_vm(vm: &VirtualMachine) {
    CURRENT_VM.with(|cell| cell.set(Some(vm as *const VirtualMachine)));
}

pub fn clear_current_vm() {
    CURRENT_VM.with(|cell| cell.set(None));
}

pub fn with_current_vm<F, R>(f: F) -> R
where
    F: FnOnce(&VirtualMachine) -> R,
{
    CURRENT_VM.with(|cell| {
        let ptr = cell.get().expect("Python::with_gil called outside RustPython interpreter context");
        let vm = unsafe { &*ptr };
        f(vm)
    })
}

/// RAII guard that sets the current VM on creation and clears it on drop.
pub struct VmGuard { _private: () }

impl VmGuard {
    pub unsafe fn enter(vm: &VirtualMachine) -> Self {
        unsafe { set_current_vm(vm) };
        VmGuard { _private: () }
    }
}

impl Drop for VmGuard {
    fn drop(&mut self) { clear_current_vm(); }
}
