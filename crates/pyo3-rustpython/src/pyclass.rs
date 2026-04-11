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

impl From<rustpython_vm::types::PyComparisonOp> for CompareOp {
    fn from(op: rustpython_vm::types::PyComparisonOp) -> Self {
        match op {
            rustpython_vm::types::PyComparisonOp::Lt => CompareOp::Lt,
            rustpython_vm::types::PyComparisonOp::Le => CompareOp::Le,
            rustpython_vm::types::PyComparisonOp::Eq => CompareOp::Eq,
            rustpython_vm::types::PyComparisonOp::Ne => CompareOp::Ne,
            rustpython_vm::types::PyComparisonOp::Gt => CompareOp::Gt,
            rustpython_vm::types::PyComparisonOp::Ge => CompareOp::Ge,
            // PyComparisonOp is #[repr(transparent)] over ComparisonOperator which
            // is a C-like enum. The above covers all 6 variants.
        }
    }
}

impl From<CompareOp> for rustpython_vm::types::PyComparisonOp {
    fn from(op: CompareOp) -> Self {
        match op {
            CompareOp::Lt => rustpython_vm::types::PyComparisonOp::Lt,
            CompareOp::Le => rustpython_vm::types::PyComparisonOp::Le,
            CompareOp::Eq => rustpython_vm::types::PyComparisonOp::Eq,
            CompareOp::Ne => rustpython_vm::types::PyComparisonOp::Ne,
            CompareOp::Gt => rustpython_vm::types::PyComparisonOp::Gt,
            CompareOp::Ge => rustpython_vm::types::PyComparisonOp::Ge,
        }
    }
}
