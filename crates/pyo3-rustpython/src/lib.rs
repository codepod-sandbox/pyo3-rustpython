pub mod bound_object;
pub mod buffer;
pub mod conversion;
pub mod err;
pub mod exceptions;
pub mod ffi;
pub mod instance;
pub mod pyclass;
pub mod python;
pub mod slots;
pub mod sync;
pub mod types;

pub mod interp {
    use rustpython::InterpreterBuilderExt;
    use rustpython_vm::Context;

    pub struct InterpreterBuilder {
        inner: rustpython_vm::InterpreterBuilder,
    }

    impl InterpreterBuilder {
        pub fn new() -> Self {
            InterpreterBuilder {
                inner: rustpython_vm::InterpreterBuilder::new().init_stdlib(),
            }
        }

        pub fn init_stdlib(self) -> Self {
            self
        }

        pub fn ctx(&self) -> &Context {
            &self.inner.ctx
        }

        pub fn add_native_module(
            mut self,
            module_def: &'static rustpython_vm::builtins::PyModuleDef,
        ) -> Self {
            self.inner = self.inner.add_native_module(module_def);
            self
        }

        pub fn build(self) -> rustpython_vm::Interpreter {
            self.inner.build()
        }
    }

    impl Default for InterpreterBuilder {
        fn default() -> Self {
            Self::new()
        }
    }

    impl std::ops::Deref for InterpreterBuilder {
        type Target = rustpython_vm::InterpreterBuilder;
        fn deref(&self) -> &Self::Target {
            &self.inner
        }
    }
}

pub use bound_object::BoundObject;
pub use conversion::{ArgIntoBool, FromPyObject, FromPyObjectOwned, IntoPy, IntoPyObject, ToPyObject};
pub use err::{PyErr, PyResult};
pub use instance::{Borrowed, Bound, Py, PyRef, PyRefMut};
pub use pyclass::CompareOp;
pub use pyo3_rustpython_derive::{pyclass, pyfunction, pymethods, pymodule, FromPyObject};
pub use python::Python;
pub use types::module::WrapPyFn;

pub trait PyClass: PyTypeInfo {}

pub trait PyTypeInfo {
    const NAME: &'static str;
    const MODULE: Option<&'static str>;

    fn type_object(py: Python<'_>) -> Bound<'_, types::PyType>
    where
        Self: PyTypeObjectExt + Sized,
    {
        <Self as PyTypeObjectExt>::type_object_bound(py)
    }
}

pub trait PyTypeObjectExt {
    fn type_object_raw(ctx: &rustpython_vm::Context) -> &'static rustpython_vm::Py<rustpython_vm::builtins::PyType>
    where
        Self: Sized;

    fn type_object_bound(py: Python<'_>) -> Bound<'_, types::PyType>
    where
        Self: Sized,
    {
        let obj: rustpython_vm::PyObjectRef = Self::type_object_raw(&py.vm.ctx).to_owned().into();
        Bound::from_object(py, obj)
    }
}

impl<T: rustpython_vm::PyPayload> PyTypeObjectExt for T
{
    fn type_object_raw(ctx: &rustpython_vm::Context) -> &'static rustpython_vm::Py<rustpython_vm::builtins::PyType> {
        T::class(ctx)
    }
}

macro_rules! impl_pytypeobjectext_for_builtin {
    ($ty:ident, $type_field:ident) => {
        impl PyTypeObjectExt for types::$ty {
            fn type_object_raw(ctx: &rustpython_vm::Context) -> &'static rustpython_vm::Py<rustpython_vm::builtins::PyType> {
                ctx.types.$type_field
            }
        }
    };
}

impl_pytypeobjectext_for_builtin!(PyString, str_type);
impl_pytypeobjectext_for_builtin!(PyBool, bool_type);
impl_pytypeobjectext_for_builtin!(PyInt, int_type);
impl_pytypeobjectext_for_builtin!(PyFloat, float_type);
impl_pytypeobjectext_for_builtin!(PyDict, dict_type);
impl_pytypeobjectext_for_builtin!(PyTuple, tuple_type);
impl_pytypeobjectext_for_builtin!(PySet, set_type);
impl_pytypeobjectext_for_builtin!(PyBytes, bytes_type);

pub trait Pyo3Accessors {
    fn __pyo3_register_accessors(
        ctx: &rustpython_vm::Context,
        class: &'static rustpython_vm::Py<rustpython_vm::builtins::PyType>,
    );
}

pub trait Pyo3BasePayload {
    type BasePayload;
}

#[doc(hidden)]
pub struct Pyo3ClassItems {
    pub methods: &'static [rustpython_vm::function::PyMethodDef],
    pub extend_class:
        fn(&rustpython_vm::Context, &'static rustpython_vm::Py<rustpython_vm::builtins::PyType>),
    pub extend_slots: fn(&mut rustpython_vm::types::PyTypeSlots),
}

#[doc(hidden)]
pub trait Pyo3ClassInventory: inventory::Collect {
    fn items(&'static self) -> &'static Pyo3ClassItems;
}

pub use inventory;

#[doc(hidden)]
pub struct SyncModuleDefPtr(pub *const rustpython_vm::builtins::PyModuleDef);
unsafe impl Sync for SyncModuleDefPtr {}
unsafe impl Send for SyncModuleDefPtr {}

pub use paste::paste;

#[doc(hidden)]
pub fn __classattr_to_pyobj<T: ClassAttrValue>(
    ctx: &rustpython_vm::Context,
    val: T,
) -> rustpython_vm::PyObjectRef {
    val.to_pyobj(ctx)
}

#[doc(hidden)]
pub trait ClassAttrValue {
    fn to_pyobj(self, ctx: &rustpython_vm::Context) -> rustpython_vm::PyObjectRef;
}

impl ClassAttrValue for &str {
    fn to_pyobj(self, ctx: &rustpython_vm::Context) -> rustpython_vm::PyObjectRef {
        ctx.new_str(self).into()
    }
}

impl ClassAttrValue for i8 {
    fn to_pyobj(self, ctx: &rustpython_vm::Context) -> rustpython_vm::PyObjectRef {
        ctx.new_int(self as i64).into()
    }
}
impl ClassAttrValue for i16 {
    fn to_pyobj(self, ctx: &rustpython_vm::Context) -> rustpython_vm::PyObjectRef {
        ctx.new_int(self as i64).into()
    }
}
impl ClassAttrValue for i32 {
    fn to_pyobj(self, ctx: &rustpython_vm::Context) -> rustpython_vm::PyObjectRef {
        ctx.new_int(self as i64).into()
    }
}
impl ClassAttrValue for i64 {
    fn to_pyobj(self, ctx: &rustpython_vm::Context) -> rustpython_vm::PyObjectRef {
        ctx.new_int(self).into()
    }
}
impl ClassAttrValue for isize {
    fn to_pyobj(self, ctx: &rustpython_vm::Context) -> rustpython_vm::PyObjectRef {
        ctx.new_int(self as i64).into()
    }
}
impl ClassAttrValue for u8 {
    fn to_pyobj(self, ctx: &rustpython_vm::Context) -> rustpython_vm::PyObjectRef {
        ctx.new_int(self as i64).into()
    }
}
impl ClassAttrValue for u16 {
    fn to_pyobj(self, ctx: &rustpython_vm::Context) -> rustpython_vm::PyObjectRef {
        ctx.new_int(self as i64).into()
    }
}
impl ClassAttrValue for u32 {
    fn to_pyobj(self, ctx: &rustpython_vm::Context) -> rustpython_vm::PyObjectRef {
        ctx.new_int(self as i64).into()
    }
}
impl ClassAttrValue for u64 {
    fn to_pyobj(self, ctx: &rustpython_vm::Context) -> rustpython_vm::PyObjectRef {
        ctx.new_int(self as i64).into()
    }
}
impl ClassAttrValue for usize {
    fn to_pyobj(self, ctx: &rustpython_vm::Context) -> rustpython_vm::PyObjectRef {
        ctx.new_int(self as i64).into()
    }
}
impl ClassAttrValue for f64 {
    fn to_pyobj(self, ctx: &rustpython_vm::Context) -> rustpython_vm::PyObjectRef {
        ctx.new_float(self).into()
    }
}
impl ClassAttrValue for bool {
    fn to_pyobj(self, ctx: &rustpython_vm::Context) -> rustpython_vm::PyObjectRef {
        ctx.new_bool(self).into()
    }
}

#[doc(hidden)]
pub fn __next_option_to_result<'py, T>(
    val: Option<T>,
    py: Python<'py>,
) -> rustpython_vm::PyResult<rustpython_vm::PyObjectRef>
where
    T: conversion::IntoPyObject<'py>,
    T::Error: Into<err::PyErr>,
{
    let vm = py.vm;
    match val {
        Some(v) => {
            let pyo3_err_mapper = |e: T::Error| err::into_vm_err(e.into());
            let bound = v.into_pyobject(py).map_err(pyo3_err_mapper)?;
            Ok(bound_object::BoundObject::into_any(bound).obj)
        }
        None => Err(vm.new_stop_iteration(None)),
    }
}

pub mod pyo3_built {
    #[macro_export]
    macro_rules! pyo3_built {
        ($py:ident, $name:ident) => {{}};
    }
    pub use pyo3_built;
}

pub mod prelude {
    pub use crate::bound_object::BoundObject;
    pub use crate::conversion::{ArgIntoBool, FromPyObject, IntoPy, IntoPyObject, ToPyObject};
    pub use crate::err::{PyErr, PyResult};
    pub use crate::instance::{Borrowed, Bound, Py, PyRef, PyRefMut};
    pub use crate::pyclass::CompareOp;
    pub use crate::python::Python;
    pub use crate::types::{
        PyAny, PyAnyMethods, PyBool, PyBytes, PyDict, PyFloat, PyInt, PyIterator, PyList, PyLong,
        PyCFunction, PyDateTime, PyFunction, PyMapping, PyModule, PyNone, PySet, PyString,
        PyTuple, PyTupleMethods, PyType,
        PyTypeMethods,
    };
    pub use crate::wrap_pyfunction;
    pub use crate::PyTypeInfo;
    pub use crate::PyTypeObjectExt;
    pub use crate::Pyo3Accessors;
    pub use pyo3_rustpython_derive::{
        pyclass, pyfunction, pymethods, pymodule, pyo3, FromPyObject,
    };
}

#[macro_export]
macro_rules! wrap_pyfunction {
    ($func:ident, $module:expr) => {
        $crate::paste! {
            unsafe {
                extern "Rust" {
                    fn [<__pyo3_wrap_symbol_ $func>](__py: $crate::Python<'_>) -> ::rustpython_vm::PyObjectRef;
                }
                let __obj = [<__pyo3_wrap_symbol_ $func>]($module.py());
                Ok::<_, $crate::PyErr>($crate::Bound::<$crate::types::PyAny>::from_object($module.py(), __obj))
            }
        }
    };
    ($func:ident) => {
        $crate::paste! {
            {
                fn __wrap<'py>(__py: $crate::Python<'py>) -> Result<$crate::Bound<'py, $crate::types::PyAny>, $crate::PyErr> {
                    unsafe {
                        extern "Rust" {
                            fn [<__pyo3_wrap_symbol_ $func>](__py: $crate::Python<'_>) -> ::rustpython_vm::PyObjectRef;
                        }
                        let __obj = [<__pyo3_wrap_symbol_ $func>](__py);
                        Ok($crate::Bound::<$crate::types::PyAny>::from_object(__py, __obj))
                    }
                }
                __wrap
            }
        }
    };
    ($mod:ident :: $func:ident, $module:expr) => {
        $crate::wrap_pyfunction!($func, $module)
    };
    ($a:ident :: $b:ident :: $func:ident, $module:expr) => {
        $crate::wrap_pyfunction!($func, $module)
    };
    (crate :: $mod:ident :: $func:ident, $module:expr) => {
        $crate::wrap_pyfunction!($func, $module)
    };
}

#[macro_export]
macro_rules! wrap_pymodule {
    ($module:ident) => {
        |__py| {
            let __module = $crate::types::PyModule::new(__py, stringify!($module)).unwrap();
            $module(&__module).unwrap();
            __module.into_any()
        }
    };
}

#[macro_export]
macro_rules! py_run {
    ($py:expr, *$dict:expr, $code:expr) => {{
        $py.run(
            &::std::ffi::CString::new($code).unwrap(),
            None,
            Some(&$dict),
        )
        .unwrap()
    }};
    ($py:expr, $($val:ident)+, $code:expr) => {{
        use $crate::types::IntoPyDict;
        let __dict = [$((stringify!($val), $crate::ToPyObject::to_object(&$val, $py)),)+].into_py_dict($py).unwrap();
        $crate::py_run!($py, *__dict, $code)
    }};
}
