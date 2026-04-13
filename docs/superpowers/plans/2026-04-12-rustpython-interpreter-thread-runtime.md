# RustPython Interpreter-Thread Runtime Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the current thread-unsafe RustPython runtime model in `third_party/pyo3-fork` with a dedicated interpreter thread and synchronous request dispatch so worker-thread PyO3 usage no longer crashes.

**Current status:** Blocked by upstream RustPython spawned-thread import bug: [RustPython/RustPython#7586](https://github.com/RustPython/RustPython/issues/7586). The reproducer is retained as an ignored expected-failure test until RustPython's import/threading path is fixed or a credible upstream patch is available.

**Architecture:** The RustPython backend will own one process-global interpreter thread. PyO3 caller threads will never execute RustPython VM work directly; instead, backend helpers will synchronously dispatch closures onto the interpreter thread and receive owned results back. CPython-family backend behavior stays unchanged, while `runtime-rustpython` becomes explicitly single-owner and thread-safe at the backend boundary.

**Tech Stack:** Rust, `std::sync::mpsc`, RustPython `InterpreterBuilder`, PyO3 fork (`third_party/pyo3-fork`), Cargo integration and lib tests

---

## File Structure

**Primary runtime files**

- Modify: `third_party/pyo3-fork/pyo3-ffi/src/rustpython_runtime.rs`
  - Replace the current process-global `Interpreter` + arbitrary-thread `enter(...)` model with a runtime-thread owner and synchronous dispatch API.
- Modify: `third_party/pyo3-fork/pyo3-ffi/src/pystate_rustpython.rs`
  - Keep `PyGILState_Ensure` / `Release` as PyO3-facing attachment bookkeeping, but route runtime work through the new runtime.
- Modify: `third_party/pyo3-fork/pyo3-ffi/src/pylifecycle_rustpython.rs`
  - Keep initialization/finalization semantics consistent with the new runtime-thread model.
- Modify: `third_party/pyo3-fork/pyo3-ffi/src/import_rustpython.rs`
  - Port import helpers off direct `with_vm(...)` usage assumptions and onto the dispatch path.

**Tests / repros**

- Create: `third_party/pyo3-fork/tests/test_rustpython_runtime.rs`
  - Integration tests proving that worker-thread `Python::attach(... py.import("array") ...)` is safe under `runtime-rustpython`.
- Modify: `third_party/pyo3-fork/src/buffer.rs`
  - Remove temporary debug prints once `test_array_buffer` passes again.

**Cleanup targets**

- Modify: `third_party/pyo3-fork/src/internal/state.rs`
  - Remove temporary RustPython attach diagnostics after the runtime redesign is verified.
- Modify: `third_party/pyo3-fork/src/types/module.rs`
  - Remove temporary import diagnostics after verification.

---

### Task 1: Add a Worker-Thread Reproducer Test

**Files:**
- Create: `third_party/pyo3-fork/tests/test_rustpython_runtime.rs`
- Test: `third_party/pyo3-fork/tests/test_rustpython_runtime.rs`

- [ ] **Step 1: Write the failing integration test**

```rust
#![cfg(PyRustPython)]

use pyo3::prelude::*;

#[test]
fn worker_thread_can_import_array() {
    let handle = std::thread::spawn(|| {
        Python::attach(|py| {
            let module = py.import("array");
            assert!(module.is_ok(), "array import failed: {module:?}");
        });
    });

    handle.join().expect("worker thread panicked");
}
```

- [ ] **Step 2: Run the test to verify it fails with the current runtime**

Run:

```bash
cargo test --manifest-path third_party/pyo3-fork/Cargo.toml --no-default-features --features macros,runtime-rustpython -p pyo3 --test test_rustpython_runtime worker_thread_can_import_array -- --exact --nocapture
```

Expected:

- FAIL with `signal: 4, SIGILL: illegal instruction`

- [ ] **Step 3: Commit the failing test**

```bash
git -C third_party/pyo3-fork add tests/test_rustpython_runtime.rs
git -C third_party/pyo3-fork commit -m "test: add RustPython worker-thread import reproducer"
```

**Checkpoint note:** This step is complete historically, but the test is now intentionally `#[ignore]` with a pointer to `RustPython/RustPython#7586` so the suite documents the blocker without keeping the fork red.

---

### Task 2: Replace the Runtime With a Dedicated Interpreter Thread

**Files:**
- Modify: `third_party/pyo3-fork/pyo3-ffi/src/rustpython_runtime.rs`
- Test: `third_party/pyo3-fork/tests/test_rustpython_runtime.rs`

- [ ] **Step 1: Replace the current global interpreter storage with a runtime-thread handle**

Use this shape in `rustpython_runtime.rs`:

```rust
use rustpython::InterpreterBuilderExt;
use rustpython_vm::{InterpreterBuilder, VirtualMachine};
use std::cell::Cell;
use std::sync::{mpsc, OnceLock};

thread_local! {
    static ATTACH_COUNT: Cell<u32> = const { Cell::new(0) };
    static ON_RUNTIME_THREAD: Cell<bool> = const { Cell::new(false) };
}

struct RuntimeHandle {
    tx: mpsc::Sender<RuntimeRequest>,
    thread_id: std::thread::ThreadId,
}

enum RuntimeRequest {
    Call(Box<dyn FnOnce(&VirtualMachine) + Send + 'static>),
}

static RUNTIME: OnceLock<RuntimeHandle> = OnceLock::new();
```

- [ ] **Step 2: Implement runtime thread startup and blocking dispatch**

Add the runtime thread shell:

```rust
fn runtime() -> &'static RuntimeHandle {
    RUNTIME.get_or_init(|| {
        let (tx, rx) = mpsc::channel::<RuntimeRequest>();
        let (ready_tx, ready_rx) = mpsc::channel();

        std::thread::spawn(move || {
            ON_RUNTIME_THREAD.with(|flag| flag.set(true));
            let interpreter = InterpreterBuilder::new().init_stdlib().interpreter();
            let thread_id = std::thread::current().id();
            ready_tx.send(thread_id).unwrap();

            interpreter.enter(|vm| {
                while let Ok(request) = rx.recv() {
                    match request {
                        RuntimeRequest::Call(f) => f(vm),
                    }
                }
            });
        });

        let thread_id = ready_rx.recv().unwrap();
        RuntimeHandle { tx, thread_id }
    })
}

pub(crate) fn dispatch<R: Send + 'static>(f: impl FnOnce(&VirtualMachine) -> R + Send + 'static) -> R {
    if ON_RUNTIME_THREAD.with(|flag| flag.get()) {
        panic!("dispatch called inline without runtime-thread VM plumbing");
    }

    let (result_tx, result_rx) = mpsc::sync_channel(1);
    runtime().tx.send(RuntimeRequest::Call(Box::new(move |vm| {
        let result = f(vm);
        result_tx.send(result).unwrap();
    }))).unwrap();
    result_rx.recv().unwrap()
}
```

- [ ] **Step 3: Rebuild `with_vm` on top of dispatch**

Keep the public helper name for now, but make it thread-safe:

```rust
pub(crate) fn with_vm<R: Send + 'static>(f: impl FnOnce(&VirtualMachine) -> R + Send + 'static) -> R {
    assert!(is_attached(), "RustPython FFI used outside an attached interpreter context");
    dispatch(f)
}
```

- [ ] **Step 4: Update initialization helpers to use the runtime thread**

Replace the current inline interpreter helpers:

```rust
pub(crate) fn initialize() {
    let _ = runtime();
}

pub(crate) fn is_initialized() -> bool {
    RUNTIME.get().is_some()
}

pub(crate) fn finalize() {
    // Still process-lifetime for now.
}
```

- [ ] **Step 5: Run the reproducer test**

Run:

```bash
cargo test --manifest-path third_party/pyo3-fork/Cargo.toml --no-default-features --features macros,runtime-rustpython -p pyo3 --test test_rustpython_runtime worker_thread_can_import_array -- --exact --nocapture
```

Expected:

- PASS

- [ ] **Step 6: Commit the runtime-thread shell**

```bash
git -C third_party/pyo3-fork add pyo3-ffi/src/rustpython_runtime.rs tests/test_rustpython_runtime.rs
git -C third_party/pyo3-fork commit -m "refactor: add RustPython interpreter-thread runtime shell"
```

---

### Task 3: Rework RustPython Attach Semantics Around Dispatch

**Files:**
- Modify: `third_party/pyo3-fork/pyo3-ffi/src/pystate_rustpython.rs`
- Modify: `third_party/pyo3-fork/pyo3-ffi/src/pylifecycle_rustpython.rs`
- Test: `third_party/pyo3-fork/tests/test_rustpython_runtime.rs`

- [ ] **Step 1: Keep `PyGILState_Ensure` as PyO3-side bookkeeping only**

Update `pystate_rustpython.rs` to preserve the existing public contract while removing assumptions that the caller thread owns the interpreter:

```rust
pub unsafe extern "C" fn PyGILState_Ensure() -> PyGILState_STATE {
    match rustpython_runtime::ensure_attached() {
        rustpython_runtime::AttachState::Assumed => PyGILState_STATE::PyGILState_LOCKED,
        rustpython_runtime::AttachState::Ensured => PyGILState_STATE::PyGILState_UNLOCKED,
    }
}
```

The semantic change is in `ensure_attached()` itself: it must no longer call `interpreter().enter(...)` from the caller thread.

- [ ] **Step 2: Make attach initialization runtime-thread-safe**

In `rustpython_runtime.rs`, use:

```rust
pub(crate) fn ensure_attached() -> AttachState {
    let already_attached = ATTACH_COUNT.with(|count| {
        let current = count.get();
        count.set(current + 1);
        current > 0
    });

    if already_attached {
        AttachState::Assumed
    } else {
        initialize();
        AttachState::Ensured
    }
}
```

- [ ] **Step 3: Keep lifecycle functions consistent with the new runtime**

In `pylifecycle_rustpython.rs`, preserve:

```rust
pub unsafe fn Py_Initialize() {
    rustpython_runtime::initialize();
}

pub unsafe fn Py_InitializeEx(_initsigs: c_int) {
    rustpython_runtime::initialize();
}

pub unsafe fn Py_IsInitialized() -> c_int {
    rustpython_runtime::is_initialized().into()
}
```

Do not add teardown complexity in this task.

- [ ] **Step 4: Run the worker-thread reproducer again**

Run:

```bash
cargo test --manifest-path third_party/pyo3-fork/Cargo.toml --no-default-features --features macros,runtime-rustpython -p pyo3 --test test_rustpython_runtime -- --nocapture
```

Expected:

- PASS

- [ ] **Step 5: Commit the attach/lifecycle update**

```bash
git -C third_party/pyo3-fork add pyo3-ffi/src/pystate_rustpython.rs pyo3-ffi/src/pylifecycle_rustpython.rs pyo3-ffi/src/rustpython_runtime.rs
git -C third_party/pyo3-fork commit -m "refactor: route RustPython attach through runtime dispatch"
```

---

### Task 4: Port Import Helpers to the Runtime Thread

**Files:**
- Modify: `third_party/pyo3-fork/pyo3-ffi/src/import_rustpython.rs`
- Modify: `third_party/pyo3-fork/src/types/module.rs`
- Test: `third_party/pyo3-fork/src/buffer.rs`

- [ ] **Step 1: Remove direct-thread assumptions from import helpers**

Use `rustpython_runtime::with_vm` as a pure dispatch wrapper only:

```rust
pub unsafe fn PyImport_ImportModule(name: *const c_char) -> *mut PyObject {
    let Some(name) = cstr_to_string(name) else {
        return std::ptr::null_mut();
    };
    rustpython_runtime::with_vm(move |vm| {
        match import_module_by_name(vm, &name, 0) {
            Ok(module) => pyobject_ref_to_ptr(module),
            Err(exc) => {
                set_vm_exception(exc);
                std::ptr::null_mut()
            }
        }
    })
}
```

Keep the same pattern for:

- `PyImport_GetModuleDict`
- `PyImport_GetModule`
- `PyImport_AddModule`
- `PyImport_ImportModuleLevel`

- [ ] **Step 2: Remove temporary diagnostics from `PyModule::import`**

Restore the RustPython branch in `src/types/module.rs` to the minimal import path:

```rust
#[cfg(PyRustPython)]
unsafe {
    let name = name.into_any().into_bound();
    let name: Bound<'py, PyString> = name.cast_into_unchecked();
    let name = name.to_cow()?;
    let c_name = std::ffi::CString::new(name.as_ref())
        .map_err(|_| PyErr::new::<exceptions::PyValueError, _>("module name contains NUL byte"))?;
    ffi::PyImport_ImportModule(c_name.as_ptr())
        .assume_owned_or_err(py)
        .cast_into_unchecked()
}
```

- [ ] **Step 3: Re-run the original failing lib test**

Run:

```bash
cargo test --manifest-path third_party/pyo3-fork/Cargo.toml --no-default-features --features macros,runtime-rustpython -p pyo3 --lib buffer::tests::test_array_buffer -- --exact --test-threads=1 --nocapture
```

Expected:

- PASS, or at minimum advance past `py.import("array")` into actual buffer assertions

- [ ] **Step 4: Commit the import-thread migration**

```bash
git -C third_party/pyo3-fork add pyo3-ffi/src/import_rustpython.rs src/types/module.rs
git -C third_party/pyo3-fork commit -m "refactor: dispatch RustPython imports on runtime thread"
```

---

### Task 5: Remove Invalid Fast Paths and Debug Noise

**Files:**
- Modify: `third_party/pyo3-fork/pyo3-ffi/src/rustpython_runtime.rs`
- Modify: `third_party/pyo3-fork/src/internal/state.rs`
- Modify: `third_party/pyo3-fork/src/buffer.rs`
- Modify: `third_party/pyo3-fork/src/types/module.rs`

- [ ] **Step 1: Delete thread-local VM shortcuts**

Remove the old runtime-thread leakage path:

```rust
thread_local! {
    static CURRENT_VM: Cell<*const VirtualMachine> = const { Cell::new(std::ptr::null()) };
}
```

and any code which tries to reuse a raw `*const VirtualMachine` on caller threads.

- [ ] **Step 2: Remove temporary `eprintln!` diagnostics**

Delete RustPython-specific debug prints from:

- `pyo3-ffi/src/rustpython_runtime.rs`
- `src/internal/state.rs`
- `src/buffer.rs`
- `src/types/module.rs`

The resulting code should keep the new runtime semantics without debug noise.

- [ ] **Step 3: Run the focused regression set**

Run:

```bash
cargo test --manifest-path third_party/pyo3-fork/Cargo.toml --no-default-features --features macros,runtime-rustpython -p pyo3 --test test_rustpython_runtime -- --nocapture
cargo test --manifest-path third_party/pyo3-fork/Cargo.toml --no-default-features --features macros,runtime-rustpython -p pyo3 --lib buffer::tests::test_array_buffer -- --exact --test-threads=1
cargo test --manifest-path third_party/pyo3-fork/Cargo.toml --no-default-features --features macros,runtime-rustpython -p pyo3 --test test_inheritance -- --test-threads=1
cargo test --manifest-path third_party/pyo3-fork/Cargo.toml --no-default-features --features macros,runtime-rustpython -p pyo3 --test test_pyfunction -- --test-threads=1
```

Expected:

- all commands PASS

- [ ] **Step 4: Commit cleanup**

```bash
git -C third_party/pyo3-fork add pyo3-ffi/src/rustpython_runtime.rs src/internal/state.rs src/buffer.rs src/types/module.rs
git -C third_party/pyo3-fork commit -m "chore: remove RustPython runtime diagnostics"
```

---

### Task 6: Full RustPython Backend Verification Checkpoint

**Files:**
- Modify: `third_party/pyo3-fork` submodule pointer in the parent repo
- Test: `third_party/pyo3-fork` runtime-rustpython suite

- [ ] **Step 1: Run the checkpoint suite**

Run:

```bash
cargo test --manifest-path third_party/pyo3-fork/Cargo.toml --no-default-features --features macros,runtime-rustpython -p pyo3 --test test_module -- --test-threads=1
cargo test --manifest-path third_party/pyo3-fork/Cargo.toml --no-default-features --features macros,runtime-rustpython -p pyo3 --test test_methods -- --test-threads=1
cargo test --manifest-path third_party/pyo3-fork/Cargo.toml --no-default-features --features macros,runtime-rustpython -p pyo3 --test test_sequence -- --test-threads=1
cargo test --manifest-path third_party/pyo3-fork/Cargo.toml --no-default-features --features macros,runtime-rustpython -p pyo3 --test test_inheritance -- --test-threads=1
cargo test --manifest-path third_party/pyo3-fork/Cargo.toml --no-default-features --features macros,runtime-rustpython -p pyo3 --test test_pyfunction -- --test-threads=1
```

Expected:

- all commands PASS

- [ ] **Step 2: Update the parent repo to the verified fork commit**

```bash
git add third_party/pyo3-fork
git commit -m "chore: update PyO3 fork after RustPython runtime-thread redesign"
```

- [ ] **Step 3: Record the checkpoint in notes if needed**

Append a short note to the active tracking docs describing:

- worker-thread crash root cause
- interpreter-thread runtime solution
- tests used to verify the fix

Suggested commit:

```bash
git commit -m "docs: record RustPython interpreter-thread runtime checkpoint"
```

---

## Self-Review

Spec coverage:

- dedicated interpreter thread: covered in Task 2
- synchronous request dispatch: covered in Task 2
- attach semantics update: covered in Task 3
- import path validation: covered in Task 4
- removal of invalid fast paths: covered in Task 5
- full regression verification: covered in Task 6

Placeholder scan:

- no `TBD` / `TODO`
- each code-changing step includes concrete code
- each verification step includes exact commands and expected outcomes

Type consistency:

- runtime API names used consistently: `runtime`, `dispatch`, `with_vm`, `AttachState`
- test target names are consistent with proposed files and existing suite names
