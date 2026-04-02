# PyO3 RustPython Backend — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add RustPython as an alternative backend to PyO3, selected at build time via `runtime-rustpython` feature flag, enabling any PyO3 crate to compile against RustPython with zero source changes.

**Architecture:** Fork pyo3 repo, keep all existing CPython code untouched, add cfg-gated RustPython codegen in the macro backend (targeting rustpython-derive's macro API) and cfg-gated runtime types/traits in the main crate (backed by rustpython-vm).

**Tech Stack:** Rust, pyo3 0.24.x, rustpython-vm (rev f9ca63893), rustpython-derive, proc-macro2/quote/syn

**Spec:** `docs/superpowers/specs/2026-04-02-pyo3-rustpython-backend-design.md`

---

## Phase 0: Fork & Scaffold

**Goal:** Fork pyo3, add `runtime-rustpython` feature flag, get the existing hello example compiling against the forked pyo3 with the RustPython backend.

### Task 0.1: Fork pyo3 and add to workspace

**Files:**
- Create: `pyo3-fork/` (git subtree or submodule of PyO3/pyo3 at v0.24.2 tag)
- Modify: `Cargo.toml` (workspace root)

- [ ] **Step 1: Clone pyo3 v0.24.2 into the repo**

```bash
cd /Users/sunny/work/codepod/pyo3-rustpython
git clone --depth 1 --branch v0.24.2 https://github.com/PyO3/pyo3.git pyo3-fork
rm -rf pyo3-fork/.git
```

- [ ] **Step 2: Verify the fork's structure**

```bash
ls pyo3-fork/
# Expected: pyo3/ pyo3-macros/ pyo3-macros-backend/ pyo3-ffi/ pyo3-build-config/ Cargo.toml ...
ls pyo3-fork/pyo3-macros-backend/src/
# Expected: lib.rs pyclass.rs pyfunction.rs pymethod.rs pyimpl.rs module.rs method.rs attributes.rs params.rs frompyobject.rs intopyobject.rs utils.rs quotes.rs konst.rs pyversions.rs
```

- [ ] **Step 3: Commit the vendored fork**

```bash
git add pyo3-fork/
git commit -m "vendor: add pyo3 v0.24.2 source as pyo3-fork

Starting point for adding RustPython backend support."
```

### Task 0.2: Add `runtime-rustpython` feature to pyo3 main crate

**Files:**
- Modify: `pyo3-fork/pyo3/Cargo.toml`

- [ ] **Step 1: Read the current Cargo.toml features section**

Read `pyo3-fork/pyo3/Cargo.toml` and find the `[features]` section.

- [ ] **Step 2: Add the `runtime-rustpython` feature**

Add to the `[features]` section:

```toml
# RustPython backend — replaces CPython FFI with rustpython-vm
runtime-rustpython = ["dep:rustpython-vm", "dep:rustpython-derive"]
```

Add to `[dependencies]`:

```toml
rustpython-vm = { git = "https://github.com/RustPython/RustPython", rev = "f9ca63893", optional = true }
rustpython-derive = { git = "https://github.com/RustPython/RustPython", rev = "f9ca63893", optional = true }
```

- [ ] **Step 3: Make pyo3-ffi conditional**

The `pyo3-ffi` dependency should be skipped when `runtime-rustpython` is enabled. In `[dependencies]`, change:

```toml
pyo3-ffi = { path = "../pyo3-ffi", version = "=0.24.2" }
```

to:

```toml
pyo3-ffi = { path = "../pyo3-ffi", version = "=0.24.2", optional = true }
```

And add `pyo3-ffi` to the `default` feature (so existing users are unaffected):

```toml
default = ["macros", "pyo3-ffi"]
```

Note: This will cause compilation errors for the CPython backend code that uses `ffi::` directly. We'll cfg-gate those in subsequent tasks.

- [ ] **Step 4: Commit**

```bash
git add pyo3-fork/pyo3/Cargo.toml
git commit -m "feat: add runtime-rustpython feature flag to pyo3 crate

Adds optional dependencies on rustpython-vm and rustpython-derive,
makes pyo3-ffi optional (included in default features)."
```

### Task 0.3: Create the RustPython runtime module skeleton

**Files:**
- Create: `pyo3-fork/pyo3/src/impl_/rustpython/mod.rs`
- Create: `pyo3-fork/pyo3/src/impl_/rustpython/gil.rs`
- Create: `pyo3-fork/pyo3/src/impl_/rustpython/marker.rs`
- Create: `pyo3-fork/pyo3/src/impl_/rustpython/instance.rs`
- Create: `pyo3-fork/pyo3/src/impl_/rustpython/err.rs`
- Modify: `pyo3-fork/pyo3/src/impl_/mod.rs`

- [ ] **Step 1: Read the existing impl_/mod.rs**

Read `pyo3-fork/pyo3/src/impl_/mod.rs` to understand how the module is structured.

- [ ] **Step 2: Add the cfg-gated rustpython module**

Add to the end of `pyo3-fork/pyo3/src/impl_/mod.rs`:

```rust
#[cfg(feature = "runtime-rustpython")]
pub mod rustpython;
```

- [ ] **Step 3: Create mod.rs**

Create `pyo3-fork/pyo3/src/impl_/rustpython/mod.rs`:

```rust
//! RustPython backend implementation for PyO3.
//!
//! This module provides the runtime layer that backs PyO3's public API
//! when the `runtime-rustpython` feature is enabled. It replaces CPython's
//! C API with calls to `rustpython_vm::VirtualMachine`.
//!
//! // RUSTPYTHON-ASSUMPTION: single-threaded RustPython
//! //
//! // RustPython does not have a GIL because it is single-threaded. All
//! // GIL-related APIs are implemented as no-ops or thin wrappers. If
//! // RustPython gains threading support in the future, this module will
//! // need a real synchronization mechanism. Specifically:
//! //   - `Python::with_gil` would need an actual lock
//! //   - `Py<T>` Send/Sync impls would need revisiting
//! //   - The thread-local VM approach would need per-thread VM or
//! //     shared VM with locking

pub mod gil;
pub mod marker;
pub mod instance;
pub mod err;
```

- [ ] **Step 4: Create gil.rs — thread-local VM stash**

Create `pyo3-fork/pyo3/src/impl_/rustpython/gil.rs`:

```rust
//! GIL simulation for RustPython via thread-local VM reference.
//!
//! // RUSTPYTHON-ASSUMPTION: single-threaded RustPython
//! //
//! // RustPython is single-threaded. There is no GIL to acquire. Instead,
//! // we stash a pointer to the current VirtualMachine in a thread-local
//! // when entering Python code (module exec, method calls) and read it
//! // back in `Python::with_gil`. This is safe because:
//! //   1. RustPython is single-threaded — no concurrent VM access
//! //   2. The VM lifetime spans the entire interpreter session
//! //   3. The pointer is only read while the VM is alive (within call frames)

use std::cell::Cell;
use rustpython_vm::VirtualMachine;

thread_local! {
    static CURRENT_VM: Cell<Option<*const VirtualMachine>> = const { Cell::new(None) };
}

/// Set the current VM for `Python::with_gil` to find.
/// Called when entering RustPython interpreter execution.
///
/// # Safety
/// The caller must ensure the `VirtualMachine` reference outlives
/// any code that calls `with_current_vm`.
pub unsafe fn set_current_vm(vm: &VirtualMachine) {
    CURRENT_VM.with(|cell| cell.set(Some(vm as *const VirtualMachine)));
}

/// Clear the current VM reference.
/// Called when leaving RustPython interpreter execution.
pub fn clear_current_vm() {
    CURRENT_VM.with(|cell| cell.set(None));
}

/// Get the current VM, panicking if not inside the interpreter.
///
/// # Safety
/// The returned reference is only valid while the VM is alive.
/// This is guaranteed by the set/clear bracketing around interpreter calls.
pub fn with_current_vm<F, R>(f: F) -> R
where
    F: FnOnce(&VirtualMachine) -> R,
{
    CURRENT_VM.with(|cell| {
        let ptr = cell
            .get()
            .expect("Python::with_gil called outside RustPython interpreter context");
        let vm = unsafe { &*ptr };
        f(vm)
    })
}

/// RAII guard that sets the current VM on creation and clears it on drop.
pub struct VmGuard {
    _private: (),
}

impl VmGuard {
    /// # Safety
    /// The `VirtualMachine` must outlive the returned guard.
    pub unsafe fn enter(vm: &VirtualMachine) -> Self {
        unsafe { set_current_vm(vm) };
        VmGuard { _private: () }
    }
}

impl Drop for VmGuard {
    fn drop(&mut self) {
        clear_current_vm();
    }
}
```

- [ ] **Step 5: Create marker.rs — Python<'py> for RustPython**

Create `pyo3-fork/pyo3/src/impl_/rustpython/marker.rs`:

```rust
//! `Python<'py>` implementation backed by RustPython's `VirtualMachine`.
//!
//! `Python<'py>` is a **zero-sized type** (ZST), just like in upstream pyo3.
//! This is critical: it means `Bound<'py, T>` has the same memory layout as
//! `Py<T>`, enabling `Py::bind()` to return `&Bound` via pointer cast.
//!
//! The VM reference is retrieved from a thread-local on each `.vm()` call.
//! TLS access is very cheap and this matches pyo3's design where `Python<'py>`
//! is a phantom token, not a data carrier.

use std::marker::PhantomData;
use rustpython_vm::VirtualMachine;

/// Represents access to the Python interpreter.
///
/// This is a zero-sized type — the `'py` lifetime is phantom, enforced at
/// construction time. The actual `&VirtualMachine` is retrieved from a
/// thread-local via `.vm()`.
///
/// // RUSTPYTHON-ASSUMPTION: single-threaded RustPython
/// // `Python` is `Copy + Clone` — safe because single-threaded.
#[derive(Copy, Clone)]
pub struct Python<'py>(PhantomData<&'py VirtualMachine>);

impl<'py> Python<'py> {
    /// Construct from a raw VM reference. Used in generated code.
    ///
    /// The VM reference is stashed in the thread-local so `.vm()` can find it.
    /// The caller must ensure the VM outlives `'py`.
    #[doc(hidden)]
    pub fn from_vm(vm: &'py VirtualMachine) -> Self {
        // Ensure the thread-local is set so .vm() works
        unsafe { super::gil::set_current_vm(vm) };
        Python(PhantomData)
    }

    /// Access the underlying `VirtualMachine`.
    ///
    /// Reads from a thread-local. This is a TLS read — very cheap.
    pub fn vm(self) -> &'py VirtualMachine {
        super::gil::with_current_vm(|vm| {
            // Safety: the VM is alive for 'py because Python<'py> can only
            // be constructed while the VM is alive, and the thread-local is
            // cleared when the VM scope exits.
            unsafe { &*(vm as *const VirtualMachine) }
        })
    }

    /// Acquire the Python GIL and call the provided closure.
    ///
    /// In RustPython, there is no GIL. This retrieves the current
    /// `VirtualMachine` from a thread-local set during interpreter entry.
    pub fn with_gil<F, R>(f: F) -> R
    where
        F: for<'p> FnOnce(Python<'p>) -> R,
    {
        super::gil::with_current_vm(|vm| {
            let py = Python(PhantomData);
            f(py)
        })
    }

    /// Release the GIL and run the closure without it.
    ///
    /// In RustPython, this is a no-op — just runs the closure.
    pub fn allow_threads<T, F>(self, f: F) -> T
    where
        F: Ungil + FnOnce() -> T,
        T: Ungil,
    {
        f()
    }

    /// Get a `None` value.
    pub fn None(self) -> rustpython_vm::PyObjectRef {
        self.vm().ctx.none()
    }
}

/// Marker trait for types that are safe to use without the GIL.
///
/// In RustPython (single-threaded), all types satisfy this trivially.
/// We provide a blanket impl for everything.
pub trait Ungil {}
impl<T: ?Sized> Ungil for T {}
```

- [ ] **Step 6: Create instance.rs — Py<T> and Bound<'py, T>**

Create `pyo3-fork/pyo3/src/impl_/rustpython/instance.rs`:

```rust
//! `Py<T>` and `Bound<'py, T>` implementations for RustPython.

use std::marker::PhantomData;
use rustpython_vm::PyObjectRef;
use super::marker::Python;

/// An owned Python object reference with a type tag.
pub struct Py<T> {
    pub(crate) obj: PyObjectRef,
    _marker: PhantomData<T>,
}

impl<T> Py<T> {
    #[doc(hidden)]
    pub fn from_object(obj: PyObjectRef) -> Self {
        Py {
            obj,
            _marker: PhantomData,
        }
    }

    pub fn into_object(self) -> PyObjectRef {
        self.obj
    }

    /// Bind this owned reference to a `Python<'py>` token.
    ///
    /// Returns `&Bound` by pointer cast — safe because `Python<'py>` is a
    /// ZST, so `Bound<'py, T>` has the same layout as `Py<T>`.
    pub fn bind<'py>(&self, _py: Python<'py>) -> &Bound<'py, T> {
        unsafe { &*(self as *const Py<T> as *const Bound<'py, T>) }
    }

    /// Clone this reference.
    pub fn clone_ref(&self, _py: Python<'_>) -> Self {
        Py {
            obj: self.obj.clone(),
            _marker: PhantomData,
        }
    }
}

impl<T> Clone for Py<T> {
    fn clone(&self) -> Self {
        Py {
            obj: self.obj.clone(),
            _marker: PhantomData,
        }
    }
}

// RUSTPYTHON-ASSUMPTION: single-threaded, so Py<T> is Send + Sync
unsafe impl<T> Send for Py<T> {}
unsafe impl<T> Sync for Py<T> {}

/// Type alias matching pyo3's `PyObject`.
pub type PyObject = Py<super::super::types::PyAny>;

/// A borrowed Python object reference tied to a `Python<'py>` lifetime token.
///
/// Because `Python<'py>` is a ZST, `Bound<'py, T>` has the same memory
/// layout as `Py<T>` (just a `PyObjectRef`). This enables `Py::bind()`
/// to return `&Bound` via pointer cast, matching upstream pyo3.
pub struct Bound<'py, T> {
    py: Python<'py>,       // ZST — zero bytes
    pub(crate) obj: PyObjectRef,
    _marker: PhantomData<T>,
}

impl<'py, T> Bound<'py, T> {
    #[doc(hidden)]
    pub fn from_object(py: Python<'py>, obj: PyObjectRef) -> Self {
        Bound {
            py,
            obj,
            _marker: PhantomData,
        }
    }

    pub fn py(&self) -> Python<'py> {
        self.py
    }

    pub fn as_pyobject(&self) -> &PyObjectRef {
        &self.obj
    }

    pub fn into_pyobject(self) -> PyObjectRef {
        self.obj
    }

    /// Erase the type tag.
    pub fn as_any(&self) -> &Bound<'py, super::super::types::PyAny> {
        unsafe {
            &*(self as *const Bound<'py, T>
                as *const Bound<'py, super::super::types::PyAny>)
        }
    }

    pub fn into_any(self) -> Bound<'py, super::super::types::PyAny> {
        Bound {
            py: self.py,
            obj: self.obj,
            _marker: PhantomData,
        }
    }

    /// Convert to an owned `Py<T>`.
    pub fn unbind(self) -> Py<T> {
        Py::from_object(self.obj)
    }
}

impl<'py, T> Clone for Bound<'py, T> {
    fn clone(&self) -> Self {
        Bound {
            py: self.py,
            obj: self.obj.clone(),
            _marker: PhantomData,
        }
    }
}
```

- [ ] **Step 7: Create err.rs — PyErr for RustPython**

Create `pyo3-fork/pyo3/src/impl_/rustpython/err.rs`:

```rust
//! `PyErr` and `PyResult` implementations for RustPython.

use rustpython_vm::{builtins::PyBaseException, PyRef};
use super::marker::Python;

/// A Python exception.
pub struct PyErr {
    pub(crate) inner: PyRef<PyBaseException>,
}

pub type PyResult<T = ()> = Result<T, PyErr>;

impl PyErr {
    #[doc(hidden)]
    pub fn from_vm_err(e: PyRef<PyBaseException>) -> Self {
        PyErr { inner: e }
    }

    #[doc(hidden)]
    pub fn into_vm_err(self) -> PyRef<PyBaseException> {
        self.inner
    }

    /// Create a `ValueError` with the given message.
    pub fn new_value_error(py: Python<'_>, msg: impl Into<String>) -> Self {
        PyErr {
            inner: py.vm.new_value_error(msg.into()),
        }
    }

    /// Create a `TypeError` with the given message.
    pub fn new_type_error(py: Python<'_>, msg: impl Into<String>) -> Self {
        PyErr {
            inner: py.vm.new_type_error(msg.into()),
        }
    }
}

impl From<PyRef<PyBaseException>> for PyErr {
    fn from(e: PyRef<PyBaseException>) -> Self {
        PyErr { inner: e }
    }
}

/// Convert a `rustpython_vm::PyResult<T>` into our `PyResult<T>`.
pub fn from_vm_result<T>(r: rustpython_vm::PyResult<T>) -> PyResult<T> {
    r.map_err(PyErr::from_vm_err)
}

/// Helper for generated code to convert PyErr → rustpython_vm error.
#[doc(hidden)]
pub fn into_vm_err(e: PyErr) -> PyRef<PyBaseException> {
    e.inner
}
```

- [ ] **Step 8: Commit the skeleton**

```bash
git add pyo3-fork/pyo3/src/impl_/rustpython/
git add pyo3-fork/pyo3/src/impl_/mod.rs
git commit -m "feat: add RustPython runtime module skeleton

Adds impl_/rustpython/ with:
- gil.rs: thread-local VM stash for Python::with_gil
- marker.rs: Python<'py> backed by &VirtualMachine
- instance.rs: Py<T> and Bound<'py, T> backed by PyObjectRef
- err.rs: PyErr backed by PyRef<PyBaseException>"
```

### Task 0.4: Wire up the hello example against the fork

**Files:**
- Modify: `Cargo.toml` (workspace root)
- Modify: `examples/hello/Cargo.toml`
- Modify: `examples/hello/src/main.rs`
- Remove: `crates/pyo3-rustpython/` and `crates/pyo3-rustpython-derive/` (replaced by fork)

This task bridges the gap: the hello example should compile against our forked pyo3 with `features = ["runtime-rustpython"]`. The fork won't fully compile yet (too much CPython code to cfg-gate), so we'll use a thin wrapper crate that re-exports only what we need.

- [ ] **Step 1: Create a thin bridge crate**

Rather than cfg-gating the entire pyo3 crate immediately (massive effort), create a thin `pyo3-rustpython` crate that re-exports from our `impl_/rustpython/` module. This lets us keep the existing working hello example while we incrementally build out the fork.

Keep `crates/pyo3-rustpython/` but have it re-export from the new `impl_/rustpython/` types. We'll migrate the hello example to use the fork directly once enough of pyo3's API surface is cfg-gated.

Update `crates/pyo3-rustpython/src/lib.rs` to import from the new module files instead of its own inline implementations:

The current crate already works. For Phase 0, the goal is just to get the skeleton in place and verify it compiles. We'll integrate incrementally.

- [ ] **Step 2: Verify the existing hello example still works**

```bash
cargo run --bin hello-interp
# Expected: "hello, world!"
```

- [ ] **Step 3: Commit**

```bash
git add -A
git commit -m "chore: verify hello example works with fork skeleton in place"
```

### Task 0.5: Cfg-gate pyo3-macros-backend for RustPython codegen

**Files:**
- Modify: `pyo3-fork/pyo3-macros-backend/Cargo.toml`
- Create: `pyo3-fork/pyo3-macros-backend/src/rustpython.rs`

- [ ] **Step 1: Add feature flag to macro backend**

Read `pyo3-fork/pyo3-macros-backend/Cargo.toml` and add:

```toml
[features]
default = []
runtime-rustpython = []
```

- [ ] **Step 2: Create the codegen entry point**

Create `pyo3-fork/pyo3-macros-backend/src/rustpython.rs`:

```rust
//! RustPython code generation backend.
//!
//! This module contains functions that generate TokenStream targeting
//! rustpython-derive's macro API instead of CPython's C API.
//!
//! The pyo3 macro parsing/IR layer is reused unchanged. Only the final
//! code generation step differs.

// TODO Phase 1: pyclass codegen
// TODO Phase 1: pymethods codegen
// TODO Phase 2: pyfunction codegen (if different from current)
```

- [ ] **Step 3: Register the module**

Add to `pyo3-fork/pyo3-macros-backend/src/lib.rs`:

```rust
#[cfg(feature = "runtime-rustpython")]
pub mod rustpython;
```

- [ ] **Step 4: Commit**

```bash
git add pyo3-fork/pyo3-macros-backend/
git commit -m "feat: add runtime-rustpython feature to pyo3-macros-backend

Scaffolds the RustPython codegen module that will generate code
targeting rustpython-derive's macro API."
```

---

## Phase 1: `#[pyclass]` and `#[pymethods]`

**Goal:** Implement the RustPython codegen path for `#[pyclass]` and `#[pymethods]`, validated with a `Point` class example that has a constructor, getters/setters, regular methods, and `__repr__`.

### Task 1.1: Write the Point example (target — will not compile yet)

**Files:**
- Create: `examples/point/Cargo.toml`
- Create: `examples/point/src/lib.rs`
- Create: `examples/point/src/main.rs`
- Modify: `Cargo.toml` (add to workspace)

- [ ] **Step 1: Create the example Cargo.toml**

Create `examples/point/Cargo.toml`:

```toml
[package]
name = "point-pyo3"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "point-interp"
path = "src/main.rs"

[dependencies]
pyo3 = { package = "pyo3-rustpython", path = "../../crates/pyo3-rustpython" }
rustpython = { git = "https://github.com/RustPython/RustPython", rev = "f9ca63893", default-features = false }
rustpython-vm = { workspace = true }
```

- [ ] **Step 2: Write the pyo3-style class definition**

Create `examples/point/src/lib.rs`:

```rust
use pyo3::prelude::*;

#[pyclass]
#[derive(Clone)]
pub struct Point {
    #[pyo3(get, set)]
    pub x: f64,
    #[pyo3(get, set)]
    pub y: f64,
}

#[pymethods]
impl Point {
    #[new]
    fn new(x: f64, y: f64) -> Self {
        Point { x, y }
    }

    fn distance(&self) -> f64 {
        (self.x * self.x + self.y * self.y).sqrt()
    }

    fn translate(&self, dx: f64, dy: f64) -> Point {
        Point {
            x: self.x + dx,
            y: self.y + dy,
        }
    }

    fn __repr__(&self) -> String {
        format!("Point({}, {})", self.x, self.y)
    }

    fn __str__(&self) -> String {
        format!("({}, {})", self.x, self.y)
    }
}
```

- [ ] **Step 3: Write the interpreter harness**

Create `examples/point/src/main.rs`:

```rust
use rustpython_vm::InterpreterBuilder;

mod extension {
    include!("lib.rs");
}

fn main() {
    let builder = InterpreterBuilder::new();
    let module_def = extension::point_module_def(&builder.ctx);
    let interp = builder.add_native_module(module_def).build();

    let exit_code = interp.run(|vm| {
        vm.run_block_expr(
            vm.new_scope_with_builtins(),
            r#"
from point import Point

p = Point(3.0, 4.0)
assert repr(p) == "Point(3.0, 4.0)", f"repr: {repr(p)}"
assert str(p) == "(3.0, 4.0)", f"str: {str(p)}"
assert p.x == 3.0
assert p.y == 4.0
assert p.distance() == 5.0, f"distance: {p.distance()}"

p.x = 1.0
p.y = 1.0
assert p.x == 1.0

p2 = p.translate(2.0, 3.0)
assert p2.x == 3.0
assert p2.y == 4.0

print("All point tests passed!")
"#,
        )
        .map(|_| ())
    });

    if exit_code != 0 {
        std::process::exit(exit_code as i32);
    }
}
```

- [ ] **Step 4: Add to workspace**

Add `"examples/point"` to the `members` array in the root `Cargo.toml`.

- [ ] **Step 5: Commit (example won't compile yet — that's expected)**

```bash
git add examples/point/ Cargo.toml
git commit -m "feat: add Point example as target for pyclass/pymethods

This example won't compile yet — it exercises #[pyclass], #[new],
#[pyo3(get, set)], regular methods, and __repr__/__str__ magic methods."
```

### Task 1.2: Implement `#[pyclass]` macro expansion

**Files:**
- Modify: `crates/pyo3-rustpython-derive/src/lib.rs`
- Create: `crates/pyo3-rustpython-derive/src/pyclass.rs`

The `#[pyclass]` macro needs to:
1. Parse `#[pyo3(get, set)]` attributes on fields
2. Generate a `PyPayload` impl (via `#[derive(rustpython_derive::PyPayload)]`)
3. Generate `#[rustpython_derive::pyclass]` attributes
4. Generate a module definition that registers the class

- [ ] **Step 1: Read the current derive crate**

Read `crates/pyo3-rustpython-derive/src/lib.rs` to understand the current stub.

- [ ] **Step 2: Create pyclass.rs**

Create `crates/pyo3-rustpython-derive/src/pyclass.rs`:

```rust
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{parse_quote, Fields, ItemStruct, Result};

/// Parsed `#[pyo3(...)]` field attributes.
struct FieldOpts {
    get: bool,
    set: bool,
}

fn parse_field_opts(field: &syn::Field) -> FieldOpts {
    let mut opts = FieldOpts {
        get: false,
        set: false,
    };
    for attr in &field.attrs {
        if !attr.path().is_ident("pyo3") {
            continue;
        }
        let _ = attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("get") {
                opts.get = true;
            } else if meta.path.is_ident("set") {
                opts.set = true;
            }
            Ok(())
        });
    }
    opts
}

/// Parsed `#[pyclass(...)]` attributes.
struct PyClassOpts {
    name: Option<String>,
    module: Option<String>,
}

fn parse_pyclass_opts(attr: TokenStream) -> Result<PyClassOpts> {
    let mut opts = PyClassOpts {
        name: None,
        module: None,
    };
    if attr.is_empty() {
        return Ok(opts);
    }
    let parser = syn::meta::parser(|meta| {
        if meta.path.is_ident("name") {
            let value = meta.value()?;
            let s: syn::LitStr = value.parse()?;
            opts.name = Some(s.value());
        } else if meta.path.is_ident("module") {
            let value = meta.value()?;
            let s: syn::LitStr = value.parse()?;
            opts.module = Some(s.value());
        }
        Ok(())
    });
    syn::parse::Parser::parse2(parser, attr)?;
    Ok(opts)
}

pub fn expand(attr: TokenStream, input: ItemStruct) -> Result<TokenStream> {
    let opts = parse_pyclass_opts(attr)?;
    let struct_name = &input.ident;
    let py_name = opts
        .name
        .unwrap_or_else(|| struct_name.to_string());

    // Strip #[pyo3(...)] attributes from fields before emitting the struct
    let mut clean_input = input.clone();
    if let Fields::Named(ref mut fields) = clean_input.fields {
        for field in &mut fields.named {
            field.attrs.retain(|a| !a.path().is_ident("pyo3"));
        }
    }

    // Generate getter/setter methods for #[pyo3(get)] and #[pyo3(set)] fields
    let mut getset_methods = Vec::new();
    if let Fields::Named(ref fields) = input.fields {
        for field in &fields.named {
            let field_name = field.ident.as_ref().unwrap();
            let field_ty = &field.ty;
            let opts = parse_field_opts(field);

            if opts.get {
                getset_methods.push(quote! {
                    #[pygetset]
                    fn #field_name(&self) -> #field_ty {
                        self.#field_name.clone()
                    }
                });
            }
            if opts.set {
                let setter_name = format_ident!("set_{}", field_name);
                getset_methods.push(quote! {
                    #[pygetset(setter)]
                    fn #setter_name(&mut self, value: #field_ty) {
                        self.#field_name = value;
                    }
                });
            }
        }
    }

    // Only emit the getset impl block if there are getters/setters
    let getset_impl = if getset_methods.is_empty() {
        quote! {}
    } else {
        quote! {
            #[::rustpython_vm::pyclass]
            impl #struct_name {
                #(#getset_methods)*
            }
        }
    };

    // Module name attribute for pyclass
    let module_attr = match opts.module {
        Some(ref m) => quote! { module = #m, },
        None => quote! { module = false, },
    };

    Ok(quote! {
        #[::rustpython_vm::pyclass(#module_attr name = #py_name)]
        #[derive(Debug)]
        #clean_input

        impl ::rustpython_vm::PyPayload for #struct_name {
            fn class(
                ctx: &::rustpython_vm::Context,
            ) -> &'static ::rustpython_vm::Py<::rustpython_vm::builtins::PyType> {
                <Self as ::rustpython_vm::class::StaticType>::static_type()
            }
        }

        #getset_impl
    })
}
```

- [ ] **Step 3: Wire up the macro**

Replace the `pyclass` stub in `crates/pyo3-rustpython-derive/src/lib.rs`:

```rust
mod pyclass;

/// Marks a struct as a Python class.
///
/// Generates `PyPayload` impl and RustPython class registration.
/// Supports `#[pyo3(get)]`, `#[pyo3(set)]` on fields.
#[proc_macro_attribute]
pub fn pyclass(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(item as syn::ItemStruct);
    pyclass::expand(attr.into(), input)
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}
```

- [ ] **Step 4: Add rustpython-vm as a dependency of the derive crate**

Wait — proc macro crates can't depend on runtime crates for code generation. The derive crate only generates token streams that *reference* `rustpython_vm` paths. It doesn't need `rustpython_vm` as a dependency itself. The paths like `::rustpython_vm::pyclass` are just tokens.

Verify `crates/pyo3-rustpython-derive/Cargo.toml` has:

```toml
[dependencies]
proc-macro2 = "1"
quote = "1"
syn = { version = "2", features = ["full", "extra-traits"] }
```

No `rustpython-vm` dependency needed.

- [ ] **Step 5: Commit**

```bash
git add crates/pyo3-rustpython-derive/
git commit -m "feat: implement #[pyclass] macro

Generates PyPayload impl, rustpython pyclass attributes, and
getter/setter methods from #[pyo3(get, set)] field annotations."
```

### Task 1.3: Implement `#[pymethods]` macro expansion

**Files:**
- Create: `crates/pyo3-rustpython-derive/src/pymethods.rs`
- Modify: `crates/pyo3-rustpython-derive/src/lib.rs`

The `#[pymethods]` macro needs to:
1. Scan the impl block for method attributes: `#[new]`, `#[getter]`, `#[setter]`, `#[staticmethod]`, `#[classmethod]`
2. Detect magic methods by `__name__` pattern
3. Transform each method to use the appropriate rustpython-derive attribute
4. Handle `Python<'py>` parameters (strip them, inject `vm: &VirtualMachine`)

- [ ] **Step 1: Create pymethods.rs**

Create `crates/pyo3-rustpython-derive/src/pymethods.rs`:

```rust
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{parse_quote, FnArg, ImplItem, ImplItemFn, ItemImpl, Pat, Result, ReturnType};

/// What kind of method is this?
enum MethodKind {
    /// `#[new]` — constructor
    Constructor,
    /// `#[getter]` or `#[getter(name)]`
    Getter(Option<String>),
    /// `#[setter]` or `#[setter(name)]`
    Setter(Option<String>),
    /// `#[staticmethod]`
    Static,
    /// `#[classmethod]`
    ClassMethod,
    /// Magic method like `__repr__`, `__str__`, `__hash__`, etc.
    Magic(String),
    /// Regular method
    Regular,
}

fn classify_method(method: &ImplItemFn) -> MethodKind {
    for attr in &method.attrs {
        if attr.path().is_ident("new") {
            return MethodKind::Constructor;
        }
        if attr.path().is_ident("getter") {
            let name = extract_attr_name(attr);
            return MethodKind::Getter(name);
        }
        if attr.path().is_ident("setter") {
            let name = extract_attr_name(attr);
            return MethodKind::Setter(name);
        }
        if attr.path().is_ident("staticmethod") {
            return MethodKind::Static;
        }
        if attr.path().is_ident("classmethod") {
            return MethodKind::ClassMethod;
        }
    }

    // Check if it's a magic method by name
    let name = method.sig.ident.to_string();
    if name.starts_with("__") && name.ends_with("__") && name.len() > 4 {
        let inner = name[2..name.len() - 2].to_string();
        return MethodKind::Magic(inner);
    }

    MethodKind::Regular
}

fn extract_attr_name(attr: &syn::Attribute) -> Option<String> {
    let mut name = None;
    let _ = attr.parse_nested_meta(|meta| {
        if let Some(ident) = meta.path.get_ident() {
            name = Some(ident.to_string());
        }
        Ok(())
    });
    name
}

/// Remove pyo3-specific attributes from a method.
fn strip_pyo3_attrs(method: &mut ImplItemFn) {
    method.attrs.retain(|attr| {
        !attr.path().is_ident("new")
            && !attr.path().is_ident("getter")
            && !attr.path().is_ident("setter")
            && !attr.path().is_ident("staticmethod")
            && !attr.path().is_ident("classmethod")
            && !attr.path().is_ident("pyo3")
    });
}

/// Check if a function argument is `Python<'_>` or `py: Python<'_>`.
fn is_python_arg(arg: &FnArg) -> bool {
    if let FnArg::Typed(pat_type) = arg {
        let ty_str = quote!(#pat_type.ty).to_string().replace(' ', "");
        ty_str.contains("Python")
    } else {
        false
    }
}

/// Check if return type contains PyResult
fn returns_pyresult(ret: &ReturnType) -> bool {
    match ret {
        ReturnType::Default => false,
        ReturnType::Type(_, ty) => {
            let s = quote!(#ty).to_string().replace(' ', "");
            s.contains("PyResult")
        }
    }
}

pub fn expand(_attr: TokenStream, mut input: ItemImpl) -> Result<TokenStream> {
    let struct_name = &input.self_ty;
    let mut transformed_items: Vec<TokenStream> = Vec::new();
    let mut constructor: Option<TokenStream> = None;

    for item in &mut input.items {
        let ImplItem::Fn(ref mut method) = item else {
            // Pass through non-method items unchanged
            transformed_items.push(quote! { #item });
            continue;
        };

        let kind = classify_method(method);
        strip_pyo3_attrs(method);

        match kind {
            MethodKind::Constructor => {
                // Generate a #[pyslot] slot_new that calls the user's constructor
                let fn_name = &method.sig.ident;
                let mut arg_names = Vec::new();
                let mut arg_types = Vec::new();
                let mut rp_params = Vec::new();

                for arg in &method.sig.inputs {
                    if let FnArg::Typed(pat_type) = arg {
                        if is_python_arg(arg) {
                            continue;
                        }
                        if let Pat::Ident(pi) = pat_type.pat.as_ref() {
                            let name = &pi.ident;
                            let ty = &pat_type.ty;
                            arg_names.push(quote! { #name });
                            arg_types.push(quote! { #ty });
                            rp_params.push(quote! { #name: #ty });
                        }
                    }
                }

                // Keep the original method
                let vis = &method.vis;
                let sig = &method.sig;
                let block = &method.block;

                transformed_items.push(quote! {
                    #vis #sig #block
                });

                // Generate slot_new
                constructor = Some(quote! {
                    #[pyslot]
                    fn slot_new(
                        cls: ::rustpython_vm::builtins::PyTypeRef,
                        #(#rp_params,)*
                        vm: &::rustpython_vm::VirtualMachine,
                    ) -> ::rustpython_vm::PyResult {
                        let obj = Self::#fn_name(#(#arg_names),*);
                        ::rustpython_vm::PyPayload::into_ref_with_type(obj, vm, cls)
                            .map(::std::convert::Into::into)
                    }
                });
            }

            MethodKind::Getter(name) => {
                let getter_name = name
                    .map(|n| format_ident!("{}", n))
                    .unwrap_or_else(|| method.sig.ident.clone());
                let vis = &method.vis;
                let ret = &method.sig.output;
                let block = &method.block;

                transformed_items.push(quote! {
                    #[pygetset]
                    #vis fn #getter_name(&self) #ret #block
                });
            }

            MethodKind::Setter(name) => {
                let setter_name = name
                    .map(|n| format_ident!("set_{}", n))
                    .unwrap_or_else(|| {
                        let n = method.sig.ident.to_string();
                        let stripped = n.strip_prefix("set_").unwrap_or(&n);
                        format_ident!("set_{}", stripped)
                    });
                let vis = &method.vis;
                // Get the value parameter (skip &mut self)
                let params: Vec<_> = method
                    .sig
                    .inputs
                    .iter()
                    .filter(|a| !matches!(a, FnArg::Receiver(_)))
                    .collect();
                let ret = &method.sig.output;
                let block = &method.block;

                transformed_items.push(quote! {
                    #[pygetset(setter)]
                    #vis fn #setter_name(&mut self, #(#params),*) #ret #block
                });
            }

            MethodKind::Static => {
                let vis = &method.vis;
                let sig = &method.sig;
                let block = &method.block;

                transformed_items.push(quote! {
                    #[pystaticmethod]
                    #vis #sig #block
                });
            }

            MethodKind::ClassMethod => {
                let vis = &method.vis;
                let sig = &method.sig;
                let block = &method.block;

                transformed_items.push(quote! {
                    #[pyclassmethod]
                    #vis #sig #block
                });
            }

            MethodKind::Magic(inner_name) => {
                let magic_fn_name = format_ident!("{}", inner_name);
                let vis = &method.vis;
                // Collect non-self, non-Python params
                let params: Vec<_> = method
                    .sig
                    .inputs
                    .iter()
                    .filter(|a| !matches!(a, FnArg::Receiver(_)) && !is_python_arg(a))
                    .collect();
                let ret = &method.sig.output;
                let block = &method.block;

                transformed_items.push(quote! {
                    #[pymethod(magic)]
                    #vis fn #magic_fn_name(&self, #(#params),*) #ret #block
                });
            }

            MethodKind::Regular => {
                let vis = &method.vis;
                let sig = &method.sig;
                let block = &method.block;

                transformed_items.push(quote! {
                    #[pymethod]
                    #vis #sig #block
                });
            }
        }
    }

    // Build the with() list based on what we found
    let with_constructor = if constructor.is_some() {
        quote! { with(::rustpython_vm::types::Constructor), }
    } else {
        quote! {}
    };

    Ok(quote! {
        #[::rustpython_vm::pyclass(#with_constructor)]
        impl #struct_name {
            #constructor
            #(#transformed_items)*
        }
    })
}
```

- [ ] **Step 2: Wire up the macro**

Replace the `pymethods` stub in `crates/pyo3-rustpython-derive/src/lib.rs`:

```rust
mod pymethods;

/// Adds methods, properties, and constructors to a `#[pyclass]`.
///
/// Transforms pyo3-style attributes (`#[new]`, `#[getter]`, `#[setter]`,
/// `#[staticmethod]`, `#[classmethod]`) and magic methods (`__repr__`, etc.)
/// into rustpython-derive equivalents.
#[proc_macro_attribute]
pub fn pymethods(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(item as syn::ItemImpl);
    pymethods::expand(attr.into(), input)
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}
```

- [ ] **Step 3: Commit**

```bash
git add crates/pyo3-rustpython-derive/
git commit -m "feat: implement #[pymethods] macro

Transforms pyo3-style method attributes to rustpython-derive equivalents:
- #[new] → #[pyslot] slot_new with Constructor trait
- #[getter]/#[setter] → #[pygetset]
- __magic__ methods → #[pymethod(magic)]
- #[staticmethod] → #[pystaticmethod]
- #[classmethod] → #[pyclassmethod]
- Regular methods → #[pymethod]"
```

### Task 1.4: Add class registration to `#[pymodule]`

**Files:**
- Modify: `crates/pyo3-rustpython-derive/src/pymodule.rs`
- Modify: `crates/pyo3-rustpython/src/types/module.rs`

The `#[pymodule]` needs to support `m.add_class::<Point>()` in addition to `m.add_function(...)`.

- [ ] **Step 1: Add `add_class` to Bound<'py, PyModule>**

Read `crates/pyo3-rustpython/src/types/module.rs` and add:

```rust
/// Register a `#[pyclass]` type with this module.
///
/// Makes the class available as `module.ClassName` in Python.
pub fn add_class<T>(&self) -> PyResult<()>
where
    T: ::rustpython_vm::PyPayload + ::rustpython_vm::class::PyClassImpl,
{
    let vm = self.py.vm;
    let class = T::make_class(&vm.ctx);
    let name = <T as ::rustpython_vm::class::PyClassDef>::NAME;
    let interned = vm.ctx.intern_str(name);
    from_vm_result(self.obj.set_attr(interned, class.to_owned().into(), vm))
}
```

- [ ] **Step 2: Re-export `add_class` in prelude if needed**

The `add_class` method is on `Bound<'py, PyModule>`, so it's automatically available. No prelude changes needed.

- [ ] **Step 3: Commit**

```bash
git add crates/pyo3-rustpython/src/types/module.rs
git commit -m "feat: add Bound<PyModule>::add_class for pyclass registration"
```

### Task 1.5: Update the Point example and make it compile

**Files:**
- Modify: `examples/point/src/lib.rs`
- Modify: `examples/point/src/main.rs`
- Modify: `examples/point/Cargo.toml`

- [ ] **Step 1: Update lib.rs to include module definition**

Update `examples/point/src/lib.rs` to add a module that registers the class:

```rust
use pyo3::prelude::*;

#[pyclass]
#[derive(Clone)]
pub struct Point {
    #[pyo3(get, set)]
    pub x: f64,
    #[pyo3(get, set)]
    pub y: f64,
}

#[pymethods]
impl Point {
    #[new]
    fn new(x: f64, y: f64) -> Self {
        Point { x, y }
    }

    fn distance(&self) -> f64 {
        (self.x * self.x + self.y * self.y).sqrt()
    }

    fn translate(&self, dx: f64, dy: f64) -> Point {
        Point {
            x: self.x + dx,
            y: self.y + dy,
        }
    }

    fn __repr__(&self) -> String {
        format!("Point({}, {})", self.x, self.y)
    }

    fn __str__(&self) -> String {
        format!("({}, {})", self.x, self.y)
    }
}

#[pymodule]
fn point(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Point>()?;
    Ok(())
}
```

- [ ] **Step 2: Update main.rs**

Update `examples/point/src/main.rs`:

```rust
use rustpython_vm::InterpreterBuilder;

mod extension {
    include!("lib.rs");
}

fn main() {
    let builder = InterpreterBuilder::new();
    let module_def = extension::point_module_def(&builder.ctx);
    let interp = builder.add_native_module(module_def).build();

    let exit_code = interp.run(|vm| {
        vm.run_block_expr(
            vm.new_scope_with_builtins(),
            r#"
from point import Point

p = Point(3.0, 4.0)
assert repr(p) == "Point(3.0, 4.0)", f"repr: {repr(p)}"
assert str(p) == "(3.0, 4.0)", f"str: {str(p)}"
assert p.x == 3.0
assert p.y == 4.0
assert p.distance() == 5.0, f"distance: {p.distance()}"

p.x = 1.0
p.y = 1.0
assert p.x == 1.0

p2 = p.translate(2.0, 3.0)
assert p2.x == 3.0
assert p2.y == 4.0

print("All point tests passed!")
"#,
        )
        .map(|_| ())
    });

    if exit_code != 0 {
        std::process::exit(exit_code as i32);
    }
}
```

- [ ] **Step 3: Add rustpython-derive dependency to pyo3-rustpython crate**

The `#[pyclass]` expansion references `::rustpython_vm::pyclass` which requires `rustpython-derive` to be available. Check `crates/pyo3-rustpython/Cargo.toml` has:

```toml
[dependencies]
rustpython-vm = { workspace = true }
rustpython-derive = { workspace = true }
```

Also add `rustpython-derive` to workspace deps in root `Cargo.toml` if not already there.

- [ ] **Step 4: Try to compile the point example**

```bash
cargo build --bin point-interp 2>&1
```

This is the moment of truth. There will likely be compilation errors from the generated code not quite matching rustpython-derive's expectations. Fix them iteratively:

Common issues to expect:
- `PyPayload` derive vs manual impl conflicts
- `StaticType` not being implemented (may need `#[derive(rustpython_derive::PyPayload)]` instead of manual impl)
- `slot_new` signature not matching `Constructor` trait exactly
- `Clone` needed on the struct for return-by-value methods

Debug by expanding the macros:

```bash
cargo expand --bin point-interp 2>&1 | head -200
```

- [ ] **Step 5: Fix compilation errors and iterate**

This step is inherently iterative. The generated code needs to match what rustpython-derive expects exactly. Use `cargo expand` to see the actual expansion and adjust `pyclass.rs` and `pymethods.rs` accordingly.

- [ ] **Step 6: Run the point example**

```bash
cargo run --bin point-interp
# Expected: "All point tests passed!"
```

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "feat: Point example compiles and runs with #[pyclass]/#[pymethods]

First working example of a pyo3-style class definition running in
RustPython via the pyo3-rustpython compatibility layer."
```

### Task 1.6: Verify hello example still works

**Files:** None (regression check)

- [ ] **Step 1: Run the hello example**

```bash
cargo run --bin hello-interp
# Expected: "hello, world!"
```

- [ ] **Step 2: Run both examples**

```bash
cargo run --bin hello-interp && cargo run --bin point-interp
# Expected: both pass
```

- [ ] **Step 3: Commit if any fixes were needed**

---

## Phase 2: Runtime Layer (outline — flesh out when Phase 1 is done)

**Goal:** Implement conversion traits, Python type wrappers, PyAnyMethods, and exception hierarchy.

### Task 2.1: Conversion traits — `FromPyObject` and `IntoPyObject`
- Define traits in `crates/pyo3-rustpython/src/conversion.rs`
- Implement for primitives: `i8`–`i64`, `u8`–`u64`, `f32`/`f64`, `bool`, `String`, `&str`
- Implement for containers: `Vec<T>`, `Option<T>`, `HashMap<K,V>`, `(T1, T2, ...)`
- Each delegates to rustpython-vm's `TryFromObject` / `ToPyObject`

### Task 2.2: Python type wrappers
- `PyDict` — `crates/pyo3-rustpython/src/types/dict.rs`
- `PyList` — `crates/pyo3-rustpython/src/types/list.rs`
- `PyTuple` — `crates/pyo3-rustpython/src/types/tuple.rs`
- `PyString` — `crates/pyo3-rustpython/src/types/string.rs`
- `PyBool`, `PyFloat`, `PyInt` — `crates/pyo3-rustpython/src/types/primitives.rs`
- `PyNone` — `crates/pyo3-rustpython/src/types/none.rs`
- `PyBytes` — `crates/pyo3-rustpython/src/types/bytes.rs`
- `PySet`, `PyFrozenSet` — `crates/pyo3-rustpython/src/types/set.rs`
- `PyType` — `crates/pyo3-rustpython/src/types/typeobj.rs`
- `PyIterator` — `crates/pyo3-rustpython/src/types/iterator.rs`
- Each with `Py*Methods` trait

### Task 2.3: `PyAnyMethods` trait
- Implement on `Bound<'py, PyAny>`
- `getattr`, `setattr`, `delattr`, `hasattr`
- `call`, `call0`, `call1`, `call_method`, `call_method0`, `call_method1`
- `extract`, `downcast`, `downcast_into`, `is_instance_of`, `get_type`
- `repr`, `str_`, `hash`, `len`, `is_truthy`, `is_none`
- Comparison methods
- Each delegates to `VirtualMachine` / `PyObjectRef` methods

### Task 2.4: Exception hierarchy
- Define `pyo3::exceptions::*` types via `impl_exception!` macro
- Map ~40 exception types to `vm.ctx.exceptions.*`
- Implement `create_exception!` and `import_exception!` macros
- Extend `PyErr` with `new::<T, A>()`, `matches()`, `value()`, `is_instance_of::<T>()`

### Task 2.5: GIL stubs
- `Python::with_gil` backed by thread-local VM (already scaffolded in Phase 0)
- `Python::allow_threads` as no-op
- `VmGuard` integration into module exec and method call entry points
- `py.None()`, `py.True()`, `py.False()` helpers

### Task 2.6: `#[derive(FromPyObject)]`
- Implement derive macro for structs and enums
- Field extraction via `getattr` + `extract`
- Enum variant matching via `downcast` attempts

---

## Phase 3: orjson (outline)

**Goal:** Get orjson compiling and passing tests against the RustPython backend.

### Task 3.1: Vendor orjson
- Clone orjson source, point `pyo3` dep at our crate
- Catalog which pyo3 features orjson uses

### Task 3.2: Compile-error-driven implementation
- Build orjson, fix errors iteratively
- Track progress: "X of Y source files compile"

### Task 3.3: Runtime testing
- Run orjson's test suite inside RustPython
- Track pass/fail/skip

---

## Phase 4: pydantic-core (outline)

**Goal:** Get pydantic-core compiling against the RustPython backend.

### Task 4.1: Vendor pydantic-core
- Clone pydantic-core source
- Catalog pyo3 API surface used

### Task 4.2: Compile-error-driven implementation
- This will be the longest phase — pydantic-core is large
- Likely needs: advanced `#[pyclass]` options, class inheritance, complex `FromPyObject` derives, `#[pyo3(signature)]` support, many more type wrappers

### Task 4.3: Iterative fixes
- Track compilation progress file by file

---

## Phase 5: Pydantic Python tests (outline)

**Goal:** Run pydantic's Python test suite inside RustPython.

### Task 5.1: Set up test runner
- Similar to numpy-rust's upstream test runner
- Run pydantic tests with RustPython + our native module

### Task 5.2: Track and fix failures
- Categorize: pyo3-rustpython issues vs RustPython limitations vs CPython-only features
- Track pass/fail/skip counts

---

## Phase 6: Sister project migration (outline)

**Goal:** Migrate numpy-rust, pandas-rust, etc. to use pyo3 with `runtime-rustpython`.

No rush — do after Phase 5 is solid.
