use std::collections::HashMap;
use std::hash::Hash;
use std::borrow::Cow;

use rustpython_vm::builtins::{PyDict, PyFloat, PyTuple};
use rustpython_vm::convert::ToPyObject as RpToPyObject;
use rustpython_vm::convert::TryFromObject as RpTryFromObject;
use rustpython_vm::PyObjectRef;

use crate::bound_object::BoundObject;
use crate::err::{PyErr, PyResult};
use crate::instance::{Borrowed, Bound};
use crate::python::Python;
use crate::types::PyAny;

pub trait FromPyObject<'py>: Sized {
    fn extract_bound(ob: &Bound<'py, PyAny>) -> PyResult<Self>;

    fn extract(ob: Borrowed<'_, 'py, PyAny>) -> PyResult<Self> {
        Self::extract_bound(&ob.into_bound())
    }
}

pub trait IntoPyObject<'py> {
    type Target;
    type Error: Into<PyErr>;

    fn into_pyobject(self, py: Python<'py>) -> Result<Bound<'py, Self::Target>, Self::Error>;
}

pub trait ToPyObject {
    fn to_object<'py>(&self, py: Python<'py>) -> Bound<'py, PyAny>;
}

pub trait IntoPy<T> {
    fn into_py(self, py: Python<'_>) -> T;
}

fn map_vm_err<T>(r: rustpython_vm::PyResult<T>) -> PyResult<T> {
    r.map_err(PyErr::from_vm_err)
}

fn new_bound<'py>(py: Python<'py>, obj: PyObjectRef) -> Bound<'py, PyAny> {
    <Bound<'_, PyAny>>::from_object(py, obj)
}

macro_rules! impl_from_py_via_try_from_object {
    ($($t:ty),* $(,)?) => { $(
        impl<'py> FromPyObject<'py> for $t {
            fn extract_bound(ob: &Bound<'py, PyAny>) -> PyResult<Self> {
                let vm = ob.py().vm;
                map_vm_err(<$t as RpTryFromObject>::try_from_object(vm, ob.obj.clone()))
            }
        }
    )* };
}

impl_from_py_via_try_from_object!(
    i8, i16, i32, i64, isize, u8, u16, u32, u64, usize, bool, String,
);

impl<'py> FromPyObject<'py> for Cow<'py, str> {
    fn extract_bound(ob: &Bound<'py, PyAny>) -> PyResult<Self> {
        let s: String = FromPyObject::extract_bound(ob)?;
        Ok(Cow::Owned(s))
    }
}

macro_rules! impl_into_pyobject_via_to_pyobject {
    ($($t:ty),* $(,)?) => { $(
        impl<'py> IntoPyObject<'py> for $t {
            type Target = PyAny;
            type Error = PyErr;

            fn into_pyobject(self, py: Python<'py>) -> Result<Bound<'py, PyAny>, PyErr> {
                let vm = py.vm;
                let obj = RpToPyObject::to_pyobject(self, vm);
                Ok(new_bound(py, obj))
            }
        }
    )* };
}

impl_into_pyobject_via_to_pyobject!(
    i8, i16, i32, i64, isize, u8, u16, u32, u64, usize, f32, f64, bool, String,
);

macro_rules! impl_legacy_to_py_object {
    ($($t:ty),* $(,)?) => { $(
        impl ToPyObject for $t {
            fn to_object<'py>(&self, py: Python<'py>) -> Bound<'py, PyAny> {
                let vm = py.vm;
                let obj = RpToPyObject::to_pyobject(self.clone(), vm);
                new_bound(py, obj)
            }
        }
    )* };
}

impl_legacy_to_py_object!(
    i8, i16, i32, i64, isize, u8, u16, u32, u64, usize, f32, f64, bool, String,
);

impl<T> ToPyObject for crate::Bound<'_, T> {
    fn to_object<'py>(&self, py: Python<'py>) -> crate::Bound<'py, PyAny> {
        new_bound(py, self.obj.clone())
    }
}

impl<'a, 'py, T> IntoPyObject<'py> for &'a crate::Bound<'py, T> {
    type Target = PyAny;
    type Error = PyErr;

    fn into_pyobject(self, py: Python<'py>) -> Result<Bound<'py, PyAny>, PyErr> {
        Ok(new_bound(py, self.obj.clone()))
    }
}

impl<'py> FromPyObject<'py> for f64 {
    fn extract_bound(ob: &Bound<'py, PyAny>) -> PyResult<Self> {
        let vm = ob.py().vm;
        match ob.obj.downcast_ref::<PyFloat>() {
            Some(f) => Ok(f.to_f64()),
            None => {
                let float_obj = map_vm_err(
                    ob.obj
                        .clone()
                        .try_into_value::<rustpython_vm::PyRef<PyFloat>>(vm),
                )?;
                Ok(float_obj.to_f64())
            }
        }
    }
}

impl<'py> FromPyObject<'py> for f32 {
    fn extract_bound(ob: &Bound<'py, PyAny>) -> PyResult<Self> {
        let val: f64 = FromPyObject::extract_bound(ob)?;
        Ok(val as f32)
    }
}

impl<'py> IntoPyObject<'py> for &str {
    type Target = PyAny;
    type Error = PyErr;

    fn into_pyobject(self, py: Python<'py>) -> Result<Bound<'py, PyAny>, PyErr> {
        let vm = py.vm;
        let obj = RpToPyObject::to_pyobject(self, vm);
        Ok(new_bound(py, obj))
    }
}

impl<'py> IntoPyObject<'py> for &String {
    type Target = PyAny;
    type Error = PyErr;

    fn into_pyobject(self, py: Python<'py>) -> Result<Bound<'py, PyAny>, PyErr> {
        self.clone().into_pyobject(py)
    }
}

impl ToPyObject for &str {
    fn to_object<'py>(&self, py: Python<'py>) -> Bound<'py, PyAny> {
        let vm = py.vm;
        let obj = RpToPyObject::to_pyobject(*self, vm);
        new_bound(py, obj)
    }
}

impl<T> ToPyObject for crate::Py<T> {
    fn to_object<'py>(&self, py: Python<'py>) -> Bound<'py, PyAny> {
        new_bound(py, self.obj.clone())
    }
}

impl<T: ToPyObject + Clone> ToPyObject for &T {
    fn to_object<'py>(&self, py: Python<'py>) -> Bound<'py, PyAny> {
        (**self).clone().to_object(py)
    }
}

impl<T> rustpython_vm::convert::ToPyObject for crate::Py<T> {
    fn to_pyobject(self, _vm: &rustpython_vm::VirtualMachine) -> PyObjectRef {
        self.obj
    }
}

impl<T> rustpython_vm::convert::ToPyObject for &crate::Py<T> {
    fn to_pyobject(self, _vm: &rustpython_vm::VirtualMachine) -> PyObjectRef {
        self.obj.clone()
    }
}

impl<'py, T> IntoPyObject<'py> for crate::Py<T> {
    type Target = PyAny;
    type Error = PyErr;

    fn into_pyobject(self, py: Python<'py>) -> Result<Bound<'py, PyAny>, PyErr> {
        Ok(new_bound(py, self.obj))
    }
}

impl<'a, 'py, T> IntoPyObject<'py> for &'a crate::Py<T> {
    type Target = PyAny;
    type Error = PyErr;

    fn into_pyobject(self, py: Python<'py>) -> Result<Bound<'py, PyAny>, PyErr> {
        Ok(new_bound(py, self.obj.clone()))
    }
}

impl<T> rustpython_vm::convert::ToPyObject for crate::Bound<'_, T> {
    fn to_pyobject(self, _vm: &rustpython_vm::VirtualMachine) -> PyObjectRef {
        self.obj
    }
}

impl<T> rustpython_vm::convert::ToPyObject for &crate::Bound<'_, T> {
    fn to_pyobject(self, _vm: &rustpython_vm::VirtualMachine) -> PyObjectRef {
        self.obj.clone()
    }
}

impl<T: ToPyObject> ToPyObject for Option<T> {
    fn to_object<'py>(&self, py: Python<'py>) -> Bound<'py, PyAny> {
        match self {
            Some(v) => v.to_object(py),
            None => new_bound(py, py.vm.ctx.none()),
        }
    }
}

impl<T: ToPyObject> ToPyObject for Vec<T> {
    fn to_object<'py>(&self, py: Python<'py>) -> Bound<'py, PyAny> {
        let elements = self
            .iter()
            .map(|item| item.to_object(py).into_any().obj)
            .collect();
        let obj: PyObjectRef = py.vm.ctx.new_list(elements).into();
        new_bound(py, obj)
    }
}

impl<'py, T: FromPyObject<'py>> FromPyObject<'py> for Option<T> {
    fn extract_bound(ob: &Bound<'py, PyAny>) -> PyResult<Self> {
        let vm = ob.py().vm;
        if vm.is_none(&ob.obj) {
            Ok(None)
        } else {
            T::extract_bound(ob).map(Some)
        }
    }
}

impl<'py, T: IntoPyObject<'py>> IntoPyObject<'py> for Option<T> {
    type Target = PyAny;
    type Error = PyErr;

    fn into_pyobject(self, py: Python<'py>) -> Result<Bound<'py, PyAny>, PyErr> {
        match self {
            Some(val) => val
                .into_pyobject(py)
                .map(|b| b.into_any())
                .map_err(Into::into),
            None => {
                let none: PyObjectRef = py.vm.ctx.none();
                Ok(new_bound(py, none))
            }
        }
    }
}

impl<'py, T: FromPyObject<'py>> FromPyObject<'py> for Vec<T> {
    fn extract_bound(ob: &Bound<'py, PyAny>) -> PyResult<Self> {
        let vm = ob.py().vm;
        let py = ob.py();
        let elems: Vec<PyObjectRef> = map_vm_err(vm.extract_elements_with(&ob.obj, Ok))?;
        elems
            .into_iter()
            .map(|elem_obj| {
                let bound_elem = Bound::<PyAny>::from_object(py, elem_obj);
                T::extract_bound(&bound_elem)
            })
            .collect()
    }
}

impl<'py, T> IntoPyObject<'py> for Vec<T>
where
    T: IntoPyObject<'py>,
    T::Error: Into<PyErr>,
{
    type Target = PyAny;
    type Error = PyErr;

    fn into_pyobject(self, py: Python<'py>) -> Result<Bound<'py, PyAny>, PyErr> {
        let vm = py.vm;
        let mut elements = Vec::with_capacity(self.len());
        for item in self {
            elements.push(item.into_pyobject(py).map_err(Into::into)?.into_any().obj);
        }
        let obj: PyObjectRef = vm.ctx.new_list(elements).into();
        Ok(new_bound(py, obj))
    }
}

impl<'py, T, const N: usize> IntoPyObject<'py> for [T; N]
where
    T: IntoPyObject<'py>,
    T::Error: Into<PyErr>,
{
    type Target = PyAny;
    type Error = PyErr;

    fn into_pyobject(self, py: Python<'py>) -> Result<Bound<'py, PyAny>, PyErr> {
        let vm = py.vm;
        let mut elements = Vec::with_capacity(N);
        for item in self {
            elements.push(item.into_pyobject(py).map_err(Into::into)?.into_any().obj);
        }
        let obj: PyObjectRef = vm.ctx.new_list(elements).into();
        Ok(new_bound(py, obj))
    }
}

impl<'py, K, V> FromPyObject<'py> for HashMap<K, V>
where
    K: FromPyObject<'py> + Eq + Hash,
    V: FromPyObject<'py>,
{
    fn extract_bound(ob: &Bound<'py, PyAny>) -> PyResult<Self> {
        let py = ob.py();
        let dict: &rustpython_vm::Py<PyDict> = ob
            .obj
            .downcast_ref::<PyDict>()
            .ok_or_else(|| PyErr::new_type_error(py, "expected a dict"))?;
        let mut map = HashMap::new();
        for (key_obj, val_obj) in dict {
            let key_bound = Bound::<PyAny>::from_object(py, key_obj);
            let key = K::extract_bound(&key_bound)?;
            let val_bound = Bound::<PyAny>::from_object(py, val_obj);
            let val = V::extract_bound(&val_bound)?;
            map.insert(key, val);
        }
        Ok(map)
    }
}

impl<'py, K, V> IntoPyObject<'py> for HashMap<K, V>
where
    K: IntoPyObject<'py>,
    V: IntoPyObject<'py>,
{
    type Target = PyAny;
    type Error = PyErr;

    fn into_pyobject(self, py: Python<'py>) -> Result<Bound<'py, PyAny>, PyErr> {
        let vm = py.vm;
        let dict = vm.ctx.new_dict();
        for (k, v) in self {
            let key_obj = k.into_pyobject(py).map_err(Into::into)?.into_any().obj;
            let val_obj = v.into_pyobject(py).map_err(Into::into)?.into_any().obj;
            map_vm_err(dict.set_item(&*key_obj, val_obj, vm))?;
        }
        let obj: PyObjectRef = dict.into();
        Ok(new_bound(py, obj))
    }
}

impl<'py> FromPyObject<'py> for () {
    fn extract_bound(ob: &Bound<'py, PyAny>) -> PyResult<Self> {
        let tuple = ob
            .obj
            .downcast_ref::<PyTuple>()
            .ok_or_else(|| PyErr::new_type_error(ob.py(), "expected a tuple"))?;
        if tuple.len() != 0 {
            return Err(PyErr::new_type_error(
                ob.py(),
                format!("expected empty tuple, got tuple of length {}", tuple.len()),
            ));
        }
        Ok(())
    }
}

impl<'py> IntoPyObject<'py> for () {
    type Target = PyAny;
    type Error = PyErr;

    fn into_pyobject(self, py: Python<'py>) -> Result<Bound<'py, PyAny>, PyErr> {
        let vm = py.vm;
        let obj: PyObjectRef = vm.ctx.new_tuple(vec![]).into();
        Ok(new_bound(py, obj))
    }
}

macro_rules! impl_tuple {
    ($len:literal; $($idx:tt: $T:ident),+) => {
        impl<'py, $($T: FromPyObject<'py>),+> FromPyObject<'py> for ($($T,)+) {
            fn extract_bound(ob: &Bound<'py, PyAny>) -> PyResult<Self> {
                let py = ob.py();
                let tuple = ob
                    .obj
                    .downcast_ref::<PyTuple>()
                    .ok_or_else(|| PyErr::new_type_error(py, "expected a tuple"))?;
                if tuple.len() != $len {
                    return Err(PyErr::new_type_error(
                        py,
                        format!(
                            "expected tuple of length {}, got length {}",
                            $len,
                            tuple.len()
                        ),
                    ));
                }
                let slice = tuple.as_slice();
                Ok(($(
                    {
                        let bound_elem = Bound::<PyAny>::from_object(py, slice[$idx].clone());
                        $T::extract_bound(&bound_elem)?
                    },
                )+))
            }
        }

        impl<'py, $($T: IntoPyObject<'py>),+> IntoPyObject<'py> for ($($T,)+) {
            type Target = PyAny;
            type Error = PyErr;

            fn into_pyobject(self, py: Python<'py>) -> Result<Bound<'py, PyAny>, PyErr> {
                let vm = py.vm;
                let elements: Vec<PyObjectRef> = vec![
                    $(
                        self.$idx.into_pyobject(py).map_err(Into::into)?.into_any().obj,
                    )+
                ];
                let obj: PyObjectRef = vm.ctx.new_tuple(elements).into();
                Ok(new_bound(py, obj))
            }
        }
    };
}

impl_tuple!(1; 0: T1);
impl_tuple!(2; 0: T1, 1: T2);
impl_tuple!(3; 0: T1, 1: T2, 2: T3);
impl_tuple!(4; 0: T1, 1: T2, 2: T3, 3: T4);
impl_tuple!(5; 0: T1, 1: T2, 2: T3, 3: T4, 4: T5);
impl_tuple!(6; 0: T1, 1: T2, 2: T3, 3: T4, 4: T5, 5: T6);

impl<'py> FromPyObject<'py> for Bound<'py, PyAny> {
    fn extract_bound(ob: &Bound<'py, PyAny>) -> PyResult<Self> {
        Ok(ob.clone())
    }
}

impl<'py, T> IntoPyObject<'py> for Bound<'py, T> {
    type Target = PyAny;
    type Error = PyErr;

    fn into_pyobject(self, _py: Python<'py>) -> Result<Bound<'py, PyAny>, PyErr> {
        Ok(self.into_any())
    }
}

impl<'py> FromPyObject<'py> for PyObjectRef {
    fn extract_bound(ob: &Bound<'py, PyAny>) -> PyResult<Self> {
        Ok(ob.obj.clone())
    }
}

impl<'py> IntoPyObject<'py> for PyObjectRef {
    type Target = PyAny;
    type Error = PyErr;

    fn into_pyobject(self, py: Python<'py>) -> Result<Bound<'py, PyAny>, PyErr> {
        Ok(new_bound(py, self))
    }
}

pub trait IntoPyArgs<'py> {
    fn into_py_args(self, py: Python<'py>) -> PyResult<Vec<PyObjectRef>>;
}

impl<'py> IntoPyArgs<'py> for () {
    fn into_py_args(self, _py: Python<'py>) -> PyResult<Vec<PyObjectRef>> {
        Ok(Vec::new())
    }
}

fn to_pyobj_ref<T: rustpython_vm::convert::ToPyObject>(
    val: T,
    vm: &rustpython_vm::VirtualMachine,
) -> PyObjectRef {
    rustpython_vm::convert::ToPyObject::to_pyobject(val, vm)
}

impl<'py, T: ToPyObject> IntoPyArgs<'py> for (T,) {
    fn into_py_args(self, py: Python<'py>) -> PyResult<Vec<PyObjectRef>> {
        Ok(vec![self.0.to_object(py).obj])
    }
}

impl<'py> IntoPyArgs<'py> for &Bound<'py, crate::types::PyTuple> {
    fn into_py_args(self, _py: Python<'py>) -> PyResult<Vec<PyObjectRef>> {
        let tuple = self
            .obj
            .downcast_ref::<PyTuple>()
            .expect("Bound<PyTuple> must wrap a tuple");
        Ok(tuple.as_slice().to_vec())
    }
}

impl<'py> FromPyObject<'py> for crate::Py<PyAny> {
    fn extract_bound(ob: &Bound<'py, PyAny>) -> PyResult<Self> {
        Ok(crate::Py::from_object(ob.obj.clone()))
    }
}

impl<'py> FromPyObject<'py> for crate::pyclass::CompareOp {
    fn extract_bound(ob: &Bound<'py, PyAny>) -> PyResult<Self> {
        let vm = ob.py().vm;
        let int_val: i32 = map_vm_err(rustpython_vm::convert::TryFromObject::try_from_object(
            vm,
            ob.obj.clone(),
        ))?;
        use crate::pyclass::CompareOp;
        match int_val {
            0 => Ok(CompareOp::Lt),
            1 => Ok(CompareOp::Le),
            2 => Ok(CompareOp::Eq),
            3 => Ok(CompareOp::Ne),
            4 => Ok(CompareOp::Gt),
            5 => Ok(CompareOp::Ge),
            _ => Err(PyErr::new_value_error(
                ob.py(),
                "invalid comparison operation",
            )),
        }
    }
}

impl<'py> FromPyObject<'py> for &'py str {
    fn extract_bound(ob: &Bound<'py, PyAny>) -> PyResult<Self> {
        let vm = ob.py().vm;
        let s: rustpython_vm::builtins::PyStrRef = map_vm_err(ob.obj.clone().try_into_value(vm))?;
        let r = map_vm_err(s.try_as_utf8(vm))?.as_str();
        let ptr = r.as_ptr();
        let len = r.len();
        Ok(unsafe { std::str::from_utf8_unchecked(std::slice::from_raw_parts(ptr, len)) })
    }
}

impl<'py> IntoPyObject<'py> for PyErr {
    type Target = PyAny;
    type Error = PyErr;

    fn into_pyobject(self, py: Python<'py>) -> Result<Bound<'py, PyAny>, PyErr> {
        Ok(self.value(py))
    }
}

impl<'py, T: rustpython_vm::PyPayload + crate::PyTypeObjectExt> FromPyObject<'py> for crate::Py<T> {
    fn extract_bound(ob: &Bound<'py, PyAny>) -> PyResult<Self> {
        let vm = ob.py().vm;
        let inner: rustpython_vm::PyRef<T> = map_vm_err(ob.obj.clone().try_into_value(vm))?;
        let obj: PyObjectRef = inner.into();
        Ok(crate::Py::from_object(obj))
    }
}

pub struct ArgIntoBool(pub bool);

impl From<ArgIntoBool> for bool {
    fn from(val: ArgIntoBool) -> Self {
        val.0
    }
}

impl<'py> FromPyObject<'py> for ArgIntoBool {
    fn extract_bound(ob: &Bound<'py, PyAny>) -> PyResult<Self> {
        let vm = ob.py().vm;
        map_vm_err(ob.obj.clone().try_to_bool(vm)).map(ArgIntoBool)
    }
}

impl From<&ArgIntoBool> for bool {
    fn from(val: &ArgIntoBool) -> Self {
        val.0
    }
}

impl<'py, A: ToPyObject, B: ToPyObject> IntoPyArgs<'py> for (A, B) {
    fn into_py_args(self, py: Python<'py>) -> PyResult<Vec<PyObjectRef>> {
        Ok(vec![self.0.to_object(py).obj, self.1.to_object(py).obj])
    }
}

impl<'py, A: ToPyObject, B: ToPyObject, C: ToPyObject> IntoPyArgs<'py> for (A, B, C) {
    fn into_py_args(self, py: Python<'py>) -> PyResult<Vec<PyObjectRef>> {
        Ok(vec![
            self.0.to_object(py).obj,
            self.1.to_object(py).obj,
            self.2.to_object(py).obj,
        ])
    }
}

impl<'py, A: ToPyObject, B: ToPyObject, C: ToPyObject, D: ToPyObject> IntoPyArgs<'py>
    for (A, B, C, D)
{
    fn into_py_args(self, py: Python<'py>) -> PyResult<Vec<PyObjectRef>> {
        Ok(vec![
            self.0.to_object(py).obj,
            self.1.to_object(py).obj,
            self.2.to_object(py).obj,
            self.3.to_object(py).obj,
        ])
    }
}

impl<'py, A: ToPyObject, B: ToPyObject, C: ToPyObject, D: ToPyObject, E: ToPyObject> IntoPyArgs<'py>
    for (A, B, C, D, E)
{
    fn into_py_args(self, py: Python<'py>) -> PyResult<Vec<PyObjectRef>> {
        Ok(vec![
            self.0.to_object(py).obj,
            self.1.to_object(py).obj,
            self.2.to_object(py).obj,
            self.3.to_object(py).obj,
            self.4.to_object(py).obj,
        ])
    }
}

impl<
        'py,
        A: ToPyObject,
        B: ToPyObject,
        C: ToPyObject,
        D: ToPyObject,
        E: ToPyObject,
        F: ToPyObject,
    > IntoPyArgs<'py> for (A, B, C, D, E, F)
{
    fn into_py_args(self, py: Python<'py>) -> PyResult<Vec<PyObjectRef>> {
        Ok(vec![
            self.0.to_object(py).obj,
            self.1.to_object(py).obj,
            self.2.to_object(py).obj,
            self.3.to_object(py).obj,
            self.4.to_object(py).obj,
            self.5.to_object(py).obj,
        ])
    }
}

impl<
        'py,
        A: ToPyObject,
        B: ToPyObject,
        C: ToPyObject,
        D: ToPyObject,
        E: ToPyObject,
        F: ToPyObject,
        G: ToPyObject,
    > IntoPyArgs<'py> for (A, B, C, D, E, F, G)
{
    fn into_py_args(self, py: Python<'py>) -> PyResult<Vec<PyObjectRef>> {
        Ok(vec![
            self.0.to_object(py).obj,
            self.1.to_object(py).obj,
            self.2.to_object(py).obj,
            self.3.to_object(py).obj,
            self.4.to_object(py).obj,
            self.5.to_object(py).obj,
            self.6.to_object(py).obj,
        ])
    }
}

impl<
        'py,
        A: ToPyObject,
        B: ToPyObject,
        C: ToPyObject,
        D: ToPyObject,
        E: ToPyObject,
        F: ToPyObject,
        G: ToPyObject,
        H: ToPyObject,
    > IntoPyArgs<'py> for (A, B, C, D, E, F, G, H)
{
    fn into_py_args(self, py: Python<'py>) -> PyResult<Vec<PyObjectRef>> {
        Ok(vec![
            self.0.to_object(py).obj,
            self.1.to_object(py).obj,
            self.2.to_object(py).obj,
            self.3.to_object(py).obj,
            self.4.to_object(py).obj,
            self.5.to_object(py).obj,
            self.6.to_object(py).obj,
            self.7.to_object(py).obj,
        ])
    }
}
