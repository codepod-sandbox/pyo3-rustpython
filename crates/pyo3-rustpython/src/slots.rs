//! Slot fixup for static types.
//!
//! RustPython's `new_static` (used by `make_class`) does not call
//! `init_slots`, so dunder methods registered via `#[pymethod]` are NOT
//! wired to their corresponding type slots (tp_repr, tp_str, etc.).
//!
//! This module provides helpers that detect registered dunder methods and
//! set the appropriate slots.

use rustpython_vm::{
    builtins::{PyStr, PyType},
    Context, Py, PyRef, PyResult,
};

/// Detect dunder methods on a class and wire them to the corresponding
/// type slots. Should be called after `make_class` for user-defined types.
///
/// Currently handles: `__repr__`, `__str__`.
/// TODO: Add `__hash__`, `__eq__`, `__len__`, `__getitem__`, `__setitem__`,
///       `__iter__`, `__next__`, `__add__`, `__sub__`, etc. as needed.
pub fn fixup_dunder_slots(class: &'static Py<PyType>, ctx: &Context) {
    let attrs = class.attributes.read();

    // __repr__
    if attrs.contains_key(ctx.intern_str("__repr__")) {
        class.slots.repr.store(Some(repr_wrapper));
    }

    // __str__
    if attrs.contains_key(ctx.intern_str("__str__")) {
        class.slots.str.store(Some(str_wrapper));
    }
}

fn repr_wrapper(
    zelf: &rustpython_vm::PyObject,
    vm: &rustpython_vm::VirtualMachine,
) -> PyResult<PyRef<PyStr>> {
    let ret = vm.call_special_method(zelf, rustpython_vm::identifier!(vm, __repr__), ())?;
    ret.downcast::<PyStr>().map_err(|obj| {
        vm.new_type_error(format!(
            "__repr__ returned non-string (type {})",
            obj.class()
        ))
    })
}

fn str_wrapper(
    zelf: &rustpython_vm::PyObject,
    vm: &rustpython_vm::VirtualMachine,
) -> PyResult<PyRef<PyStr>> {
    let ret = vm.call_special_method(zelf, rustpython_vm::identifier!(vm, __str__), ())?;
    ret.downcast::<PyStr>().map_err(|obj| {
        vm.new_type_error(format!(
            "__str__ returned non-string (type {})",
            obj.class()
        ))
    })
}
