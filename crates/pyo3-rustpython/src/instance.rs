use std::marker::PhantomData;

use rustpython_vm::PyObjectRef;

use crate::python::Python;

/// An owned Python object reference with a type tag. Analogous to PyO3's `Py<T>`.
///
/// The type parameter `T` is a marker only; the underlying object is a
/// `PyObjectRef` (ref-counted `PyObject`).
pub struct Py<T> {
    pub(crate) obj: PyObjectRef,
    _marker: PhantomData<T>,
}

impl<T> Py<T> {
    #[doc(hidden)]
    pub fn from_object(obj: PyObjectRef) -> Self {
        Py {
            obj,
            _marker: PhantomData,
        }
    }

    pub fn into_object(self) -> PyObjectRef {
        self.obj
    }

    /// Create a `Bound<'py, T>` from this `Py<T>`.
    pub fn into_bound<'py>(self, py: Python<'py>) -> Bound<'py, T> {
        Bound {
            py,
            obj: self.obj,
            _marker: PhantomData,
        }
    }

    /// Borrow as a `&Bound<'py, T>` without consuming self.
    ///
    /// In real pyo3, `bind(py)` returns `&Bound<'py, T>` by borrowing from
    /// the Py object's internal storage.  Since our `Py<T>` stores a
    /// `PyObjectRef` (not a `Bound`), we create a new `Bound` and leak it
    /// so the reference outlives the call.  This is safe because the data
    /// is ref-counted and lives as long as the GIL is held.
    pub fn bind<'py>(&self, py: Python<'py>) -> &'py Bound<'py, T> {
        let bound = Bound {
            py,
            obj: self.obj.clone(),
            _marker: PhantomData,
        };
        Box::leak(Box::new(bound))
    }

    /// Create a `Borrowed<'a, 'py, T>` reference.
    pub fn bind_borrowed<'a, 'py>(&'a self, py: Python<'py>) -> Borrowed<'a, 'py, T> {
        Borrowed {
            py,
            obj: self.obj.clone(),
            _marker: PhantomData,
        }
    }

    /// Clone the underlying object reference. In RustPython this is just a
    /// refcount increment.
    pub fn clone_ref(&self, _py: Python<'_>) -> Self {
        Py {
            obj: self.obj.clone(),
            _marker: PhantomData,
        }
    }

    pub fn into_any(self) -> Py<crate::types::PyAny> {
        Py::from_object(self.obj)
    }

    /// No-op: returns self. In real pyo3, `Py::unbind()` returns `Py<T>`.
    pub fn unbind(self) -> Self {
        self
    }

    pub fn borrow<'py>(&self, py: Python<'py>) -> Bound<'py, T> {
        Bound::from_object(py, self.obj.clone())
    }
}

// Methods specific to Py<PyAny>
impl Py<crate::types::PyAny> {
    /// Convert a `Bound<'_, PyAny>` into `Py<PyAny>`.
    ///
    /// This is an inherent method so that `Py::<PyAny>::try_from` used as a
    /// function reference (e.g. in `.and_then(Py::<PyAny>::try_from)`) returns
    /// `PyResult<Py<PyAny>>` instead of `Result<Py<PyAny>, Infallible>`.
    pub fn try_from(bound: crate::Bound<'_, crate::types::PyAny>) -> crate::PyResult<Self> {
        Ok(Py::from_object(bound.obj))
    }

    /// Call a method with no arguments.
    pub fn call_method0(
        &self,
        py: Python<'_>,
        name: &str,
    ) -> crate::PyResult<Py<crate::types::PyAny>> {
        let bound = self.bind(py);
        let result = bound.as_any().call_method0(name)?;
        Ok(Py::from_object(result.obj))
    }

    /// Call a method with one tuple of arguments.
    pub fn call_method1<'py>(
        &self,
        py: Python<'py>,
        name: &str,
        args: impl crate::conversion::IntoPyArgs<'py>,
    ) -> crate::PyResult<Py<crate::types::PyAny>> {
        let bound = self.bind(py);
        let vm = py.vm;
        let method = bound.as_any().getattr(name)?;
        let arg_objs = args.into_py_args(py)?;
        let func_args: rustpython_vm::function::FuncArgs = arg_objs.into();
        let result = crate::err::from_vm_result(method.obj.call_with_args(func_args, vm))?;
        Ok(Py::from_object(result))
    }

    /// Extract a Rust value from this Python object.
    pub fn extract<'py, T: crate::FromPyObject<'py>>(&self, py: Python<'py>) -> crate::PyResult<T> {
        let bound = Bound::<crate::types::PyAny>::from_object(py, self.obj.clone());
        T::extract_bound(&bound)
    }

    /// Convert into a `Bound<'py, PyAny>`.
    pub fn into_pyobject<'py>(
        self,
        py: Python<'py>,
    ) -> Result<Bound<'py, crate::types::PyAny>, crate::PyErr> {
        Ok(Bound::from_object(py, self.obj))
    }

    /// Call this Python object with the given arguments.
    pub fn call<'py>(
        &self,
        py: Python<'py>,
        args: impl crate::conversion::IntoPyArgs<'py>,
        _kwargs: Option<&crate::Bound<'py, crate::types::PyDict>>,
    ) -> crate::PyResult<Py<crate::types::PyAny>> {
        let vm = py.vm;
        let arg_objs = args.into_py_args(py)?;
        let func_args: rustpython_vm::function::FuncArgs = arg_objs.into();
        let result = crate::err::from_vm_result(self.obj.call_with_args(func_args, vm))?;
        Ok(Py::from_object(result))
    }

    /// Check if this object is "truthy" in Python's boolean context.
    pub fn is_truthy(&self, py: Python<'_>) -> crate::PyResult<bool> {
        let bound = self.bind(py);
        bound.is_truthy()
    }

    /// Call with a tuple of arguments and optional keyword dict.
    pub fn call1<'py>(
        &self,
        py: Python<'py>,
        args: impl crate::conversion::IntoPyArgs<'py>,
    ) -> crate::PyResult<Py<crate::types::PyAny>> {
        self.call(py, args, None)
    }

    /// Call with args tuple and optional kwargs dict (pyo3 3-arg call pattern).
    pub fn call3<'py>(
        &self,
        py: Python<'py>,
        args: impl crate::conversion::IntoPyArgs<'py>,
        kwargs: Option<&crate::Bound<'py, crate::types::PyDict>>,
    ) -> crate::PyResult<Py<crate::types::PyAny>> {
        self.call(py, args, kwargs)
    }
}

impl<T> Clone for Py<T> {
    fn clone(&self) -> Self {
        Py {
            obj: self.obj.clone(),
            _marker: PhantomData,
        }
    }
}

impl<T> std::fmt::Debug for Py<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Py({:?})", self.obj)
    }
}

/// Conversion from Bound to Py (unbind).
impl<'py, T> From<Bound<'py, T>> for Py<crate::types::PyAny> {
    fn from(bound: Bound<'py, T>) -> Self {
        Py::from_object(bound.obj)
    }
}

/// Conversion from Py to PyObjectRef.
impl<T> From<Py<T>> for PyObjectRef {
    fn from(py: Py<T>) -> Self {
        py.obj
    }
}

/// Conversion from &Py to PyObjectRef (via clone).
impl<T> From<&Py<T>> for PyObjectRef {
    fn from(py: &Py<T>) -> Self {
        py.obj.clone()
    }
}

/// Implement TryFromObject for Py<PyAny> so it works with #[pymethod] params.
impl rustpython_vm::convert::TryFromObject for Py<crate::types::PyAny> {
    fn try_from_object(
        _vm: &rustpython_vm::VirtualMachine,
        obj: PyObjectRef,
    ) -> rustpython_vm::PyResult<Self> {
        Ok(Py::from_object(obj))
    }
}

impl<T: rustpython_vm::PyPayload + crate::PyTypeObjectExt> TryFrom<crate::Bound<'_, T>>
    for crate::Py<T>
{
    type Error = crate::PyErr;

    fn try_from(bound: crate::Bound<'_, T>) -> crate::PyResult<Self> {
        Ok(crate::Py::from_object(bound.obj))
    }
}

/// A borrowed Python object reference tied to a `Python<'py>` lifetime token.
/// Analogous to PyO3's `Bound<'py, T>`.
pub struct Bound<'py, T> {
    pub(crate) py: Python<'py>,
    pub(crate) obj: PyObjectRef,
    _marker: PhantomData<T>,
}

impl<'py, T> Bound<'py, T> {
    fn marker_type_matches<U>(&self) -> Option<bool> {
        let type_name = std::any::type_name::<U>();
        if type_name.contains("PyType")
            && type_name.contains("::types::")
            && !type_name.contains("PyTypeError")
        {
            return Some(
                self.obj
                    .class()
                    .fast_issubclass(self.py.vm.ctx.types.type_type),
            );
        }
        if type_name.contains("::types::dict::PyDict") {
            return Some(
                self.obj
                    .class()
                    .fast_issubclass(self.py.vm.ctx.types.dict_type),
            );
        }
        if type_name.contains("::types::list::PyList") {
            return Some(
                self.obj
                    .class()
                    .fast_issubclass(self.py.vm.ctx.types.list_type),
            );
        }
        if type_name.contains("::types::tuple::PyTuple") {
            return Some(
                self.obj
                    .class()
                    .fast_issubclass(self.py.vm.ctx.types.tuple_type),
            );
        }
        if type_name.contains("::types::string::PyString") {
            return Some(
                self.obj
                    .class()
                    .fast_issubclass(self.py.vm.ctx.types.str_type),
            );
        }
        if type_name.contains("::types::module::PyModule") {
            return Some(
                self.obj
                    .class()
                    .fast_issubclass(self.py.vm.ctx.types.module_type),
            );
        }
        None
    }

    /// Construct from a raw `PyObjectRef`.
    #[doc(hidden)]
    pub fn from_object(py: Python<'py>, obj: PyObjectRef) -> Self {
        Bound {
            py,
            obj,
            _marker: PhantomData,
        }
    }

    /// Return the `Python<'py>` token this reference is tied to.
    pub fn py(&self) -> Python<'py> {
        self.py
    }

    /// Access the inner `PyObjectRef`.
    pub fn as_pyobject(&self) -> &PyObjectRef {
        &self.obj
    }

    /// Access the inner `PyObjectRef` (public accessor for derive macros).
    #[doc(hidden)]
    pub fn as_pyobject_ref(&self) -> &PyObjectRef {
        &self.obj
    }

    /// Erase the type tag, returning `Bound<'py, PyAny>`.
    pub fn as_any(&self) -> &Bound<'py, crate::types::PyAny> {
        // SAFETY: Bound<'py, T> has identical layout for all T (PhantomData is ZST).
        unsafe { &*(self as *const Bound<'py, T> as *const Bound<'py, crate::types::PyAny>) }
    }

    /// Reinterpret as `&Bound<'py, U>` without any runtime check (borrowed).
    ///
    /// Used in generated wrappers to convert `Bound<'py, PyAny>` into
    /// whatever typed `Bound<'py, T>` the original method expects.
    pub fn downcast_unchecked<U>(&self) -> &Bound<'py, U> {
        // SAFETY: Bound<'py, T> and Bound<'py, U> have the same layout.
        unsafe { &*(self as *const Bound<'py, T> as *const Bound<'py, U>) }
    }

    /// Reinterpret as `Bound<'py, U>` without any runtime check (owned).
    pub fn unchecked_cast<U>(self) -> Bound<'py, U> {
        Bound {
            py: self.py,
            obj: self.obj,
            _marker: PhantomData,
        }
    }

    /// Downcast and extract a reference to the underlying Rust payload `T`.
    ///
    /// Returns `Some(&'py T)` if the object is an instance of `T`,
    /// or `None` otherwise. The returned reference has lifetime `'py`
    /// because the data lives on the heap behind a reference-counted
    /// pointer that is guaranteed to be alive while the GIL is held.
    pub fn downcast_payload<P: rustpython_vm::PyPayload>(&self) -> Option<&'py P> {
        if self.obj.downcastable::<P>() {
            Some(unsafe {
                let py_ref = self.obj.downcast_ref::<P>().unwrap();
                &*(&**py_ref as *const P)
            })
        } else {
            None
        }
    }

    pub fn cast_into<U: rustpython_vm::PyPayload>(self) -> crate::PyResult<Bound<'py, U>> {
        let can_cast = if self.obj.downcast_ref::<U>().is_some() {
            true
        } else if let Some(matches) = self.marker_type_matches::<U>() {
            matches
        } else {
            false
        };
        if can_cast {
            Ok(Bound {
                py: self.py,
                obj: self.obj,
                _marker: PhantomData,
            })
        } else {
            let type_name = std::any::type_name::<U>();
            Err(crate::PyErr::new_type_error(
                self.py,
                format!("expected {}, got {}", type_name, self.obj.class().name()),
            ))
        }
    }

    fn can_cast_into<U: rustpython_vm::PyPayload>(this: &Bound<'py, crate::types::PyAny>) -> bool {
        if this.obj.downcast_ref::<U>().is_some() {
            return true;
        }
        this.marker_type_matches::<U>().unwrap_or(false)
    }

    /// Alias for `unchecked_cast`. Used by some pyo3 code.
    pub fn cast_into_unchecked<U>(self) -> Bound<'py, U> {
        self.unchecked_cast()
    }

    /// Convert to an owned, untyped `Bound<'py, PyAny>`.
    pub fn into_any(self) -> Bound<'py, crate::types::PyAny> {
        Bound {
            py: self.py,
            obj: self.obj,
            _marker: PhantomData,
        }
    }

    /// Detach from the `Python<'py>` lifetime, producing a `Py<T>`.
    pub fn unbind(self) -> Py<T> {
        Py::from_object(self.obj)
    }

    /// Create a `Borrowed` reference from this `Bound`.
    pub fn as_borrowed(&self) -> Borrowed<'_, 'py, T> {
        Borrowed {
            py: self.py,
            obj: self.obj.clone(),
            _marker: PhantomData,
        }
    }

    /// Extract a Rust value from this Python object.
    pub fn extract<R: crate::FromPyObject<'py>>(&self) -> crate::PyResult<R> {
        let ob = self.as_any();
        R::extract_bound(ob)
    }

    /// Get an attribute by name. Works on any `Bound<'py, T>`.
    pub fn getattr(&self, name: &str) -> crate::PyResult<Bound<'py, crate::types::PyAny>> {
        let vm = self.py.vm;
        let name_obj = vm.ctx.new_str(name);
        let result = crate::err::from_vm_result(self.obj.get_attr(&name_obj, vm))?;
        Ok(Bound::from_object(self.py, result))
    }

    /// Call a method with no arguments. Works on any `Bound<'py, T>`.
    pub fn call_method0(&self, name: &str) -> crate::PyResult<Bound<'py, crate::types::PyAny>> {
        let vm = self.py.vm;
        let result = crate::err::from_vm_result(vm.call_method(&self.obj, name, ()))?;
        Ok(Bound::from_object(self.py, result))
    }

    /// Call a method with positional arguments. Works on any `Bound<'py, T>`.
    /// Accepts either a `&Bound<PyTuple>` or a tuple of args implementing `IntoPyArgs`.
    pub fn call_method1(
        &self,
        name: &str,
        args: impl crate::conversion::IntoPyArgs<'py>,
    ) -> crate::PyResult<Bound<'py, crate::types::PyAny>> {
        let vm = self.py.vm;
        let positional = args.into_py_args(self.py)?;
        let func_args: rustpython_vm::function::FuncArgs = positional.into();
        let interned = vm.ctx.intern_str(name);
        let method = crate::err::from_vm_result(self.obj.get_attr(interned, vm))?;
        let result = crate::err::from_vm_result(method.call_with_args(func_args, vm))?;
        Ok(Bound::from_object(self.py, result))
    }

    /// Python `==` comparison.
    pub fn eq<O: crate::conversion::ToPyObject>(&self, other: O) -> crate::PyResult<bool> {
        let vm = self.py.vm;
        let other_obj = other.to_object(self.py).obj;
        crate::err::from_vm_result(self.obj.rich_compare_bool(
            &other_obj,
            rustpython_vm::types::PyComparisonOp::Eq,
            vm,
        ))
    }

    /// Python `!=` comparison.
    pub fn ne<O: crate::conversion::ToPyObject>(&self, other: O) -> crate::PyResult<bool> {
        let vm = self.py.vm;
        let other_obj = other.to_object(self.py).obj;
        crate::err::from_vm_result(self.obj.rich_compare_bool(
            &other_obj,
            rustpython_vm::types::PyComparisonOp::Ne,
            vm,
        ))
    }

    /// Python `<` comparison.
    pub fn lt<O: crate::conversion::ToPyObject>(&self, other: O) -> crate::PyResult<bool> {
        let vm = self.py.vm;
        let other_obj = other.to_object(self.py).obj;
        crate::err::from_vm_result(self.obj.rich_compare_bool(
            &other_obj,
            rustpython_vm::types::PyComparisonOp::Lt,
            vm,
        ))
    }

    /// Python `<=` comparison.
    pub fn le<O: crate::conversion::ToPyObject>(&self, other: O) -> crate::PyResult<bool> {
        let vm = self.py.vm;
        let other_obj = other.to_object(self.py).obj;
        crate::err::from_vm_result(self.obj.rich_compare_bool(
            &other_obj,
            rustpython_vm::types::PyComparisonOp::Le,
            vm,
        ))
    }

    /// Python `>` comparison.
    pub fn gt<O: crate::conversion::ToPyObject>(&self, other: O) -> crate::PyResult<bool> {
        let vm = self.py.vm;
        let other_obj = other.to_object(self.py).obj;
        crate::err::from_vm_result(self.obj.rich_compare_bool(
            &other_obj,
            rustpython_vm::types::PyComparisonOp::Gt,
            vm,
        ))
    }

    /// Python `>=` comparison.
    pub fn ge<O: crate::conversion::ToPyObject>(&self, other: O) -> crate::PyResult<bool> {
        let vm = self.py.vm;
        let other_obj = other.to_object(self.py).obj;
        crate::err::from_vm_result(self.obj.rich_compare_bool(
            &other_obj,
            rustpython_vm::types::PyComparisonOp::Ge,
            vm,
        ))
    }

    /// Access the pyclass payload of a frozen class.
    ///
    /// This is pyo3's pattern for `#[pyclass(frozen)]` types: `this.get()`
    /// returns `&T` by extracting the payload from the inner PyObjectRef.
    pub fn get(&self) -> &T
    where
        T: rustpython_vm::PyPayload,
    {
        self.obj
            .downcast_ref::<T>()
            .expect("Bound::get(): wrong payload type")
    }

    /// Convert this `Bound<T>` into a `Bound<U>` by value, without any runtime check.
    ///
    /// Unlike `downcast_unchecked` which returns a reference, this consumes `self`.
    /// In our shim `U` is a phantom type marker only, so this is always memory-safe.
    pub fn into_type<U>(self) -> Bound<'py, U> {
        Bound {
            py: self.py,
            obj: self.obj,
            _marker: PhantomData,
        }
    }

    /// Try to downcast to a specific type.
    pub fn cast<U>(&self) -> crate::PyResult<Bound<'py, U>> {
        let type_name = std::any::type_name::<U>();
        if type_name.contains("PyMapping") {
            let vm = self.py.vm;
            use rustpython_vm::builtins::PyDict;
            if self.obj.downcast_ref::<PyDict>().is_some() {
                return Ok(Bound {
                    py: self.py,
                    obj: self.obj.clone(),
                    _marker: PhantomData,
                });
            }
            if vm.call_method(&self.obj, "items", ()).is_ok() {
                return Ok(Bound {
                    py: self.py,
                    obj: self.obj.clone(),
                    _marker: PhantomData,
                });
            }
            return Err(crate::PyErr::from_vm_err(
                vm.new_type_error("not a mapping"),
            ));
        }
        if let Some(matches) = self.marker_type_matches::<U>() {
            if !matches {
                return Err(crate::PyErr::new_type_error(
                    self.py,
                    format!("expected {}, got {}", type_name, self.obj.class().name()),
                ));
            }
        }
        Ok(Bound {
            py: self.py,
            obj: self.obj.clone(),
            _marker: PhantomData,
        })
    }
}

impl<'py, T: rustpython_vm::PyPayload> std::ops::Deref for Bound<'py, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.get()
    }
}

impl<'py, T> Clone for Bound<'py, T> {
    fn clone(&self) -> Self {
        Bound {
            py: self.py,
            obj: self.obj.clone(),
            _marker: PhantomData,
        }
    }
}

impl<T> std::fmt::Debug for Bound<'_, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Bound({:?})", self.obj)
    }
}

/// Conversion from Bound<'_, T> to PyObjectRef.
impl<T> From<Bound<'_, T>> for PyObjectRef {
    fn from(bound: Bound<'_, T>) -> Self {
        bound.obj
    }
}

/// Conversion from &Bound<'_, T> to PyObjectRef (via clone).
impl<T> From<&Bound<'_, T>> for PyObjectRef {
    fn from(bound: &Bound<'_, T>) -> Self {
        bound.obj.clone()
    }
}

/// Conversion from Borrowed<'_, '_, T> to PyObjectRef (via clone).
impl<T> From<Borrowed<'_, '_, T>> for PyObjectRef {
    fn from(b: Borrowed<'_, '_, T>) -> Self {
        b.obj
    }
}

/// A borrowed reference to a Python object. Lighter-weight than `Bound`.
/// Analogous to pyo3's `Borrowed<'a, 'py, T>`.
pub struct Borrowed<'a, 'py, T> {
    pub(crate) py: Python<'py>,
    pub(crate) obj: PyObjectRef,
    _marker: PhantomData<&'a T>,
}

impl<'a, 'py, T> Borrowed<'a, 'py, T> {
    /// Construct a `Borrowed` from raw parts (crate-internal only).
    ///
    /// # Safety
    /// The caller must ensure the Python object outlives both `'a` and `'py`.
    /// In our shim, `PyObjectRef` is ref-counted so this is always safe.
    pub(crate) fn from_raw(py: Python<'py>, obj: PyObjectRef) -> Self {
        Borrowed {
            py,
            obj,
            _marker: PhantomData,
        }
    }

    pub fn py(&self) -> Python<'py> {
        self.py
    }

    /// Detach from both lifetimes, producing a `Py<PyAny>`.
    pub fn unbind(self) -> Py<crate::types::PyAny> {
        Py::from_object(self.obj)
    }

    /// Convert to `Bound<'py, PyAny>`.
    pub fn into_any(self) -> Bound<'py, crate::types::PyAny> {
        Bound::from_object(self.py, self.obj)
    }

    /// Convert into an owned `Bound<'py, T>`.
    pub fn into_bound(self) -> Bound<'py, T> {
        Bound {
            py: self.py,
            obj: self.obj,
            _marker: PhantomData,
        }
    }

    /// View as borrowed `PyAny`.
    pub fn as_any(&self) -> &Borrowed<'a, 'py, crate::types::PyAny> {
        unsafe {
            &*(self as *const Borrowed<'a, 'py, T> as *const Borrowed<'a, 'py, crate::types::PyAny>)
        }
    }
}

impl<'a, 'py, T> Clone for Borrowed<'a, 'py, T> {
    fn clone(&self) -> Self {
        Borrowed {
            py: self.py,
            obj: self.obj.clone(),
            _marker: PhantomData,
        }
    }
}

// Note: Borrowed is not Copy because PyObjectRef (Arc-based) is not Copy.

// Borrowed<PyAny> methods
impl<'a, 'py> Borrowed<'a, 'py, crate::types::PyAny> {
    /// Get hash of this object.
    pub fn hash(&self) -> crate::PyResult<isize> {
        let vm = self.py.vm;
        crate::err::from_vm_result(self.obj.hash(vm)).map(|h| h as isize)
    }

    /// Try to iterate over this object.
    pub fn try_iter(&self) -> crate::PyResult<Bound<'py, crate::types::PyIterator>> {
        let vm = self.py.vm;
        let iter_obj = crate::err::from_vm_result(self.obj.get_iter(vm))?;
        let obj_ref: PyObjectRef = iter_obj.into();
        Ok(Bound::from_object(self.py, obj_ref))
    }

    /// Extract a value from this borrowed reference.
    pub fn extract<T: crate::FromPyObject<'py>>(&self) -> crate::PyResult<T> {
        let bound = Bound::<crate::types::PyAny>::from_object(self.py, self.obj.clone());
        T::extract_bound(&bound)
    }

    /// Convert into an owned `Bound<'py, PyAny>`.
    pub fn into_bound_any(self) -> Bound<'py, crate::types::PyAny> {
        Bound::from_object(self.py, self.obj)
    }

    /// Try to cast this borrowed reference to a different type.
    pub fn cast<U>(&self) -> crate::PyResult<Bound<'py, U>> {
        let type_name = std::any::type_name::<U>();
        if type_name.contains("PyMapping") {
            let vm = self.py.vm;
            use rustpython_vm::builtins::PyDict;
            if self.obj.downcast_ref::<PyDict>().is_some() {
                return Ok(Bound {
                    py: self.py,
                    obj: self.obj.clone(),
                    _marker: std::marker::PhantomData,
                });
            }
            if vm.call_method(&self.obj, "items", ()).is_ok() {
                return Ok(Bound {
                    py: self.py,
                    obj: self.obj.clone(),
                    _marker: std::marker::PhantomData,
                });
            }
            return Err(crate::PyErr::from_vm_err(
                vm.new_type_error("not a mapping"),
            ));
        }
        let any_bound = Bound::<crate::types::PyAny>::from_object(self.py, self.obj.clone());
        if let Some(matches) = any_bound.marker_type_matches::<U>() {
            if !matches {
                return Err(crate::PyErr::new_type_error(
                    self.py,
                    format!("expected {}, got {}", type_name, self.obj.class().name()),
                ));
            }
        }
        Ok(Bound {
            py: self.py,
            obj: self.obj.clone(),
            _marker: std::marker::PhantomData,
        })
    }
}

/// Type alias: `PyRef<'py, T>` is a reference to a pyclass payload.
/// In our shim, since RustPython doesn't have PyCell, this wraps a Bound.
pub struct PyRef<'py, T: rustpython_vm::PyPayload> {
    py: Python<'py>,
    inner: rustpython_vm::PyRef<T>,
}

impl<'py, T: rustpython_vm::PyPayload> PyRef<'py, T> {
    pub fn py(&self) -> Python<'py> {
        self.py
    }

    /// Construct a `PyRef<'py, T>` from a Python token and a RustPython `PyRef<T>`.
    pub fn from_vm_ref(py: Python<'py>, inner: rustpython_vm::PyRef<T>) -> Self {
        PyRef { py, inner }
    }

    /// Access the inner RustPython `PyRef<T>`.
    pub fn inner_ref(&self) -> &rustpython_vm::PyRef<T> {
        &self.inner
    }
}

impl<'py, T: rustpython_vm::PyPayload> std::ops::Deref for PyRef<'py, T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.inner
    }
}

/// Type alias: `PyRefMut<'py, T>` is a mutable reference to a pyclass payload.
/// In our single-threaded RustPython context, we use interior mutability.
pub struct PyRefMut<'py, T: rustpython_vm::PyPayload> {
    py: Python<'py>,
    inner: rustpython_vm::PyRef<T>,
}

impl<'py, T: rustpython_vm::PyPayload> PyRefMut<'py, T> {
    pub fn py(&self) -> Python<'py> {
        self.py
    }

    /// Construct a `PyRefMut<'py, T>` from a Python token and a RustPython `PyRef<T>`.
    pub fn from_vm_ref(py: Python<'py>, inner: rustpython_vm::PyRef<T>) -> Self {
        PyRefMut { py, inner }
    }
}

impl<'py, T: rustpython_vm::PyPayload> std::ops::Deref for PyRefMut<'py, T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.inner
    }
}

impl<'py, T: rustpython_vm::PyPayload> std::ops::DerefMut for PyRefMut<'py, T> {
    #[allow(invalid_reference_casting)]
    fn deref_mut(&mut self) -> &mut T {
        // SAFETY: RustPython is single-threaded; no concurrent access.
        // The PyRef is the sole accessor in this context.
        // This is technically UB per Rust's aliasing model but is sound
        // in RustPython's single-threaded execution model.
        unsafe {
            let const_ptr = std::ptr::from_ref::<T>(&*self.inner);
            &mut *(const_ptr as *mut T)
        }
    }
}

/// Allow `PyRef<'py, T>` to be converted into a Python object.
/// This enables `into_pyobject(py)` calls in generated wrappers.
impl<'py, T: rustpython_vm::PyPayload> crate::conversion::IntoPyObject<'py> for PyRef<'py, T> {
    type Target = crate::types::PyAny;
    type Error = crate::PyErr;

    fn into_pyobject(
        self,
        _py: crate::Python<'py>,
    ) -> Result<Bound<'py, crate::types::PyAny>, Self::Error> {
        let obj: rustpython_vm::PyObjectRef = self.inner.into();
        Ok(Bound::from_object(self.py, obj))
    }
}
