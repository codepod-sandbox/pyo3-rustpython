use std::ffi::CStr;
use std::os::raw::{c_char, c_int, c_long};

use rustpython_vm::builtins::{PyDict, PyTuple};

use super::ffi_object::{ptr_to_pyobject_ref_borrowed, PyObject};
use super::vm;

unsafe fn parse_long_arg(
    obj: rustpython_vm::PyObjectRef,
    vm: &rustpython_vm::VirtualMachine,
    name: &str,
) -> Result<c_long, rustpython_vm::builtins::PyBaseExceptionRef> {
    obj.try_into_value::<i64>(vm)
        .map(|v| v as c_long)
        .map_err(|_| vm.new_type_error(format!("{name} must be an integer")))
}

pub unsafe fn PyArg_ParseTupleAndKeywords(
    args: *mut PyObject,
    kwds: *mut PyObject,
    format: *const c_char,
    kwlist: *mut *mut c_char,
    out_foo: *mut c_long,
    out_bar: *mut c_long,
) -> c_int {
    if args.is_null() || format.is_null() || kwlist.is_null() || out_foo.is_null() || out_bar.is_null() {
        return 0;
    }

    let vm = vm();
    let format = match CStr::from_ptr(format).to_str() {
        Ok(s) => s,
        Err(_) => {
            crate::err::PyErr::from_vm_err(vm.new_type_error("invalid format string".to_owned())).restore();
            return 0;
        }
    };

    if format != "l|l" {
        crate::err::PyErr::from_vm_err(
            vm.new_not_implemented_error(format!(
                "PyArg_ParseTupleAndKeywords only supports format 'l|l', got {format:?}"
            )),
        )
        .restore();
        return 0;
    }

    let args_ref = ptr_to_pyobject_ref_borrowed(args);
    let Some(tuple) = args_ref.downcast_ref::<PyTuple>() else {
        crate::err::PyErr::from_vm_err(vm.new_type_error("args must be a tuple".to_owned())).restore();
        return 0;
    };

    let arg_values = tuple.as_slice();
    if arg_values.is_empty() || arg_values.len() > 2 {
        crate::err::PyErr::from_vm_err(
            vm.new_type_error("expected 1 or 2 positional arguments".to_owned()),
        )
        .restore();
        return 0;
    }

    let foo_name = match CStr::from_ptr(*kwlist).to_str() {
        Ok(s) => s,
        Err(_) => {
            crate::err::PyErr::from_vm_err(vm.new_type_error("invalid keyword name".to_owned())).restore();
            return 0;
        }
    };
    let bar_name = match CStr::from_ptr(*kwlist.add(1)).to_str() {
        Ok(s) => s,
        Err(_) => {
            crate::err::PyErr::from_vm_err(vm.new_type_error("invalid keyword name".to_owned())).restore();
            return 0;
        }
    };

    let mut foo = match parse_long_arg(arg_values[0].clone(), vm, foo_name) {
        Ok(v) => v,
        Err(e) => {
            crate::err::PyErr::from_vm_err(e).restore();
            return 0;
        }
    };
    let mut bar = if arg_values.len() >= 2 {
        match parse_long_arg(arg_values[1].clone(), vm, bar_name) {
            Ok(v) => v,
            Err(e) => {
                crate::err::PyErr::from_vm_err(e).restore();
                return 0;
            }
        }
    } else {
        0
    };

    if !kwds.is_null() {
        let kwds_ref = ptr_to_pyobject_ref_borrowed(kwds);
        let Some(dict) = kwds_ref.downcast_ref::<PyDict>() else {
            crate::err::PyErr::from_vm_err(vm.new_type_error("keywords must be a dict".to_owned())).restore();
            return 0;
        };

        if let Ok(Some(value)) = dict.get_item_opt(foo_name, vm) {
            match parse_long_arg(value, vm, foo_name) {
                Ok(v) => foo = v,
                Err(e) => {
                    crate::err::PyErr::from_vm_err(e).restore();
                    return 0;
                }
            }
        }
        if let Ok(Some(value)) = dict.get_item_opt(bar_name, vm) {
            match parse_long_arg(value, vm, bar_name) {
                Ok(v) => bar = v,
                Err(e) => {
                    crate::err::PyErr::from_vm_err(e).restore();
                    return 0;
                }
            }
        }
    }

    *out_foo = foo;
    *out_bar = bar;
    1
}
