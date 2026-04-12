# RustPython Interpreter-Thread Runtime Design

## Context

The current `runtime-rustpython` backend in `third_party/pyo3-fork` uses a process-global RustPython `Interpreter` and calls into it from whichever host thread reaches PyO3.

That model is incorrect.

Observed behavior:

- `Python::attach(... py.import("array") ...)` works in a standalone binary on the process main thread.
- The same path crashes with `SIGILL` in a Rust integration test.
- The same path also crashes in a standalone binary when moved onto a spawned thread.
- Initializing on the main thread first does not fix the spawned-thread crash.

This shows the failure is not `array`-specific and not a generic import bug. It is a thread-ownership bug in the current RustPython backend model.

## Goal

Define a correct RustPython runtime model for the PyO3 fork:

- RustPython interpreter state is owned by one dedicated thread.
- All RustPython backend operations execute on that thread.
- PyO3 frontend code stays backend-agnostic.
- CPython-family backend behavior is unchanged.

## Non-Goals

- Do not preserve the current “enter RustPython from arbitrary caller threads” model.
- Do not optimize for maximum throughput yet.
- Do not add async dispatch in the first version.
- Do not redesign CPython-family backend threading semantics.

## Recommended Approach

Use a dedicated interpreter thread plus synchronous request/response dispatch.

### Why this approach

- It matches the root cause we proved: RustPython execution is thread-affine in practice for this integration.
- It keeps the frontend/backend split clean by making thread ownership a backend concern.
- It gives deterministic semantics for `runtime-rustpython` while we continue the larger PyO3 split.
- It is upstream-defensible because it is explicit, correct, and isolated to the RustPython backend.

## Runtime Model

### Global Runtime Handle

The RustPython backend owns a single global runtime handle:

- lazily initialized
- process-global
- permanent for process lifetime

The handle contains:

- interpreter-thread startup coordination
- a request sender
- runtime state needed for shutdown/finalization bookkeeping if that is added later

The runtime thread constructs and owns the RustPython `Interpreter`.

### Interpreter Thread

One dedicated thread:

- creates `InterpreterBuilder::new().init_stdlib().interpreter()`
- enters the interpreter as needed
- receives backend requests
- executes each request fully on the interpreter thread
- sends results back to the caller

The interpreter object never crosses thread boundaries.

### Request Dispatch

All RustPython backend operations become synchronous RPC onto the interpreter thread.

Initial request categories:

- object operations
- attribute get/set/del
- imports
- calls
- exception indicator operations
- type lookup / type creation support
- module creation / module dict access
- buffer / sequence / mapping helpers as needed

The first implementation can use:

- `std::sync::mpsc` or equivalent standard blocking channels
- one-shot response channels per request

Correctness matters more than dispatch efficiency in the first version.

## API Shape

### Replace Current `with_vm`

Today the backend exposes a `with_vm(|vm| ...)` style helper that assumes the current caller thread can re-enter the interpreter safely.

That helper should be replaced with a dispatch API that makes thread ownership explicit.

Recommended shape:

- `runtime::dispatch(f)` where `f` runs on the interpreter thread
- no direct `&VirtualMachine` escape to caller threads

Backend helpers must return:

- plain Rust values
- owned RustPython objects converted immediately to FFI-safe pointers/handles
- or backend-local opaque IDs if needed

They must not return borrowed VM references across the thread boundary.

### Attachment Semantics

For RustPython, PyO3 attachment no longer means “this caller thread is executing inside the interpreter.”

Instead it means:

- the caller has an active PyO3-side access session
- backend operations performed during that session are dispatched onto the interpreter thread

So the RustPython backend’s attach state becomes a PyO3 bookkeeping concept, not interpreter ownership.

### Exception State

The current backend already stores a RustPython-side exception indicator.

That indicator should remain backend-local, but all reads/writes to the underlying interpreter state must happen on the interpreter thread.

Cross-thread behavior:

- caller submits request
- backend thread executes RustPython work
- backend thread captures result or exception
- caller receives translated success/failure result

## Boundary Rules

### Must stay on interpreter thread

- `Interpreter`
- `VirtualMachine`
- RustPython object graph mutation
- imports
- Python calls
- type creation/finalization
- exception creation/fetch/restore against live VM state

### May cross back to caller thread

- FFI-compatible opaque pointers if their lifetime model is already backend-managed
- copied primitive values
- copied strings / byte buffers
- backend-translated error payloads

### Must not cross thread boundary

- borrowed `&VirtualMachine`
- borrowed RustPython payload references
- closures that capture non-sendable interpreter internals and run on caller threads

## Migration Plan

### Phase 1: Introduce runtime thread shell

- add `rustpython_runtime` interpreter thread startup
- add request loop and synchronous dispatch primitive
- keep current public backend entry points but route them through dispatch

Success criterion:

- simple import path like `PyModule::import("array")` works both on main thread and worker thread

### Phase 2: Port core backend helpers

Move current inline interpreter calls onto dispatch:

- import helpers
- object attribute helpers
- call helpers
- exception helpers
- type lookup helpers

Success criterion:

- reproducer that currently crashes on worker thread no longer crashes
- `buffer::tests::test_array_buffer` reaches actual buffer logic

### Phase 3: Remove invalid fast paths

- delete current caller-thread interpreter re-entry assumptions
- delete any `CURRENT_VM` / thread-local shortcuts that bypass dispatch
- tighten helper signatures so backend code cannot accidentally leak VM borrows

Success criterion:

- no backend entry point executes RustPython VM work outside the interpreter thread

### Phase 4: Re-verify suite

Re-run:

- the standalone spawned-thread reproducer
- external integration test reproducer
- `pyo3` lib test `buffer::tests::test_array_buffer`
- previously green `test_module`
- previously green `test_pyfunction`
- previously green `test_inheritance`
- previously green `test_methods`
- previously green `test_sequence`

## Alternatives Considered

### 1. Keep current model and patch specific crashes

Rejected.

This is what we have been doing. The reproducer evidence shows the model itself is wrong.

### 2. Restrict RustPython backend to owner-thread-only usage temporarily

Rejected as the primary design.

This might be useful as a debugging aid, but it is not a credible upstream backend architecture.

### 3. Async interpreter thread

Deferred.

Possible later, but unnecessary complexity for the first correct implementation.

## Risks

### Throughput and latency

Every backend call becomes a synchronous cross-thread hop.

Accepted for first implementation.

### FFI pointer semantics

Some current code assumes CPython-like pointer semantics while also assuming same-thread execution.

This design does not solve all FFI-lifetime problems by itself. It only fixes execution ownership.

### Re-entrancy

If backend callbacks triggered on the interpreter thread synchronously try to dispatch back onto the interpreter thread, deadlock is possible.

Mitigation:

- runtime dispatch must detect “already on interpreter thread”
- in that case, execute inline

This is the only safe fast path.

## Success Criteria

The design is successful when:

- RustPython backend no longer crashes merely because PyO3 code runs on a worker thread
- the current worker-thread `array` import reproducer passes
- `buffer::tests::test_array_buffer` advances past import and into real buffer behavior
- CPython-family backend behavior is unchanged
- the runtime ownership rule is explicit and isolated to the RustPython backend

