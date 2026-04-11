use crate::{
    err::{from_vm_result, PyResult},
    instance::Bound,
    python::Python,
};

pub struct PySequence;

impl PySequence {
    pub fn register<T>(_py: Python<'_>) -> PyResult<()> {
        Ok(())
    }
}

impl<'py> Bound<'py, PySequence> {
    pub fn len(&self) -> PyResult<usize> {
        let len_obj = self.call_method0("__len__")?;
        len_obj.extract()
    }
}
