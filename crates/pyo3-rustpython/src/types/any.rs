/// Marker type for an untyped Python object. Analogous to PyO3's `PyAny`.
///
/// `Bound<'py, PyAny>` is the most general Python object reference.
pub struct PyAny;

use rustpython_vm::{
    builtins::{PyTuple as RpTuple, PyDict as RpDict},
    function::FuncArgs,
    types::PyComparisonOp,
    AsObject, PyObjectRef,
};

use crate::{
    err::{from_vm_result, PyErr, PyResult},
    instance::Bound,
    types::{PyDict, PyString, PyTuple, PyType},
};

/// Universal object API on `Bound<'py, PyAny>`.
///
/// Note: `extract` is defined in `conversion.rs` to avoid circular imports.
impl<'py> Bound<'py, PyAny> {
    // -----------------------------------------------------------------------
    // Attribute access
    // -----------------------------------------------------------------------

    /// Get an attribute by name. Equivalent to Python's `getattr(obj, name)`.
    pub fn getattr(&self, name: &str) -> PyResult<Bound<'py, PyAny>> {
        let vm = self.py.vm;
        let name_obj = vm.ctx.new_str(name);
        let result = from_vm_result(self.obj.get_attr(&name_obj, vm))?;
        Ok(Bound::from_object(self.py, result))
    }

    /// Set an attribute by name. Equivalent to Python's `setattr(obj, name, value)`.
    pub fn setattr(&self, name: &str, value: impl Into<PyObjectRef>) -> PyResult<()> {
        let vm = self.py.vm;
        let name_obj = vm.ctx.new_str(name);
        from_vm_result(self.obj.set_attr(&name_obj, value, vm))
    }

    /// Delete an attribute by name. Equivalent to Python's `delattr(obj, name)`.
    pub fn delattr(&self, name: &str) -> PyResult<()> {
        let vm = self.py.vm;
        let name_obj = vm.ctx.new_str(name);
        from_vm_result(self.obj.del_attr(&name_obj, vm))
    }

    /// Check if the object has an attribute. Equivalent to Python's `hasattr(obj, name)`.
    ///
    /// Returns `false` if `getattr` would raise `AttributeError`, `true` otherwise.
    pub fn hasattr(&self, name: &str) -> PyResult<bool> {
        let vm = self.py.vm;
        let name_obj = vm.ctx.new_str(name);
        match vm.get_attribute_opt(self.obj.clone(), &name_obj) {
            Ok(Some(_)) => Ok(true),
            Ok(None) => Ok(false),
            Err(e) => Err(PyErr::from_vm_err(e)),
        }
    }

    // -----------------------------------------------------------------------
    // Calling
    // -----------------------------------------------------------------------

    /// Call the object with positional args and optional keyword args.
    pub fn call(
        &self,
        args: &Bound<'py, PyTuple>,
        kwargs: Option<&Bound<'py, PyDict>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let vm = self.py.vm;
        let positional: Vec<PyObjectRef> = {
            let tuple = args.obj.downcast_ref::<RpTuple>()
                .expect("Bound<PyTuple> must wrap a tuple");
            tuple.as_slice().to_vec()
        };
        let func_args = match kwargs {
            Some(d) => {
                let dict = d.obj.downcast_ref::<RpDict>()
                    .expect("Bound<PyDict> must wrap a dict");
                let kw_pairs: Vec<(String, PyObjectRef)> = {
                    let mut pairs = Vec::new();
                    for (k, v) in dict {
                        let key_str: String = from_vm_result(
                            rustpython_vm::convert::TryFromObject::try_from_object(vm, k)
                        )?;
                        pairs.push((key_str, v));
                    }
                    pairs
                };
                let kw: rustpython_vm::function::KwArgs = kw_pairs.into_iter().collect();
                let mut fa: FuncArgs = positional.into();
                fa.kwargs = kw.into_iter().collect();
                fa
            }
            None => positional.into(),
        };
        let result = from_vm_result(self.obj.call_with_args(func_args, vm))?;
        Ok(Bound::from_object(self.py, result))
    }

    /// Call the object with no arguments.
    pub fn call0(&self) -> PyResult<Bound<'py, PyAny>> {
        let vm = self.py.vm;
        let result = from_vm_result(self.obj.call((), vm))?;
        Ok(Bound::from_object(self.py, result))
    }

    /// Call the object with positional arguments only.
    pub fn call1(&self, args: &Bound<'py, PyTuple>) -> PyResult<Bound<'py, PyAny>> {
        self.call(args, None)
    }

    /// Call a method on the object with positional args and optional keyword args.
    pub fn call_method(
        &self,
        name: &str,
        args: &Bound<'py, PyTuple>,
        kwargs: Option<&Bound<'py, PyDict>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let method = self.getattr(name)?;
        method.call(args, kwargs)
    }

    /// Call a method on the object with no arguments.
    pub fn call_method0(&self, name: &str) -> PyResult<Bound<'py, PyAny>> {
        let vm = self.py.vm;
        let result = from_vm_result(vm.call_method(&self.obj, name, ()))?;
        Ok(Bound::from_object(self.py, result))
    }

    /// Call a method on the object with positional arguments only.
    pub fn call_method1(&self, name: &str, args: &Bound<'py, PyTuple>) -> PyResult<Bound<'py, PyAny>> {
        self.call_method(name, args, None)
    }

    // -----------------------------------------------------------------------
    // Type operations
    // -----------------------------------------------------------------------

    // Note: `extract` is defined in `conversion.rs`.

    /// Check if this object is an instance of a specific Python type.
    pub fn is_instance_of_type(&self, type_obj: &Bound<'py, PyType>) -> PyResult<bool> {
        let vm = self.py.vm;
        let type_ref = type_obj.obj.clone();
        from_vm_result(self.obj.is_instance(&type_ref, vm))
    }

    /// Get the Python type of this object.
    pub fn get_type(&self) -> Bound<'py, PyType> {
        let type_obj: PyObjectRef = self.obj.class().to_owned().into();
        Bound::from_object(self.py, type_obj)
    }

    /// Check if this object is Python `None`.
    pub fn is_none(&self) -> bool {
        let vm = self.py.vm;
        vm.is_none(&self.obj)
    }

    /// Check if this object is "truthy" in Python's boolean context.
    pub fn is_truthy(&self) -> PyResult<bool> {
        let vm = self.py.vm;
        from_vm_result(self.obj.clone().try_to_bool(vm))
    }

    /// Identity check: returns `true` if `self` and `other` are the same object.
    pub fn is(&self, other: &Bound<'py, PyAny>) -> bool {
        self.obj.is(&other.obj)
    }

    // -----------------------------------------------------------------------
    // Representation
    // -----------------------------------------------------------------------

    /// Get the `repr()` of this object.
    pub fn repr(&self) -> PyResult<Bound<'py, PyString>> {
        let vm = self.py.vm;
        let repr_str = from_vm_result(self.obj.repr(vm))?;
        let obj: PyObjectRef = repr_str.into();
        Ok(Bound::from_object(self.py, obj))
    }

    /// Get the `str()` of this object.
    pub fn str_(&self) -> PyResult<Bound<'py, PyString>> {
        let vm = self.py.vm;
        let str_val = from_vm_result(self.obj.str(vm))?;
        let obj: PyObjectRef = str_val.into();
        Ok(Bound::from_object(self.py, obj))
    }

    /// Get the length of this object. Equivalent to Python's `len(obj)`.
    pub fn len(&self) -> PyResult<usize> {
        let vm = self.py.vm;
        from_vm_result(self.obj.length(vm))
    }

    /// Get the hash of this object. Equivalent to Python's `hash(obj)`.
    pub fn hash(&self) -> PyResult<i64> {
        let vm = self.py.vm;
        from_vm_result(self.obj.hash(vm))
    }

    // -----------------------------------------------------------------------
    // Comparison
    // -----------------------------------------------------------------------

    /// Python `==` comparison.
    pub fn eq(&self, other: &Bound<'py, PyAny>) -> PyResult<bool> {
        let vm = self.py.vm;
        from_vm_result(self.obj.rich_compare_bool(&other.obj, PyComparisonOp::Eq, vm))
    }

    /// Python `!=` comparison.
    pub fn ne(&self, other: &Bound<'py, PyAny>) -> PyResult<bool> {
        let vm = self.py.vm;
        from_vm_result(self.obj.rich_compare_bool(&other.obj, PyComparisonOp::Ne, vm))
    }

    /// Python `<` comparison.
    pub fn lt(&self, other: &Bound<'py, PyAny>) -> PyResult<bool> {
        let vm = self.py.vm;
        from_vm_result(self.obj.rich_compare_bool(&other.obj, PyComparisonOp::Lt, vm))
    }

    /// Python `<=` comparison.
    pub fn le(&self, other: &Bound<'py, PyAny>) -> PyResult<bool> {
        let vm = self.py.vm;
        from_vm_result(self.obj.rich_compare_bool(&other.obj, PyComparisonOp::Le, vm))
    }

    /// Python `>` comparison.
    pub fn gt(&self, other: &Bound<'py, PyAny>) -> PyResult<bool> {
        let vm = self.py.vm;
        from_vm_result(self.obj.rich_compare_bool(&other.obj, PyComparisonOp::Gt, vm))
    }

    /// Python `>=` comparison.
    pub fn ge(&self, other: &Bound<'py, PyAny>) -> PyResult<bool> {
        let vm = self.py.vm;
        from_vm_result(self.obj.rich_compare_bool(&other.obj, PyComparisonOp::Ge, vm))
    }
}
