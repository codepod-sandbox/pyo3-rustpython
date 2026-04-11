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
    common::hash::{fix_sentinel, hash_bigint},
    function::{Either, FuncArgs, PyComparisonValue},
    protocol::{PyIterReturn, PyMapping, PySequence},
    types::PyComparisonOp,
    Context, Py, PyObject, PyObjectRef, PyResult, VirtualMachine,
};

/// Detect dunder methods on a class and wire them to the corresponding
/// type slots. Should be called after `make_class` for user-defined types.
pub fn fixup_dunder_slots(class: &'static Py<PyType>, ctx: &Context) {
    let attrs = class.attributes.read();

    drop(attrs); // release read lock before storing slots

    // Mirror safe slot aliases back to their real dunder names so the
    // existing slot wiring below can discover them.
    for dunder in [
        "__repr__",
        "__str__",
        "__hash__",
        "__iter__",
        "__next__",
        "__eq__",
        "__ne__",
        "__lt__",
        "__le__",
        "__gt__",
        "__ge__",
        "__richcmp__",
        "__and__",
        "__or__",
        "__sub__",
        "__xor__",
        "__add__",
        "__mul__",
        "__truediv__",
        "__floordiv__",
        "__mod__",
        "__pow__",
        "__lshift__",
        "__rshift__",
        "__matmul__",
        "__neg__",
        "__pos__",
        "__abs__",
        "__invert__",
        "__int__",
        "__float__",
        "__bool__",
        "__iadd__",
        "__isub__",
        "__imul__",
        "__iand__",
        "__ior__",
        "__ixor__",
        "__contains__",
        "__len__",
        "__getitem__",
        "__setitem__",
        "__delitem__",
        "__reversed__",
        "__reduce__",
        "__call__",
    ] {
        let alias_name = format!("_pyo3_slot_{dunder}");
        let slot_fn = {
            class.attributes
                .read()
                .get(ctx.intern_str(alias_name.as_str()))
                .cloned()
        };
        if let Some(slot_fn) = slot_fn {
            class.set_str_attr(dunder, slot_fn, ctx);
        }
    }

    // __repr__ / __str__
    if class
        .attributes
        .read()
        .contains_key(ctx.intern_str("__repr__"))
    {
        class.slots.repr.store(Some(repr_wrapper));
    }
    if class
        .attributes
        .read()
        .contains_key(ctx.intern_str("__str__"))
    {
        class.slots.str.store(Some(str_wrapper));
    }

    // __hash__
    if class
        .attributes
        .read()
        .contains_key(ctx.intern_str("__hash__"))
    {
        class.slots.hash.store(Some(hash_wrapper));
    }

    // __iter__
    if class
        .attributes
        .read()
        .contains_key(ctx.intern_str("__iter__"))
    {
        class.slots.iter.store(Some(iter_wrapper));
    }

    // __next__
    if class
        .attributes
        .read()
        .contains_key(ctx.intern_str("__next__"))
    {
        class.slots.iternext.store(Some(iternext_wrapper));
    }

    // __eq__ / __ne__ / __lt__ / __le__ / __gt__ / __ge__
    let has_richcompare = {
        let a = class.attributes.read();
        a.contains_key(ctx.intern_str("__eq__"))
            || a.contains_key(ctx.intern_str("__ne__"))
            || a.contains_key(ctx.intern_str("__lt__"))
            || a.contains_key(ctx.intern_str("__le__"))
            || a.contains_key(ctx.intern_str("__gt__"))
            || a.contains_key(ctx.intern_str("__ge__"))
    };
    if has_richcompare {
        class.slots.richcompare.store(Some(richcompare_wrapper));
    }

    // __len__ → both sequence.length and mapping.length
    if class
        .attributes
        .read()
        .contains_key(ctx.intern_str("__len__"))
    {
        class.slots.as_sequence.length.store(Some(seq_len_wrapper));
        class.slots.as_mapping.length.store(Some(map_len_wrapper));
    }

    // __getitem__ → mapping.subscript (handles both integer and key access)
    if class
        .attributes
        .read()
        .contains_key(ctx.intern_str("__getitem__"))
    {
        class
            .slots
            .as_mapping
            .subscript
            .store(Some(map_subscript_wrapper));
    }

    // __contains__ → sequence.contains
    if class
        .attributes
        .read()
        .contains_key(ctx.intern_str("__contains__"))
    {
        class
            .slots
            .as_sequence
            .contains
            .store(Some(seq_contains_wrapper));
    }

    // __call__
    if class
        .attributes
        .read()
        .contains_key(ctx.intern_str("__call__"))
    {
        class.slots.call.store(Some(call_wrapper));
    }
}

// ─── slot wrappers ────────────────────────────────────────────────────────────

fn repr_wrapper(zelf: &PyObject, vm: &VirtualMachine) -> PyResult<rustpython_vm::PyRef<PyStr>> {
    let ret = vm.call_special_method(zelf, rustpython_vm::identifier!(vm, __repr__), ())?;
    ret.downcast::<PyStr>().map_err(|obj| {
        vm.new_type_error(format!(
            "__repr__ returned non-string (type {})",
            obj.class()
        ))
    })
}

fn str_wrapper(zelf: &PyObject, vm: &VirtualMachine) -> PyResult<rustpython_vm::PyRef<PyStr>> {
    let ret = vm.call_special_method(zelf, rustpython_vm::identifier!(vm, __str__), ())?;
    ret.downcast::<PyStr>().map_err(|obj| {
        vm.new_type_error(format!(
            "__str__ returned non-string (type {})",
            obj.class()
        ))
    })
}

fn hash_wrapper(zelf: &PyObject, vm: &VirtualMachine) -> PyResult<i64> {
    let ret = vm.call_special_method(zelf, rustpython_vm::identifier!(vm, __hash__), ())?;
    use rustpython_vm::builtins::PyInt;
    let py_int = ret
        .downcast_ref::<PyInt>()
        .ok_or_else(|| vm.new_type_error("__hash__ method should return an integer"))?;
    let bigint = py_int.as_bigint();
    Ok(bigint
        .try_into()
        .map(fix_sentinel)
        .unwrap_or_else(|_| hash_bigint(bigint)))
}

fn iter_wrapper(zelf: PyObjectRef, vm: &VirtualMachine) -> PyResult {
    vm.call_special_method(&zelf, rustpython_vm::identifier!(vm, __iter__), ())
}

fn iternext_wrapper(zelf: &PyObject, vm: &VirtualMachine) -> PyResult<PyIterReturn> {
    PyIterReturn::from_pyresult(
        vm.call_special_method(zelf, rustpython_vm::identifier!(vm, __next__), ()),
        vm,
    )
}

fn richcompare_wrapper(
    zelf: &PyObject,
    other: &PyObject,
    op: PyComparisonOp,
    vm: &VirtualMachine,
) -> PyResult<Either<PyObjectRef, PyComparisonValue>> {
    let method_name = op.method_name(&vm.ctx);
    vm.call_special_method(zelf, method_name, (other.to_owned(),))
        .map(Either::A)
}

fn len_from_obj(obj: &PyObject, vm: &VirtualMachine) -> PyResult<usize> {
    let ret = vm.call_special_method(obj, rustpython_vm::identifier!(vm, __len__), ())?;
    use rustpython_vm::builtins::PyInt;
    let n = ret
        .downcast_ref::<PyInt>()
        .ok_or_else(|| vm.new_type_error("__len__ should return an integer"))?;
    let v: isize = n.try_to_primitive::<isize>(vm)?;
    if v < 0 {
        return Err(vm.new_value_error("__len__() should return >= 0"));
    }
    Ok(v as usize)
}

fn seq_len_wrapper(seq: PySequence<'_>, vm: &VirtualMachine) -> PyResult<usize> {
    len_from_obj(seq.obj, vm)
}

fn map_len_wrapper(mapping: PyMapping<'_>, vm: &VirtualMachine) -> PyResult<usize> {
    len_from_obj(mapping.obj, vm)
}

fn map_subscript_wrapper(mapping: PyMapping<'_>, key: &PyObject, vm: &VirtualMachine) -> PyResult {
    vm.call_special_method(
        mapping.obj,
        rustpython_vm::identifier!(vm, __getitem__),
        (key.to_owned(),),
    )
}

fn seq_contains_wrapper(
    seq: PySequence<'_>,
    needle: &PyObject,
    vm: &VirtualMachine,
) -> PyResult<bool> {
    let ret = vm.call_special_method(
        seq.obj,
        rustpython_vm::identifier!(vm, __contains__),
        (needle.to_owned(),),
    )?;
    ret.try_to_bool(vm)
}

fn call_wrapper(zelf: &PyObject, args: FuncArgs, vm: &VirtualMachine) -> PyResult {
    vm.call_special_method(zelf, rustpython_vm::identifier!(vm, __call__), args)
}
