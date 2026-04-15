mod any;
mod bytes;
mod callable;
mod capsule;
mod datetime;
mod dict;
mod iterator;
mod list;
mod mapping;
pub mod module;
mod none;
mod primitives;
mod sequence;
mod set;
mod string;
mod tuple;
mod typeobj;

pub use any::PyAny;
pub use bytes::PyBytes;
pub use callable::{PyCFunction, PyFunction};
pub use capsule::PyCapsule;
pub use datetime::PyDateTime;
pub use dict::PyDict;
pub use iterator::PyIterator;
pub use list::PyList;
pub use mapping::PyMapping;
pub use module::PyModule;
pub use none::PyNone;
pub use primitives::{PyBool, PyFloat, PyInt, PyLong};
pub use sequence::PySequence;
pub use set::{PyFrozenSet, PySet};
pub use string::PyString;
pub use tuple::{PyTuple, PyTupleMethods};
pub use typeobj::PyType;
pub use dict::IntoPyDict;
pub(crate) use mapping::is_registered_mapping_obj;
pub(crate) use sequence::is_registered_sequence_obj;

pub trait PyAnyMethods<'py> {
    fn len(&self) -> crate::PyResult<usize>;

    fn setattr(
        &self,
        name: &str,
        value: impl crate::conversion::ToPyObject,
    ) -> crate::PyResult<()>;

    fn set_item(
        &self,
        key: impl crate::conversion::ToPyObject,
        value: impl crate::conversion::ToPyObject,
    ) -> crate::PyResult<()>;
}

impl<'py, T> PyAnyMethods<'py> for Bound<'py, T> {
    fn len(&self) -> crate::PyResult<usize> {
        self.as_any().len()
    }

    fn setattr(
        &self,
        name: &str,
        value: impl crate::conversion::ToPyObject,
    ) -> crate::PyResult<()> {
        self.as_any().setattr(name, value)
    }

    fn set_item(
        &self,
        key: impl crate::conversion::ToPyObject,
        value: impl crate::conversion::ToPyObject,
    ) -> crate::PyResult<()> {
        let vm = self.py().vm();
        let key_obj = key.to_object(self.py()).obj;
        let val_obj = value.to_object(self.py()).obj;
        crate::err::from_vm_result(self.as_pyobject().set_item(&*key_obj, val_obj, vm))
    }
}

pub trait PyTypeMethods {}
impl PyTypeMethods for Bound<'_, PyType> {}

use crate::instance::Bound;
