//! Buffer protocol support.
//!
//! Provides `PyBuffer<T>` and `Element` to bridge pyo3's typed buffer API
//! to RustPython's `rustpython_vm::protocol::PyBuffer`.

use std::marker::PhantomData;

/// Marker trait for element types that can appear in a Python buffer.
pub trait Element: Copy + 'static {}

impl Element for u8 {}
impl Element for i8 {}
impl Element for i32 {}

/// A typed buffer wrapping a Python bytes-like object.
///
/// In real pyo3 this holds a reference into the Python buffer; here we copy
/// the data out for simplicity (and safety in the single-threaded RustPython).
pub struct PyBuffer<T: Element> {
    data: Vec<u8>,
    _marker: PhantomData<T>,
}

impl<T: Element> PyBuffer<T> {
    /// Try to obtain a contiguous byte buffer from a Python object that
    /// supports the buffer protocol (bytes, bytearray, memoryview, etc.).
    ///
    /// Fails if the buffer's item size does not match `size_of::<T>()`.
    /// For example, `PyBuffer::<u8>::get` fails on `array.array("i")` because
    /// that array has 4-byte items, not 1-byte items. This mirrors pyo3's
    /// typed buffer semantics.
    pub fn get(obj: &crate::Bound<'_, crate::types::PyAny>) -> crate::PyResult<Self> {
        let vm = obj.py().vm;
        let rp_buf: rustpython_vm::protocol::PyBuffer =
            rustpython_vm::TryFromBorrowedObject::try_from_borrowed_object(vm, &obj.obj)
                .map_err(crate::PyErr::from_vm_err)?;

        // Reject buffers whose item size doesn't match T.
        let expected = std::mem::size_of::<T>();
        if rp_buf.desc.itemsize != expected {
            return Err(crate::PyErr::from_vm_err(vm.new_value_error(format!(
                "buffer must have {}-byte items (itemsize={}), got itemsize={}",
                expected, expected, rp_buf.desc.itemsize,
            ))));
        }

        let data = rp_buf.contiguous_or_collect(|bytes: &[u8]| bytes.to_vec());
        Ok(PyBuffer {
            data,
            _marker: PhantomData,
        })
    }

    /// Get a raw pointer to the buffer data, cast to element type `T`.
    pub fn buf_ptr(&self) -> *const T {
        self.data.as_ptr() as *const T
    }

    /// The length of the buffer in bytes.
    pub fn len_bytes(&self) -> usize {
        self.data.len()
    }

    /// Whether the buffer is C-contiguous. Always true for our copied buffer.
    pub fn is_c_contiguous(&self) -> bool {
        true
    }

    pub fn as_slice(&self, _py: crate::Python<'_>) -> crate::PyResult<&[std::cell::Cell<T>]> {
        let ptr = self.data.as_ptr() as *const std::cell::Cell<T>;
        let len = self.data.len() / std::mem::size_of::<T>();
        Ok(unsafe { std::slice::from_raw_parts(ptr, len) })
    }

    pub fn as_mut_slice(
        &self,
        _py: crate::Python<'_>,
    ) -> crate::PyResult<&[std::cell::Cell<T>]> {
        let ptr = self.data.as_ptr() as *const std::cell::Cell<T>;
        let len = self.data.len() / std::mem::size_of::<T>();
        Ok(unsafe { std::slice::from_raw_parts(ptr, len) })
    }
}

impl<'a, 'py, T: Element> crate::FromPyObject<'a, 'py> for PyBuffer<T> {
    type Error = crate::PyErr;

    fn extract_bound(
        ob: &crate::Bound<'py, crate::types::PyAny>,
    ) -> crate::PyResult<Self> {
        Self::get(ob)
    }
}
