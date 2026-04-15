# PyO3 RustPython Port Completion Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Finish the late-stage RustPython backend port in `third_party/pyo3-fork`, keeping CPython-family behavior intact, driving unchanged upstream PyO3 tests to green or explicit upstream-issue xfail status, and leaving the fork in a checkpointable state.

**Architecture:** PyO3 remains the frontend and preserves semantic method / slot metadata in a backend-neutral way. The RustPython backend owns runtime realization, FFI shims, lifecycle behavior, and shutdown semantics. Remaining work is now compatibility closure: fix true backend contract gaps locally, classify real RustPython bugs upstream, and keep the unchanged upstream PyO3 test suite as the acceptance harness.

**Tech Stack:** Rust, Cargo, PyO3 fork (`third_party/pyo3-fork`), RustPython fork (`third_party/rustpython-fork`), unchanged upstream PyO3 tests, Git submodule-style fork workflow.

---

## Current Status Snapshot

**Known green under `runtime-rustpython`:**
- `pyo3 --lib` is green with explicit upstream-blocker ignores.
- Major integration targets are green: `test_inheritance`, `test_methods`, `test_pyfunction` (except documented upstream blockers), `test_buffer`, `test_buffer_protocol`, `test_gc`, `test_getter_setter`, `test_proto_methods` (except documented upstream blockers), `test_class_*`, `test_mapping`, `test_module`, `test_enum`, `test_datetime*`, `test_compile_error`.

**Known upstream RustPython blockers already tracked:**
- `RustPython/RustPython#7586` worker-thread import recursion
- `RustPython/RustPython#7587` embedded stdlib import recursion (`re`, `warnings`, `collections`, `asyncio`, related paths)
- `RustPython/RustPython#7589` weakref semantics, partially improved by merged `#7590`

**Current local dirty frontier at plan creation:**
- `third_party/pyo3-fork/pyo3-ffi/src/abstract_rustpython.rs`
- `third_party/pyo3-fork/pyo3-ffi/src/object_rustpython.rs`
- `third_party/pyo3-fork/pyo3-ffi/src/pybuffer_rustpython.rs`
- `third_party/pyo3-fork/pyo3-ffi/src/rustpython_runtime.rs`
- `third_party/pyo3-fork/src/interpreter_lifecycle.rs`
- `third_party/pyo3-fork/tests/test_proto_methods.rs`

## Files and Responsibilities

- `third_party/pyo3-fork/pyo3-ffi/src/object_rustpython.rs`
  - RustPython heap type metadata, slot recording, slot finalization, object protocol wrappers.
- `third_party/pyo3-fork/pyo3-ffi/src/abstract_rustpython.rs`
  - Generic object / mapping / sequence protocol FFI behavior for RustPython.
- `third_party/pyo3-fork/pyo3-ffi/src/pybuffer_rustpython.rs`
  - RustPython buffer API emulation, `Py_buffer` lifecycle, release / teardown semantics.
- `third_party/pyo3-fork/pyo3-ffi/src/rustpython_runtime.rs`
  - Embedded runtime lifecycle, attach contract, logical init/finalize state, runtime dispatch.
- `third_party/pyo3-fork/src/interpreter_lifecycle.rs`
  - PyO3-facing embedded interpreter lifecycle integration.
- `third_party/pyo3-fork/tests/*.rs`
  - Unchanged upstream integration targets, plus explicit RustPython-specific xfails when justified.
- `docs/upstream-issues/*.md`
  - Exact upstream RustPython blocker notes with repro paths and scope.

## Verification Gates

These are the mandatory gates to reuse throughout execution:

- Narrow target:
  - `cargo test --manifest-path third_party/pyo3-fork/Cargo.toml --no-default-features --features macros,runtime-rustpython -p pyo3 --test <target> -- --test-threads=1`
- Full lib gate:
  - `cargo test --manifest-path third_party/pyo3-fork/Cargo.toml --no-default-features --features macros,runtime-rustpython -p pyo3 --lib -- --test-threads=1 --format=terse`
- Full integration gate:
  - `cargo test --manifest-path third_party/pyo3-fork/Cargo.toml --no-default-features --features macros,runtime-rustpython -p pyo3 --tests -- --test-threads=1 --format=terse`
- CPython-family regression smoke:
  - `cargo check --manifest-path third_party/pyo3-fork/Cargo.toml -p pyo3`

## Task 1: Stabilize Current Dirty Baseline

**Files:**
- Modify: `third_party/pyo3-fork/pyo3-ffi/src/object_rustpython.rs`
- Modify: `third_party/pyo3-fork/pyo3-ffi/src/abstract_rustpython.rs`
- Modify: `third_party/pyo3-fork/pyo3-ffi/src/pybuffer_rustpython.rs`
- Modify: `third_party/pyo3-fork/pyo3-ffi/src/rustpython_runtime.rs`
- Modify: `third_party/pyo3-fork/src/interpreter_lifecycle.rs`
- Modify: `third_party/pyo3-fork/tests/test_proto_methods.rs`
- Modify: `docs/upstream-issues/rustpython-main-thread-re-import-recursion.md`

- [ ] Verify narrow targets for the current fixes:
  - `test_proto_methods`
  - `test_pybuffer_drop_without_interpreter`
- [ ] Re-run the full `--tests` sweep to confirm the baseline moved forward rather than regressed.
- [ ] Commit this checkpoint with one fork commit and one parent repo commit.

## Task 2: Continue the Remaining `--tests` Sweep

**Files:**
- Modify: whichever target-specific RustPython backend / frontend files the next failing test requires
- Modify: relevant `tests/*.rs` only for justified RustPython xfails or temporary diagnostics which must be reverted before commit

- [ ] Run the full integration sweep.
- [ ] Stop at the first failing target only.
- [ ] Classify the failure:
  - local backend bug
  - frontend/backend contract leak
  - upstream RustPython blocker
- [ ] Fix only that first local failure or document/xfail the confirmed upstream blocker.
- [ ] Re-run the narrow target until green.
- [ ] Re-run the broad `--tests` sweep to the next frontier.
- [ ] Repeat until the sweep completes or only documented upstream blockers remain.

## Task 3: Audit the Ignore Set

**Files:**
- Modify: affected `tests/*.rs`
- Modify: `docs/upstream-issues/*.md`

- [ ] Enumerate all RustPython-only ignored tests under `third_party/pyo3-fork`.
- [ ] Re-test each ignore against the current RustPython pin before keeping it.
- [ ] Remove stale ignores resolved by recent RustPython changes.
- [ ] Ensure every remaining ignore names the exact upstream issue and failure class.

## Task 4: Repin and Upstream Synchronization Check

**Files:**
- Modify: RustPython pin files if needed
- Modify: `docs/upstream-issues/*.md`

- [ ] Reconfirm the current RustPython pin includes merged fixes already relied on.
- [ ] If new upstream fixes land during execution, repin in a controlled checkpoint only when it clearly reduces the blocker set.
- [ ] After any repin, rerun:
  - `test_proto_methods`
  - `test_pyfunction`
  - `test_mapping`
  - `test_class_basics`
  - `test_pybuffer_drop_without_interpreter`
  - full `--lib`
  - full `--tests`

## Task 5: Final Verification and Checkpoint

**Files:**
- Modify: status docs only if needed for final blocker summary

- [ ] Run `cargo check --manifest-path third_party/pyo3-fork/Cargo.toml -p pyo3`
- [ ] Run full `--lib` gate.
- [ ] Run full `--tests` gate.
- [ ] Summarize:
  - what is fully green
  - what remains ignored and why
  - which upstream RustPython issues still block full parity
- [ ] Commit the final checkpoint in:
  - `third_party/pyo3-fork`
  - parent repo pointer/docs

## Success Criteria

- `runtime-rustpython` passes unchanged upstream PyO3 `--lib` and `--tests` suites except for explicitly documented upstream RustPython blockers.
- Every remaining RustPython-only ignore points to a specific upstream issue and has been recently revalidated.
- No local test regressions are hidden behind generic ignores.
- CPython-family backend still compiles cleanly.
- The worktree is checkpointable with clear fork and parent commits.
