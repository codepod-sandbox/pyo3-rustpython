/// Comparison operation enum, matching pyo3's `CompareOp`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompareOp {
    Lt,
    Le,
    Eq,
    Ne,
    Gt,
    Ge,
}
