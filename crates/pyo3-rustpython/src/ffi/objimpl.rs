//! Object implementation FFI (refcount, allocation).

pub use super::ffi_object::{Py_DECREF, Py_IncRef};

/// Alias for `Py_DECREF` (some code uses `Py_DecRef`).
pub use super::ffi_object::Py_DECREF as Py_DecRef;
