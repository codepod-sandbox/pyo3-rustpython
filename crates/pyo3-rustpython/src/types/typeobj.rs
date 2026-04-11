use rustpython_vm::builtins::PyType as RpType;
use rustpython_vm::AsObject;

use crate::{
    err::{from_vm_result, PyResult},
    ffi::PyTypeObject,
    instance::Bound,
    types::PyAny,
};

use crate::python::Python;

pub struct PyType;

impl<'py> Bound<'py, PyType> {
    pub fn name(&self) -> PyResult<String> {
        let pytype = self
            .obj
            .downcast_ref::<RpType>()
            .expect("Bound<PyType> must wrap a type");
        Ok(pytype.name().to_string())
    }

    pub fn call1(
        &self,
        args: impl crate::conversion::IntoPyArgs<'py>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let vm = self.py.vm;
        let positional = args.into_py_args(self.py)?;
        let func_args: rustpython_vm::function::FuncArgs = positional.into();
        let result = from_vm_result(self.obj.call_with_args(func_args, vm))?;
        Ok(Bound::from_object(self.py, result))
    }

    pub fn as_type_ptr(&self) -> *mut PyTypeObject {
        crate::ffi::pyobject_ref_as_ptr(&self.obj) as *mut PyTypeObject
    }

    pub fn is_subtype_of(&self, other: &Bound<'py, PyType>) -> bool {
        let vm = self.py.vm;
        from_vm_result(self.obj.real_is_subclass(&other.obj, vm)).unwrap_or(false)
    }

    pub unsafe fn from_borrowed_type_ptr(
        py: Python<'py>,
        ptr: *mut crate::ffi::PyTypeObject,
    ) -> Self {
        let obj = crate::ffi::ffi_object::ptr_to_pyobject_ref_borrowed(
            ptr as *mut crate::ffi::ffi_object::PyObject,
        );
        Bound::from_object(py, obj.clone())
    }
}

impl PyType {
    pub fn new<'py>(
        py: Python<'py>,
        name: &str,
        bases: Option<&Bound<'py, PyAny>>,
        dict: Option<&Bound<'py, crate::types::PyDict>>,
    ) -> PyResult<Bound<'py, PyType>> {
        let vm = py.vm;
        let name_obj = vm.ctx.new_str(name);
        let bases_obj = match bases {
            Some(b) => b.obj.clone(),
            None => vm.ctx.new_tuple(vec![]).into(),
        };
        let dict_obj = match dict {
            Some(d) => d.obj.clone(),
            None => vm.ctx.new_dict().into(),
        };
        let args = rustpython_vm::function::FuncArgs::new(
            vec![name_obj.into(), bases_obj, dict_obj],
            rustpython_vm::function::KwArgs::default(),
        );
        let type_type = vm.ctx.types.type_type.as_object();
        let type_obj = from_vm_result(type_type.call_with_args(args, vm))?;
        Ok(Bound::from_object(py, type_obj))
    }

    pub unsafe fn from_borrowed_type_ptr<'py>(
        py: Python<'py>,
        ptr: *mut crate::ffi::PyTypeObject,
    ) -> Bound<'py, PyType> {
        let obj = crate::ffi::ffi_object::ptr_to_pyobject_ref_borrowed(
            ptr as *mut crate::ffi::ffi_object::PyObject,
        );
        Bound::from_object(py, obj.clone())
    }
}

impl rustpython_vm::PyPayload for PyType {
    fn class(
        ctx: &rustpython_vm::Context,
    ) -> &'static rustpython_vm::Py<rustpython_vm::builtins::PyType> {
        ctx.types.type_type
    }
}

impl rustpython_vm::object::MaybeTraverse for PyType {
    fn try_traverse(&self, _traverse_fn: &mut rustpython_vm::object::TraverseFn<'_>) {}
}
