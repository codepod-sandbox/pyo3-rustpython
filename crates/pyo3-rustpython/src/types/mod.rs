mod any;
mod bytes;
mod dict;
mod iterator;
mod list;
mod mapping;
pub mod module;
mod none;
mod primitives;
mod set;
mod string;
mod tuple;
mod typeobj;

pub use any::PyAny;
pub use bytes::PyBytes;
pub use dict::PyDict;
pub use iterator::PyIterator;
pub use list::PyList;
pub use mapping::PyMapping;
pub use module::PyModule;
pub use none::PyNone;
pub use primitives::{PyBool, PyFloat, PyInt, PyLong};
pub use set::{PyFrozenSet, PySet};
pub use string::PyString;
pub use tuple::{PyTuple, PyTupleMethods};
pub use typeobj::PyType;

pub trait PyAnyMethods {}
impl PyAnyMethods for Bound<'_, PyAny> {}

pub trait PyTypeMethods {}
impl PyTypeMethods for Bound<'_, PyType> {}

use crate::instance::Bound;
