use std::sync::OnceLock;

pub struct PyOnceLock<T>(OnceLock<T>);

impl<T> PyOnceLock<T> {
    pub const fn new() -> Self {
        PyOnceLock(OnceLock::new())
    }

    pub fn get(&self) -> Option<&T> {
        self.0.get()
    }

    pub fn get_or_init<F>(&self, _py: crate::Python<'_>, f: F) -> &T
    where
        F: FnOnce() -> T,
    {
        self.0.get_or_init(f)
    }
}
