/// Marker type for an untyped Python object. Analogous to PyO3's `PyAny`.
///
/// `Bound<'py, PyAny>` is the most general Python object reference.
pub struct PyAny;

use rustpython_vm::{
    builtins::{PyDict as RpDict, PyTuple as RpTuple},
    function::FuncArgs,
    AsObject, PyObjectRef,
};

use crate::{
    err::{from_vm_result, PyErr, PyResult},
    instance::Bound,
    types::{PyDict, PyString, PyTuple, PyType},
};

/// Trait for types that can be converted to a `PyObjectRef`.
/// This is similar to `Into<PyObjectRef>` but we own the trait, allowing us
/// to impl it for foreign types like integers and tuples.
pub trait IntoPyObjectRef {
    fn into_pyobject_ref(self, vm: &rustpython_vm::VirtualMachine) -> PyObjectRef;
}

// PyObjectRef itself
impl IntoPyObjectRef for PyObjectRef {
    fn into_pyobject_ref(self, _vm: &rustpython_vm::VirtualMachine) -> PyObjectRef {
        self
    }
}

// Py<T> -> PyObjectRef
impl<T> IntoPyObjectRef for crate::instance::Py<T> {
    fn into_pyobject_ref(self, _vm: &rustpython_vm::VirtualMachine) -> PyObjectRef {
        self.obj
    }
}

// &Py<T> -> PyObjectRef (clone)
impl<T> IntoPyObjectRef for &crate::instance::Py<T> {
    fn into_pyobject_ref(self, _vm: &rustpython_vm::VirtualMachine) -> PyObjectRef {
        self.obj.clone()
    }
}

// Bound<'_, T> -> PyObjectRef
impl<T> IntoPyObjectRef for Bound<'_, T> {
    fn into_pyobject_ref(self, _vm: &rustpython_vm::VirtualMachine) -> PyObjectRef {
        self.obj
    }
}

// &Bound<'_, T> -> PyObjectRef
impl<T> IntoPyObjectRef for &Bound<'_, T> {
    fn into_pyobject_ref(self, _vm: &rustpython_vm::VirtualMachine) -> PyObjectRef {
        self.obj.clone()
    }
}

// Integer types
impl IntoPyObjectRef for i32 {
    fn into_pyobject_ref(self, vm: &rustpython_vm::VirtualMachine) -> PyObjectRef {
        vm.ctx.new_int(self).into()
    }
}

impl IntoPyObjectRef for i64 {
    fn into_pyobject_ref(self, vm: &rustpython_vm::VirtualMachine) -> PyObjectRef {
        vm.ctx.new_int(self).into()
    }
}

impl IntoPyObjectRef for usize {
    fn into_pyobject_ref(self, vm: &rustpython_vm::VirtualMachine) -> PyObjectRef {
        vm.ctx.new_int(self).into()
    }
}

// Tuple of two PyObjectRef-convertible items -> Python tuple
impl<A: Into<PyObjectRef>, B: Into<PyObjectRef>> IntoPyObjectRef for (A, B) {
    fn into_pyobject_ref(self, vm: &rustpython_vm::VirtualMachine) -> PyObjectRef {
        let a: PyObjectRef = self.0.into();
        let b: PyObjectRef = self.1.into();
        vm.ctx.new_tuple(vec![a, b]).into()
    }
}

/// Universal object API on `Bound<'py, PyAny>`.
///
/// Note: `extract` is defined in `conversion.rs` to avoid circular imports.
impl<'py> Bound<'py, PyAny> {
    // -----------------------------------------------------------------------
    // Attribute access
    // -----------------------------------------------------------------------

    /// Set an attribute by name. Equivalent to Python's `setattr(obj, name, value)`.
    pub fn setattr(&self, name: &str, value: impl crate::conversion::ToPyObject) -> PyResult<()> {
        let vm = self.py.vm;
        let name_obj = vm.ctx.new_str(name);
        let val_obj = value.to_object(self.py).obj;
        from_vm_result(self.obj.set_attr(&name_obj, val_obj, vm))
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
    // Callable / Iterator
    // -----------------------------------------------------------------------

    /// Check if this object is callable. Equivalent to Python's `callable(obj)`.
    pub fn is_callable(&self) -> bool {
        self.obj.is_callable()
    }

    /// Return an iterator over this object. Equivalent to Python's `iter(obj)`.
    pub fn iter(&self) -> PyResult<Bound<'py, crate::types::PyIterator>> {
        self.try_iter()
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
            let tuple = args
                .obj
                .downcast_ref::<RpTuple>()
                .expect("Bound<PyTuple> must wrap a tuple");
            tuple.as_slice().to_vec()
        };
        let func_args = match kwargs {
            Some(d) => {
                let dict = d
                    .obj
                    .downcast_ref::<RpDict>()
                    .expect("Bound<PyDict> must wrap a dict");
                let kw_pairs: Vec<(String, PyObjectRef)> = {
                    let mut pairs = Vec::new();
                    for (k, v) in dict {
                        let key_str: String = from_vm_result(
                            rustpython_vm::convert::TryFromObject::try_from_object(vm, k),
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
    ///
    /// Accepts either a `&Bound<'py, PyTuple>` or any tuple implementing
    /// `IntoPyArgs` (e.g. `(arg,)`, `(arg1, arg2)`, etc.).
    pub fn call1(
        &self,
        args: impl crate::conversion::IntoPyArgs<'py>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let vm = self.py.vm;
        let positional = args.into_py_args(self.py)?;
        let func_args: FuncArgs = positional.into();
        let result = from_vm_result(self.obj.call_with_args(func_args, vm))?;
        Ok(Bound::from_object(self.py, result))
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
    pub fn is_instance_of<T: rustpython_vm::PyPayload + rustpython_vm::class::StaticType>(
        &self,
    ) -> bool {
        self.obj.downcast_ref::<T>().is_some()
    }

    /// Check if this container contains the given value.
    /// Equivalent to Python's `value in self`.
    pub fn contains<V: IntoPyObjectRef>(&self, value: V) -> PyResult<bool> {
        let vm = self.py.vm;
        let value_obj: PyObjectRef = value.into_pyobject_ref(vm);
        from_vm_result(vm.call_method(&self.obj, "__contains__", (value_obj,)))
            .and_then(|result| from_vm_result(result.try_to_bool(vm)))
    }

    /// Get an item by index/key. Equivalent to Python's `self[key]`.
    pub fn get_item<K: IntoPyObjectRef>(&self, key: K) -> PyResult<Bound<'py, PyAny>> {
        let vm = self.py.vm;
        let key_obj: PyObjectRef = key.into_pyobject_ref(vm);
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

    /// Get the Python string representation (`str()`). Returns `PyResult` so
    /// callers can handle conversion failures.
    ///
    /// Named `try_to_string` to avoid shadowing `ToString::to_string()` from
    /// the `Display` blanket impl.
    pub fn try_to_string(&self) -> PyResult<String> {
        let vm = self.py.vm;
        let str_val = from_vm_result(self.obj.str(vm))?;
        Ok(str_val.as_str().to_string())
    }
}

impl std::fmt::Display for Bound<'_, PyAny> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.try_to_string() {
            Ok(s) => write!(f, "{}", s),
            Err(_) => write!(f, "<unrepresentable>"),
        }
    }
}
