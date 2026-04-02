use std::marker::PhantomData;
use rustpython_vm::VirtualMachine;

#[derive(Copy, Clone)]
pub struct Python<'py>(PhantomData<&'py VirtualMachine>);

impl<'py> Python<'py> {
    #[doc(hidden)]
    pub fn from_vm(vm: &'py VirtualMachine) -> Self {
        unsafe { super::gil::set_current_vm(vm) };
        Python(PhantomData)
    }

    pub fn vm(self) -> &'py VirtualMachine {
        super::gil::with_current_vm(|vm| unsafe { &*(vm as *const VirtualMachine) })
    }

    pub fn with_gil<F, R>(f: F) -> R
    where F: for<'p> FnOnce(Python<'p>) -> R {
        super::gil::with_current_vm(|_vm| f(Python(PhantomData)))
    }

    pub fn allow_threads<T, F>(self, f: F) -> T
    where F: Ungil + FnOnce() -> T, T: Ungil {
        f()
    }

    pub fn None(self) -> rustpython_vm::PyObjectRef {
        self.vm().ctx.none()
    }
}

pub trait Ungil {}
impl<T: ?Sized> Ungil for T {}
