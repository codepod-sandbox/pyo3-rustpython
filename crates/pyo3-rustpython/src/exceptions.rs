//! Python exception types.
//!
//! Each type is a zero-sized struct that knows how to look up its
//! RustPython counterpart from `vm.ctx.exceptions`.

use crate::err::PyErr;
use crate::python::Python;

/// Trait for exception types that can be used with `PyErr::new::<T, _>()`.
pub trait PyExceptionType {
    /// Get the exception type object from the VM context.
    fn type_object_raw(py: Python<'_>) -> &'static rustpython_vm::Py<rustpython_vm::builtins::PyType>;

    /// Create a new PyErr with a message.
    ///
    /// Requires being inside a RustPython interpreter context
    /// (the thread-local VM must be set). In practice, exception
    /// creation always happens during Python execution.
    fn new_err(msg: impl Into<String>) -> PyErr {
        Python::with_gil(|py| {
            let exc_type = Self::type_object_raw(py);
            let vm = py.vm;
            let msg_obj = vm.ctx.new_str(msg.into());
            match vm.invoke_exception(exc_type.to_owned(), vec![msg_obj.into()]) {
                Ok(exc) => PyErr::from_vm_err(exc),
                Err(exc) => PyErr::from_vm_err(exc),
            }
        })
    }
}

macro_rules! impl_exception {
    ($name:ident, $zoo_field:ident) => {
        pub struct $name;

        impl $name {
            /// Create a new `PyErr` with this exception type and a message.
            pub fn new_err(msg: impl Into<String>) -> PyErr {
                <Self as PyExceptionType>::new_err(msg)
            }
        }

        impl PyExceptionType for $name {
            fn type_object_raw(
                py: Python<'_>,
            ) -> &'static rustpython_vm::Py<rustpython_vm::builtins::PyType> {
                py.vm.ctx.exceptions.$zoo_field
            }
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
