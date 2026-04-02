//! Conversion traits bridging pyo3's API to RustPython's conversion machinery.
//!
//! - [`FromPyObject`] extracts Rust values from Python objects.
//! - [`IntoPyObject`] converts Rust values into Python objects.
//! - [`ToPyObject`] and [`IntoPy`] are legacy traits for compatibility.

use std::collections::HashMap;
use std::hash::Hash;

use rustpython_vm::builtins::{PyDict, PyFloat, PyTuple};
use rustpython_vm::convert::ToPyObject as RpToPyObject;
use rustpython_vm::convert::TryFromObject as RpTryFromObject;
use rustpython_vm::PyObjectRef;

use crate::err::{PyErr, PyResult};
use crate::instance::Bound;
use crate::python::Python;
use crate::types::PyAny;

// ---------------------------------------------------------------------------
// Core traits
// ---------------------------------------------------------------------------

/// Extract a Rust value from a Python object.
pub trait FromPyObject<'py>: Sized {
    fn extract_bound(obj: &Bound<'py, PyAny>) -> PyResult<Self>;
}

/// Convert a Rust value into a Python object.
pub trait IntoPyObject<'py> {
    type Target;
    type Error: Into<PyErr>;

    fn into_pyobject(self, py: Python<'py>) -> Result<Bound<'py, Self::Target>, Self::Error>;
}

// ---------------------------------------------------------------------------
// Legacy traits (needed by existing pyo3-style code)
// ---------------------------------------------------------------------------

/// Legacy trait: convert a reference to a Python object.
pub trait ToPyObject {
    fn to_object<'py>(&self, py: Python<'py>) -> Bound<'py, PyAny>;
}

/// Legacy trait: consume self and produce a Python-side value `T`.
pub trait IntoPy<T> {
    fn into_py(self, py: Python<'_>) -> T;
}

// ---------------------------------------------------------------------------
// extract() on Bound<'py, PyAny>
// ---------------------------------------------------------------------------

impl<'py> Bound<'py, PyAny> {
    /// Extract a Rust value from this Python object.
    pub fn extract<T: FromPyObject<'py>>(&self) -> PyResult<T> {
        T::extract_bound(self)
    }
}

// ---------------------------------------------------------------------------
// Helper: bridge RustPython's PyResult to ours
// ---------------------------------------------------------------------------

fn map_vm_err<T>(r: rustpython_vm::PyResult<T>) -> PyResult<T> {
    r.map_err(PyErr::from_vm_err)
}

/// Helper to construct a `Bound<'py, PyAny>` without ambiguity from
/// `Bound<'py, PyIterator>::from_object` which has a different signature.
fn new_bound<'py>(py: Python<'py>, obj: PyObjectRef) -> Bound<'py, PyAny> {
    <Bound<'_, PyAny>>::from_object(py, obj)
}

// ---------------------------------------------------------------------------
// Macro: FromPyObject for types that have RustPython TryFromObject
// ---------------------------------------------------------------------------

macro_rules! impl_from_py_via_try_from_object {
    ($($t:ty),* $(,)?) => { $(
        impl<'py> FromPyObject<'py> for $t {
            fn extract_bound(obj: &Bound<'py, PyAny>) -> PyResult<Self> {
                let vm = obj.py().vm;
                map_vm_err(<$t as RpTryFromObject>::try_from_object(vm, obj.obj.clone()))
            }
        }
    )* };
}

impl_from_py_via_try_from_object!(
    i8, i16, i32, i64, isize,
    u8, u16, u32, u64, usize,
    bool,
    String,
);

// ---------------------------------------------------------------------------
// Macro: IntoPyObject for types that have RustPython ToPyObject
// ---------------------------------------------------------------------------

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
    i8, i16, i32, i64, isize,
    u8, u16, u32, u64, usize,
    f32, f64,
    bool,
    String,
);

// ---------------------------------------------------------------------------
// Macro: ToPyObject (legacy) for the same primitive types
// ---------------------------------------------------------------------------

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
    i8, i16, i32, i64, isize,
    u8, u16, u32, u64, usize,
    f32, f64,
    bool,
    String,
);

// ---------------------------------------------------------------------------
// f32 / f64: FromPyObject (no blanket TryFromObject in RustPython for floats)
// ---------------------------------------------------------------------------

impl<'py> FromPyObject<'py> for f64 {
    fn extract_bound(obj: &Bound<'py, PyAny>) -> PyResult<Self> {
        let vm = obj.py().vm;
        match obj.obj.downcast_ref::<PyFloat>() {
            Some(f) => Ok(f.to_f64()),
            None => {
                // Try coercing via PyRef<PyFloat>
                let float_obj = map_vm_err(
                    obj.obj.clone().try_into_value::<rustpython_vm::PyRef<PyFloat>>(vm),
                )?;
                Ok(float_obj.to_f64())
            }
        }
    }
}

impl<'py> FromPyObject<'py> for f32 {
    fn extract_bound(obj: &Bound<'py, PyAny>) -> PyResult<Self> {
        let val: f64 = FromPyObject::extract_bound(obj)?;
        Ok(val as f32)
    }
}

// ---------------------------------------------------------------------------
// &str: FromPyObject (borrows from the Python string for lifetime 'py)
// ---------------------------------------------------------------------------

// NOTE: Extracting &str from a Python string requires that the PyStr object
// lives long enough. RustPython's PyStr stores a Wtf8 buffer; as_str() may
// fail for non-UTF-8. We cannot safely return a &'py str because the
// PyObjectRef is ref-counted and may not live exactly 'py. For now we only
// support extracting owned String. Users needing &str should extract String.

// ---------------------------------------------------------------------------
// &str: IntoPyObject
// ---------------------------------------------------------------------------

impl<'py> IntoPyObject<'py> for &str {
    type Target = PyAny;
    type Error = PyErr;

    fn into_pyobject(self, py: Python<'py>) -> Result<Bound<'py, PyAny>, PyErr> {
        let vm = py.vm;
        let obj = RpToPyObject::to_pyobject(self, vm);
        Ok(new_bound(py, obj))
    }
}

impl ToPyObject for &str {
    fn to_object<'py>(&self, py: Python<'py>) -> Bound<'py, PyAny> {
        let vm = py.vm;
        let obj = RpToPyObject::to_pyobject(*self, vm);
        new_bound(py, obj)
    }
}

// ---------------------------------------------------------------------------
// Option<T>
// ---------------------------------------------------------------------------

impl<'py, T: FromPyObject<'py>> FromPyObject<'py> for Option<T> {
    fn extract_bound(obj: &Bound<'py, PyAny>) -> PyResult<Self> {
        let vm = obj.py().vm;
        if vm.is_none(&obj.obj) {
            Ok(None)
        } else {
            T::extract_bound(obj).map(Some)
        }
    }
}

impl<'py, T: IntoPyObject<'py>> IntoPyObject<'py> for Option<T> {
    type Target = PyAny;
    type Error = PyErr;

    fn into_pyobject(self, py: Python<'py>) -> Result<Bound<'py, PyAny>, PyErr> {
        match self {
            Some(val) => val.into_pyobject(py).map(|b| b.into_any()).map_err(Into::into),
            None => {
                let none: PyObjectRef = py.vm.ctx.none();
                Ok(new_bound(py, none))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Vec<T>
// ---------------------------------------------------------------------------

impl<'py, T: FromPyObject<'py>> FromPyObject<'py> for Vec<T> {
    fn extract_bound(obj: &Bound<'py, PyAny>) -> PyResult<Self> {
        let vm = obj.py().vm;
        let py = obj.py();
        let elems: Vec<PyObjectRef> =
            map_vm_err(vm.extract_elements_with(&obj.obj, Ok))?;
        elems
            .into_iter()
            .map(|elem_obj| {
                let bound = new_bound(py,elem_obj);
                T::extract_bound(&bound)
            })
            .collect()
    }
}

impl<'py, T: IntoPyObject<'py>> IntoPyObject<'py> for Vec<T> {
    type Target = PyAny;
    type Error = PyErr;

    fn into_pyobject(self, py: Python<'py>) -> Result<Bound<'py, PyAny>, PyErr> {
        let vm = py.vm;
        let elements: Vec<PyObjectRef> = self
            .into_iter()
            .map(|item| {
                item.into_pyobject(py)
                    .map(|b| b.into_any().obj)
                    .map_err(Into::into)
            })
            .collect::<Result<_, PyErr>>()?;
        let list_obj: PyObjectRef = vm.ctx.new_list(elements).into();
        Ok(new_bound(py, list_obj))
    }
}

// ---------------------------------------------------------------------------
// HashMap<K, V>
// ---------------------------------------------------------------------------

impl<'py, K, V> FromPyObject<'py> for HashMap<K, V>
where
    K: FromPyObject<'py> + Eq + Hash,
    V: FromPyObject<'py>,
{
    fn extract_bound(obj: &Bound<'py, PyAny>) -> PyResult<Self> {
        let py = obj.py();
        let dict: &rustpython_vm::Py<PyDict> = obj
            .obj
            .downcast_ref::<PyDict>()
            .ok_or_else(|| PyErr::new_type_error(py, "expected a dict"))?;
        let mut map = HashMap::new();
        for (key_obj, val_obj) in dict {
            let key_bound = new_bound(py,key_obj);
            let val_bound = new_bound(py,val_obj);
            let key = K::extract_bound(&key_bound)?;
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

// ---------------------------------------------------------------------------
// Tuples: () through (T1, ..., T6)
// ---------------------------------------------------------------------------

impl<'py> FromPyObject<'py> for () {
    fn extract_bound(obj: &Bound<'py, PyAny>) -> PyResult<Self> {
        let tuple = obj
            .obj
            .downcast_ref::<PyTuple>()
            .ok_or_else(|| PyErr::new_type_error(obj.py(), "expected a tuple"))?;
        if tuple.len() != 0 {
            return Err(PyErr::new_type_error(
                obj.py(),
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
            fn extract_bound(obj: &Bound<'py, PyAny>) -> PyResult<Self> {
                let py = obj.py();
                let tuple = obj
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
                        let bound = new_bound(py,slice[$idx].clone());
                        $T::extract_bound(&bound)?
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

// ---------------------------------------------------------------------------
// Bound<'py, PyAny>: identity conversions
// ---------------------------------------------------------------------------

impl<'py> FromPyObject<'py> for Bound<'py, PyAny> {
    fn extract_bound(obj: &Bound<'py, PyAny>) -> PyResult<Self> {
        Ok(obj.clone())
    }
}

impl<'py> IntoPyObject<'py> for Bound<'py, PyAny> {
    type Target = PyAny;
    type Error = PyErr;

    fn into_pyobject(self, _py: Python<'py>) -> Result<Bound<'py, PyAny>, PyErr> {
        Ok(self)
    }
}

// ---------------------------------------------------------------------------
// PyObjectRef: pass-through conversion
// ---------------------------------------------------------------------------

impl<'py> FromPyObject<'py> for PyObjectRef {
    fn extract_bound(obj: &Bound<'py, PyAny>) -> PyResult<Self> {
        Ok(obj.obj.clone())
    }
}

impl<'py> IntoPyObject<'py> for PyObjectRef {
    type Target = PyAny;
    type Error = PyErr;

    fn into_pyobject(self, py: Python<'py>) -> Result<Bound<'py, PyAny>, PyErr> {
        Ok(new_bound(py, self))
    }
}
