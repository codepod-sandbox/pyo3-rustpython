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
    /// Returns isize for pyo3 compatibility.
    pub fn hash(&self) -> PyResult<isize> {
        let vm = self.py.vm;
        from_vm_result(self.obj.hash(vm)).map(|h| h as isize)
    }

    // Note: eq, ne, lt, le, gt, ge are defined generically on Bound<'py, T>
    // in instance.rs.

    // -----------------------------------------------------------------------
    // isinstance / contains / getitem
    // -----------------------------------------------------------------------

    /// Check if this object is an instance of the given type.
    /// Accepts any `Bound<'py, T>` as the type argument (usually PyType or PyAny).
    pub fn is_instance<T>(&self, type_obj: &Bound<'py, T>) -> PyResult<bool> {
        let vm = self.py.vm;
        from_vm_result(self.obj.is_instance(&type_obj.obj, vm))
    }

    /// Check if this object is an instance of a specific Rust pyclass type.
    pub fn is_instance_of<T: rustpython_vm::PyPayload + rustpython_vm::class::StaticType>(&self) -> bool {
        self.obj.downcast_ref::<T>().is_some()
    }

    /// Check if this container contains the given value.
    /// Equivalent to Python's `value in self`.
    pub fn contains<V: Into<PyObjectRef>>(&self, value: V) -> PyResult<bool> {
        let vm = self.py.vm;
        let value_obj: PyObjectRef = value.into();
        from_vm_result(vm.call_method(&self.obj, "__contains__", (value_obj,)))
            .and_then(|result| from_vm_result(result.try_to_bool(vm)))
    }

    /// Get an item by index/key. Equivalent to Python's `self[key]`.
    pub fn get_item<K: Into<PyObjectRef>>(&self, key: K) -> PyResult<Bound<'py, PyAny>> {
        let vm = self.py.vm;
        let key_obj: PyObjectRef = key.into();
        let result = from_vm_result(self.obj.get_item(&*key_obj, vm))?;
        Ok(Bound::from_object(self.py, result))
    }

    /// Get an iterator over this object. Equivalent to Python's `iter(self)`.
    pub fn try_iter(&self) -> PyResult<Bound<'py, crate::types::PyIterator>> {
        let vm = self.py.vm;
        let iter_obj = from_vm_result(self.obj.get_iter(vm))?;
        let obj_ref: PyObjectRef = iter_obj.into();
        Ok(Bound::from_object(self.py, obj_ref))
    }

    /// Coerce the hash() return type to isize for pyo3 compatibility.
    pub fn hash_isize(&self) -> PyResult<isize> {
        self.hash().map(|h| h as isize)
    }
}
