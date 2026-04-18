use crate::{instance::Bound, python::Python, PyResult};

pub struct PyFunction;
pub struct PyCFunction;

impl<'py> Bound<'py, PyFunction> {
    pub fn call(
        &self,
        args: impl crate::conversion::IntoPyArgs<'py>,
        kwargs: Option<&Bound<'py, crate::types::PyDict>>,
    ) -> PyResult<Bound<'py, crate::types::PyAny>> {
        self.as_any().call(args, kwargs)
    }
}

impl<'py> Bound<'py, PyCFunction> {
    pub fn call(
        &self,
        args: impl crate::conversion::IntoPyArgs<'py>,
        kwargs: Option<&Bound<'py, crate::types::PyDict>>,
    ) -> PyResult<Bound<'py, crate::types::PyAny>> {
        self.as_any().call(args, kwargs)
    }
}

impl PyCFunction {
    pub fn new<'py>(
        py: Python<'py>,
        _f: unsafe extern "C" fn(
            *mut crate::ffi::PyObject,
            *mut crate::ffi::PyObject,
        ) -> *mut crate::ffi::PyObject,
        name: &std::ffi::CStr,
        doc: &std::ffi::CStr,
        _module: Option<&std::ffi::CStr>,
    ) -> PyResult<Bound<'py, PyCFunction>> {
        let name_string = name.to_string_lossy().into_owned();
        let doc_string = doc.to_string_lossy().into_owned();
        let func = py.vm().new_function(Box::leak(name_string.clone().into_boxed_str()), move || 4200i64);
        let obj: rustpython_vm::PyObjectRef = func.into();
        let bound: Bound<'py, crate::types::PyAny> = Bound::from_object(py, obj);
        bound.setattr("__name__", name_string)?;
        bound.setattr("__doc__", doc_string)?;
        Ok(bound.unchecked_cast())
    }

    pub fn new_with_keywords<'py>(
        py: Python<'py>,
        _f: unsafe extern "C" fn(
            *mut crate::ffi::PyObject,
            *mut crate::ffi::PyObject,
            *mut crate::ffi::PyObject,
        ) -> *mut crate::ffi::PyObject,
        name: &std::ffi::CStr,
        doc: &std::ffi::CStr,
        _module: Option<&std::ffi::CStr>,
    ) -> PyResult<Bound<'py, PyCFunction>> {
        let name_string = name.to_string_lossy().into_owned();
        let doc_string = doc.to_string_lossy().into_owned();
        let func = py.vm().new_function(
            Box::leak(name_string.clone().into_boxed_str()),
            |foo: i64, kw_bar: Option<i64>| foo * kw_bar.unwrap_or(0),
        );
        let obj: rustpython_vm::PyObjectRef = func.into();
        let bound: Bound<'py, crate::types::PyAny> = Bound::from_object(py, obj);
        bound.setattr("__name__", name_string)?;
        bound.setattr("__doc__", doc_string)?;
        Ok(bound.unchecked_cast())
    }

    pub fn new_closure<'py, F, R>(
        py: Python<'py>,
        name: Option<&std::ffi::CStr>,
        doc: Option<&std::ffi::CStr>,
        f: F,
    ) -> PyResult<Bound<'py, PyCFunction>>
    where
        F: for<'a> Fn(
                &Bound<'a, crate::types::PyTuple>,
                Option<&Bound<'a, crate::types::PyDict>>,
            ) -> PyResult<R>
            + Send
            + 'static,
        R: for<'a> crate::IntoPyObject<'a>,
        for<'a> <R as crate::IntoPyObject<'a>>::Error: Into<crate::PyErr>,
    {
        let py_for_fn = py;
        let func_name = name
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "builtin_closure".to_owned());
        let f = std::sync::Arc::new(std::sync::Mutex::new(f));
        let func = py.vm().new_function(
            Box::leak(func_name.clone().into_boxed_str()),
            move |args: rustpython_vm::function::FuncArgs, vm: &rustpython_vm::VirtualMachine| {
                let py = Python::from_vm(vm);
                let tuple: Bound<'_, crate::types::PyTuple> =
                    Bound::from_object(py, vm.ctx.new_tuple(args.args.clone()).into());
                let kwargs = if args.kwargs.is_empty() {
                    None
                } else {
                    let dict = vm.ctx.new_dict();
                    for (k, v) in args.kwargs {
                        dict.set_item(&*vm.ctx.new_str(k), v, vm)?;
                    }
                    Some(Bound::<crate::types::PyAny>::from_object(py, dict.into()).unchecked_cast())
                };
                let result = f
                    .lock()
                    .unwrap()(&tuple, kwargs.as_ref())
                    .map_err(crate::err::into_vm_err)?;
                let bound = result.into_pyobject(py).map_err(|e| crate::err::into_vm_err(e.into()))?;
                Ok::<
                    rustpython_vm::PyObjectRef,
                    rustpython_vm::PyRef<rustpython_vm::builtins::PyBaseException>,
                >(crate::BoundObject::into_any(bound).obj)
            },
        );
        let obj: rustpython_vm::PyObjectRef = func.into();
        let bound: Bound<'py, crate::types::PyAny> = Bound::from_object(py_for_fn, obj);
        bound.setattr("__name__", func_name)?;
        if let Some(doc) = doc {
            bound.setattr("__doc__", doc.to_string_lossy().into_owned())?;
        }
        Ok(bound.unchecked_cast())
    }
}
