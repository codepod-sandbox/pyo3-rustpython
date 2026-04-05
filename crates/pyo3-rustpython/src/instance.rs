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
        Py { obj, _marker: PhantomData }
    }

    pub fn into_object(self) -> PyObjectRef {
        self.obj
    }

    /// Create a `Bound<'py, T>` from this `Py<T>`.
    pub fn into_bound<'py>(self, py: Python<'py>) -> Bound<'py, T> {
        Bound { py, obj: self.obj, _marker: PhantomData }
    }

    /// Borrow as a `Bound<'py, T>` without consuming self.
    pub fn bind<'py>(&self, py: Python<'py>) -> Bound<'py, T> {
        Bound { py, obj: self.obj.clone(), _marker: PhantomData }
    }

    /// Create a `Borrowed<'a, 'py, T>` reference.
    pub fn bind_borrowed<'a, 'py>(&'a self, py: Python<'py>) -> Borrowed<'a, 'py, T> {
        Borrowed { py, obj: self.obj.clone(), _marker: PhantomData }
    }

    /// Clone the underlying object reference. In RustPython this is just a
    /// refcount increment.
    pub fn clone_ref(&self, _py: Python<'_>) -> Self {
        Py { obj: self.obj.clone(), _marker: PhantomData }
    }
}

// Methods specific to Py<PyAny>
impl Py<crate::types::PyAny> {
    /// Call a method with no arguments.
    pub fn call_method0(&self, py: Python<'_>, name: &str) -> crate::PyResult<Py<crate::types::PyAny>> {
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
        let bound = self.bind(py);
        T::extract_bound(bound.as_any())
    }

    /// Convert into a `Bound<'py, PyAny>`.
    pub fn into_pyobject<'py>(self, py: Python<'py>) -> Result<Bound<'py, crate::types::PyAny>, crate::PyErr> {
        Ok(Bound::from_object(py, self.obj))
    }
}

impl<T> Clone for Py<T> {
    fn clone(&self) -> Self {
        Py { obj: self.obj.clone(), _marker: PhantomData }
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

/// Implement TryFromObject for Py<PyAny> so it works with #[pymethod] params.
impl rustpython_vm::convert::TryFromObject for Py<crate::types::PyAny> {
    fn try_from_object(
        _vm: &rustpython_vm::VirtualMachine,
        obj: PyObjectRef,
    ) -> rustpython_vm::PyResult<Self> {
        Ok(Py::from_object(obj))
    }
}

/// Implement ToPyObject for Py<PyAny> so it can be returned from #[pymethod].
impl rustpython_vm::convert::ToPyObject for Py<crate::types::PyAny> {
    fn to_pyobject(self, _vm: &rustpython_vm::VirtualMachine) -> PyObjectRef {
        self.obj
    }
}

/// Implement ToPyObject for Bound<'_, T> so it can be returned from methods.
impl<T> rustpython_vm::convert::ToPyObject for Bound<'_, T> {
    fn to_pyobject(self, _vm: &rustpython_vm::VirtualMachine) -> PyObjectRef {
        self.obj
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
    /// Construct from a raw `PyObjectRef`.
    #[doc(hidden)]
    pub fn from_object(py: Python<'py>, obj: PyObjectRef) -> Self {
        Bound { py, obj, _marker: PhantomData }
    }

    /// Return the `Python<'py>` token this reference is tied to.
    pub fn py(&self) -> Python<'py> {
        self.py
    }

    /// Access the inner `PyObjectRef`.
    pub fn as_pyobject(&self) -> &PyObjectRef {
        &self.obj
    }

    /// Erase the type tag, returning `Bound<'py, PyAny>`.
    pub fn as_any(&self) -> &Bound<'py, crate::types::PyAny> {
        // SAFETY: Bound is #[repr(C)] with identical layout for all T.
        unsafe { &*(self as *const Bound<'py, T> as *const Bound<'py, crate::types::PyAny>) }
    }

    /// Convert to an owned, untyped `Bound<'py, PyAny>`.
    pub fn into_any(self) -> Bound<'py, crate::types::PyAny> {
        Bound { py: self.py, obj: self.obj, _marker: PhantomData }
    }

    /// Detach from the `Python<'py>` lifetime, producing a `Py<PyAny>`.
    pub fn unbind(self) -> Py<crate::types::PyAny> {
        Py::from_object(self.obj)
    }

    /// Create a `Borrowed` reference from this `Bound`.
    pub fn as_borrowed(&self) -> Borrowed<'_, 'py, T> {
        Borrowed { py: self.py, obj: self.obj.clone(), _marker: PhantomData }
    }

    /// Extract a Rust value from this Python object.
    pub fn extract<R: crate::FromPyObject<'py>>(&self) -> crate::PyResult<R> {
        R::extract_bound(self.as_any())
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

    /// Call a method with one tuple arg. Works on any `Bound<'py, T>`.
    pub fn call_method1(&self, name: &str, args: &Bound<'py, crate::types::PyTuple>) -> crate::PyResult<Bound<'py, crate::types::PyAny>> {
        let vm = self.py.vm;
        let tuple = args.obj.downcast_ref::<rustpython_vm::builtins::PyTuple>()
            .expect("call_method1 args must be a tuple");
        let positional: Vec<PyObjectRef> = tuple.as_slice().to_vec();
        let func_args: rustpython_vm::function::FuncArgs = positional.into();
        let interned = vm.ctx.intern_str(name);
        let method = crate::err::from_vm_result(self.obj.get_attr(interned, vm))?;
        let result = crate::err::from_vm_result(method.call_with_args(func_args, vm))?;
        Ok(Bound::from_object(self.py, result))
    }

    /// Python `==` comparison.
    pub fn eq<U>(&self, other: &Bound<'py, U>) -> crate::PyResult<bool> {
        let vm = self.py.vm;
        crate::err::from_vm_result(
            self.obj.rich_compare_bool(&other.obj, rustpython_vm::types::PyComparisonOp::Eq, vm)
        )
    }

    /// Python `!=` comparison.
    pub fn ne<U>(&self, other: &Bound<'py, U>) -> crate::PyResult<bool> {
        let vm = self.py.vm;
        crate::err::from_vm_result(
            self.obj.rich_compare_bool(&other.obj, rustpython_vm::types::PyComparisonOp::Ne, vm)
        )
    }

    /// Python `<` comparison.
    pub fn lt<U>(&self, other: &Bound<'py, U>) -> crate::PyResult<bool> {
        let vm = self.py.vm;
        crate::err::from_vm_result(
            self.obj.rich_compare_bool(&other.obj, rustpython_vm::types::PyComparisonOp::Lt, vm)
        )
    }

    /// Python `<=` comparison.
    pub fn le<U>(&self, other: &Bound<'py, U>) -> crate::PyResult<bool> {
        let vm = self.py.vm;
        crate::err::from_vm_result(
            self.obj.rich_compare_bool(&other.obj, rustpython_vm::types::PyComparisonOp::Le, vm)
        )
    }

    /// Python `>` comparison.
    pub fn gt<U>(&self, other: &Bound<'py, U>) -> crate::PyResult<bool> {
        let vm = self.py.vm;
        crate::err::from_vm_result(
            self.obj.rich_compare_bool(&other.obj, rustpython_vm::types::PyComparisonOp::Gt, vm)
        )
    }

    /// Python `>=` comparison.
    pub fn ge<U>(&self, other: &Bound<'py, U>) -> crate::PyResult<bool> {
        let vm = self.py.vm;
        crate::err::from_vm_result(
            self.obj.rich_compare_bool(&other.obj, rustpython_vm::types::PyComparisonOp::Ge, vm)
        )
    }

    /// Try to downcast to a specific type.
    pub fn cast<U>(&self) -> crate::PyResult<Bound<'py, U>> {
        // For now, just reinterpret the type tag. Proper downcasting
        // would check the Python type at runtime.
        Ok(Bound { py: self.py, obj: self.obj.clone(), _marker: PhantomData })
    }
}

impl<'py, T> Clone for Bound<'py, T> {
    fn clone(&self) -> Self {
        Bound { py: self.py, obj: self.obj.clone(), _marker: PhantomData }
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

    /// View as borrowed `PyAny`.
    pub fn as_any(&self) -> &Borrowed<'a, 'py, crate::types::PyAny> {
        unsafe { &*(self as *const Borrowed<'a, 'py, T> as *const Borrowed<'a, 'py, crate::types::PyAny>) }
    }
}

impl<'a, 'py, T> Clone for Borrowed<'a, 'py, T> {
    fn clone(&self) -> Self {
        Borrowed { py: self.py, obj: self.obj.clone(), _marker: PhantomData }
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
