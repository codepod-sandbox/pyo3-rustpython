//! RustPython backend implementation for PyO3.
//!
//! // RUSTPYTHON-ASSUMPTION: single-threaded RustPython
//! // See design spec for full rationale.

pub mod gil;
pub mod marker;
pub mod instance;
pub mod err;
