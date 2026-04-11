//! Python exception types.
//!
//! Each type is a zero-sized struct that knows how to look up its
//! RustPython counterpart from `vm.ctx.exceptions`.

use crate::err::PyErr;
use crate::python::Python;

/// Trait for exception types that can be used with `PyErr::new::<T, _>()`.
pub trait PyExceptionType {
    /// Get the exception type object from the VM context.
    fn type_object_raw(
        py: Python<'_>,
    ) -> &'static rustpython_vm::Py<rustpython_vm::builtins::PyType>;

    /// Create a new PyErr with a message or Python object as the argument.
    ///
    /// Requires being inside a RustPython interpreter context
    /// (the thread-local VM must be set). In practice, exception
    /// creation always happens during Python execution.
    fn new_err<A: ExcArg>(arg: A) -> PyErr {
        Python::with_gil(|py| {
            let exc_type = Self::type_object_raw(py);
            let vm = py.vm;
            let arg_obj = arg.into_exc_arg(py);
            match vm.invoke_exception(exc_type.to_owned(), vec![arg_obj]) {
                Ok(exc) => PyErr::from_vm_err(exc),
                Err(exc) => PyErr::from_vm_err(exc),
            }
        })
    }
}

/// Trait for values that can be used as exception arguments.
/// Analogous to pyo3's `PyErrArguments`.
///
/// Blanket-implemented for all types that implement `IntoPyObject` for any lifetime.
/// This covers `String`, `&str`, `Key`, and other Python-convertible types.
pub trait ExcArg {
    fn into_exc_arg(self, py: Python<'_>) -> rustpython_vm::PyObjectRef;
}

/// Blanket impl for types implementing `IntoPyObject` for any lifetime.
impl<T> ExcArg for T
where
    T: for<'py> crate::conversion::IntoPyObject<'py>,
{
    fn into_exc_arg(self, py: Python<'_>) -> rustpython_vm::PyObjectRef {
        Python::with_gil(|py2| match self.into_pyobject(py2) {
            Ok(bound) => crate::bound_object::BoundObject::into_any(bound).obj,
            Err(_) => py.vm.ctx.none(),
        })
    }
}

macro_rules! impl_exception {
    ($name:ident, $zoo_field:ident) => {
        pub struct $name;

        impl $name {
            /// Create a new `PyErr` with this exception type and a message or value.
            pub fn new_err<A: ExcArg>(arg: A) -> PyErr {
                <Self as PyExceptionType>::new_err(arg)
            }
        }

        impl PyExceptionType for $name {
            fn type_object_raw(
                py: Python<'_>,
            ) -> &'static rustpython_vm::Py<rustpython_vm::builtins::PyType> {
                py.vm.ctx.exceptions.$zoo_field
            }
        }

        impl crate::PyTypeObjectExt for $name {
            fn type_object_raw(
                ctx: &rustpython_vm::Context,
            ) -> &'static rustpython_vm::Py<rustpython_vm::builtins::PyType> {
                ctx.exceptions.$zoo_field
            }
        }

        impl crate::PyTypeInfo for $name {
            const NAME: &'static str = stringify!($name);
            const MODULE: Option<&'static str> = Some("builtins");
        }
    };
}

// Base exceptions
impl_exception!(PyBaseException, base_exception_type);
impl_exception!(PyException, exception_type);

// System
impl_exception!(PySystemExit, system_exit);
impl_exception!(PyKeyboardInterrupt, keyboard_interrupt);
impl_exception!(PyGeneratorExit, generator_exit);

// Iteration
impl_exception!(PyStopIteration, stop_iteration);
impl_exception!(PyStopAsyncIteration, stop_async_iteration);

// Arithmetic
impl_exception!(PyArithmeticError, arithmetic_error);
impl_exception!(PyFloatingPointError, floating_point_error);
impl_exception!(PyOverflowError, overflow_error);
impl_exception!(PyZeroDivisionError, zero_division_error);

// Assertion / Attribute
impl_exception!(PyAssertionError, assertion_error);
impl_exception!(PyAttributeError, attribute_error);

// Buffer / EOF
impl_exception!(PyBufferError, buffer_error);
impl_exception!(PyEOFError, eof_error);

// Import
impl_exception!(PyImportError, import_error);
impl_exception!(PyModuleNotFoundError, module_not_found_error);

// Lookup
impl_exception!(PyLookupError, lookup_error);
impl_exception!(PyIndexError, index_error);
impl_exception!(PyKeyError, key_error);

// Memory
impl_exception!(PyMemoryError, memory_error);

// Name
impl_exception!(PyNameError, name_error);
impl_exception!(PyUnboundLocalError, unbound_local_error);

// OS
impl_exception!(PyOSError, os_error);
impl_exception!(PyBlockingIOError, blocking_io_error);
impl_exception!(PyChildProcessError, child_process_error);
impl_exception!(PyConnectionError, connection_error);
impl_exception!(PyBrokenPipeError, broken_pipe_error);
impl_exception!(PyConnectionAbortedError, connection_aborted_error);
impl_exception!(PyConnectionRefusedError, connection_refused_error);
impl_exception!(PyConnectionResetError, connection_reset_error);
impl_exception!(PyFileExistsError, file_exists_error);
impl_exception!(PyFileNotFoundError, file_not_found_error);
impl_exception!(PyInterruptedError, interrupted_error);
impl_exception!(PyIsADirectoryError, is_a_directory_error);
impl_exception!(PyNotADirectoryError, not_a_directory_error);
impl_exception!(PyPermissionError, permission_error);
impl_exception!(PyProcessLookupError, process_lookup_error);
impl_exception!(PyTimeoutError, timeout_error);

// IOError is an alias for OSError in Python 3
pub type PyIOError = PyOSError;

// Reference
impl_exception!(PyReferenceError, reference_error);

// Runtime
impl_exception!(PyRuntimeError, runtime_error);
impl_exception!(PyNotImplementedError, not_implemented_error);
impl_exception!(PyRecursionError, recursion_error);

// Syntax
impl_exception!(PySyntaxError, syntax_error);
impl_exception!(PyIndentationError, indentation_error);
impl_exception!(PyTabError, tab_error);

// System / Type / Value
impl_exception!(PySystemError, system_error);
impl_exception!(PyTypeError, type_error);
impl_exception!(PyValueError, value_error);

// Unicode
impl_exception!(PyUnicodeError, unicode_error);
impl_exception!(PyUnicodeDecodeError, unicode_decode_error);
impl_exception!(PyUnicodeEncodeError, unicode_encode_error);
impl_exception!(PyUnicodeTranslateError, unicode_translate_error);

// Warnings
impl_exception!(PyWarning, warning);
impl_exception!(PyDeprecationWarning, deprecation_warning);
impl_exception!(PyPendingDeprecationWarning, pending_deprecation_warning);
impl_exception!(PyRuntimeWarning, runtime_warning);
impl_exception!(PySyntaxWarning, syntax_warning);
impl_exception!(PyUserWarning, user_warning);
impl_exception!(PyFutureWarning, future_warning);
impl_exception!(PyImportWarning, import_warning);
impl_exception!(PyUnicodeWarning, unicode_warning);
impl_exception!(PyBytesWarning, bytes_warning);
impl_exception!(PyResourceWarning, resource_warning);
impl_exception!(PyEncodingWarning, encoding_warning);

/// Create a new exception type. Analogous to pyo3's `create_exception!`.
/// Usage: `create_exception!(module_name, ExceptionName, BaseException)`
/// The generated struct implements `PyExceptionType` and `new_err()`.
#[macro_export]
macro_rules! create_exception {
    ($module:ident, $name:ident, $base:ty) => {
        pub struct $name;

        impl $name {
            pub fn new_err<A: $crate::exceptions::ExcArg>(arg: A) -> $crate::PyErr {
                $crate::Python::with_gil(|py| {
                    let vm = py.vm;
                    let base_type =
                        <$base as $crate::exceptions::PyExceptionType>::type_object_raw(py);
                    let module_obj = vm
                        .import($module.to_string(), vm.new_scope_with_builtins(), 0)
                        .ok();
                    let base_type_obj: rustpython_vm::PyObjectRef = base_type.to_owned().into();
                    let name_str = vm.ctx.new_str(stringify!($name));
                    let bases = vm.ctx.new_tuple(vec![base_type_obj]).into();
                    let dict = vm.ctx.new_dict().into();
                    let args = rustpython_vm::function::FuncArgs::new(
                        vec![name_str.into(), bases, dict],
                        rustpython_vm::function::KwArgs::default(),
                    );
                    let type_type = vm.ctx.types.type_type.as_object();
                    let exc_type = match type_type.call_with_args(args, vm) {
                        Ok(t) => t,
                        Err(e) => return $crate::err::PyErr::from_vm_err(e),
                    };
                    if let Some(mod_obj) = module_obj {
                        let _ = vm.set_attr(&mod_obj, stringify!($name), exc_type.clone());
                    }
                    let arg_obj = arg.into_exc_arg(py);
                    match vm.invoke_exception(
                        rustpython_vm::AsObject::as_object(&exc_type)
                            .clone()
                            .downcast_ref::<rustpython_vm::builtins::PyType>()
                            .unwrap()
                            .to_owned(),
                        vec![arg_obj],
                    ) {
                        Ok(exc) => $crate::err::PyErr::from_vm_err(exc),
                        Err(exc) => $crate::err::PyErr::from_vm_err(exc),
                    }
                })
            }
        }
    };
}

/// Import an exception type from a module. Analogous to pyo3's `import_exception!`.
/// Usage: `import_exception!(module_name, ExceptionName)`
#[macro_export]
macro_rules! import_exception {
    ($module:ident, $name:ident) => {
        pub struct $name;

        impl $name {
            pub fn new_err<A: $crate::exceptions::ExcArg>(arg: A) -> $crate::PyErr {
                $crate::Python::with_gil(|py| {
                    let vm = py.vm;
                    let mod_obj =
                        match vm.import($module.to_string(), vm.new_scope_with_builtins(), 0) {
                            Ok(m) => m,
                            Err(e) => return $crate::err::PyErr::from_vm_err(e),
                        };
                    let exc_type_obj = match vm.get_attr(&mod_obj, stringify!($name)) {
                        Ok(obj) => obj,
                        Err(e) => return $crate::err::PyErr::from_vm_err(e),
                    };
                    let arg_obj = arg.into_exc_arg(py);
                    match vm.invoke_exception(
                        rustpython_vm::AsObject::as_object(&exc_type_obj)
                            .clone()
                            .downcast_ref::<rustpython_vm::builtins::PyType>()
                            .unwrap()
                            .to_owned(),
                        vec![arg_obj],
                    ) {
                        Ok(exc) => $crate::err::PyErr::from_vm_err(exc),
                        Err(exc) => $crate::err::PyErr::from_vm_err(exc),
                    }
                })
            }
        }
    };
}
