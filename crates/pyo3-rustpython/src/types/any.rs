/// Marker type for an untyped Python object. Analogous to PyO3's `PyAny`.
///
/// `Bound<'py, PyAny>` is the most general Python object reference.
pub struct PyAny;
