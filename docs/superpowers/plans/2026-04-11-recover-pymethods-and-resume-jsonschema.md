# Recover `#[pymethods]` and Resume `jsonschema-rs`

## Goal

Restore the lost `crates/pyo3-rustpython-derive/src/pymethods.rs` behavior to the minimum feature-complete state needed to:

1. Get `examples/jsonschema-rs` compiling again.
2. Reconfirm the earlier working examples still compile and run.
3. Reproduce the prior `jsonschema-rs` runtime state of roughly `20 passed / 3 failed`.
4. Then fix the subclass-construction bug for `extends=Validator` so inherited validator methods work.

Do **not** modify vendor package source logic. Allowed changes remain limited to:

- `Cargo.toml` / `pyproject.toml`
- one-line test harness `use` swap in `main.rs`
- the already-accepted stubs in `examples/jsonschema-rs/src/lib.rs` and `examples/jsonschema-dep`
- compatibility-layer code under `crates/pyo3-rustpython*`
- the local smoke harness in `examples/jsonschema-rs/src/main.rs`

## Current Repo Facts

These facts are confirmed from the current workspace, not just the handoff:

- `crates/pyo3-rustpython-derive/src/pymethods.rs` is currently `518` lines, which is the old version.
- `crates/pyo3-rustpython-derive/src/pyclass.rs` still contains the newer `extends=...` parsing and `base_init` MRO wiring, including temporary `eprintln!` debug output.
- `crates/pyo3-rustpython/src/types/module.rs` already calls `PyPayload::class()` from `add_class()`, so base-init and slot fixup are reachable again once macros are restored.
- `examples/jsonschema-rs/src/main.rs` still contains the dynamic `ValidationError` / `ReferencingError` injection and temporary debug smoke cases.
- `cargo test -p jsonschema-rs --no-run` currently fails with exactly three regressions:
  - constructor handling for `#[new]` returning `PyResult<Self>` in `examples/jsonschema-rs/src/http.rs`
  - getter wrapping for `fn name(&self) -> &str` in `examples/jsonschema-rs/src/lib.rs`
  - getter wrapping for `fn value(&self, py: Python<'_>) -> Py<PyAny>` in `examples/jsonschema-rs/src/lib.rs`

## Recommendation

Treat this as a **reconstruction** task, not a recovery task.

Do not spend more time trying to “find” the lost file unless the editor has private undo history outside the repo. The workspace evidence already shows the live codebase has moved on around `pymethods.rs`; the shortest path is to rebuild the missing macro behavior against the current runtime.

## File Map

These are the files the junior engineer should understand before editing anything:

- `crates/pyo3-rustpython-derive/src/pymethods.rs`
  - Primary blocker.
  - Must regain wrapper generation, constructor generation, getter/setter handling, slot aliasing, and special-case wrappers.
- `crates/pyo3-rustpython-derive/src/pyclass.rs`
  - Already contains `extends=...` support and MRO/base wiring.
  - Needs debug output removed later.
- `crates/pyo3-rustpython-derive/src/pyfunction.rs`
  - Best surviving reference for the FuncArgs-based extraction style used in the lost `#[pymethods]` code.
- `crates/pyo3-rustpython/src/types/module.rs`
  - `add_class()` correctly uses `PyPayload::class()`.
- `crates/pyo3-rustpython/src/slots.rs`
  - Dunder fixup target. Reconstructed slot aliasing must feed this file’s behavior.
- `crates/pyo3-rustpython/src/instance.rs`
  - Contains `Py`, `Bound`, `PyRef` plumbing relied on by wrappers and the planned subclass-construction fix.
- `examples/jsonschema-rs/src/lib.rs`
  - Contains real usage patterns the macros must compile.
- `examples/jsonschema-rs/src/main.rs`
  - Smoke harness. Keep it until runtime parity is back, then remove temporary debug checks.

## Non-Goals

Do not try to rebuild all of PyO3 macro compatibility in one pass.

Only restore the feature surface already proven necessary by:

- `hello`
- `point`
- `phase2-test`
- `blake3`
- `jiter`
- `rpds`
- `jsonschema-rs`

If a behavior is not exercised by those packages, defer it.

## Phase 0: Baseline and Safety

### Step 0.1

Create a dedicated branch before touching macros.

```bash
git checkout -b recover-pymethods-jsonschema
```

### Step 0.2

Capture the current failing baseline.

```bash
cargo test -p jsonschema-rs --no-run
```

Expected result:

- exactly the three current macro regressions
- warnings are acceptable

If new hard errors appear, stop and inspect the worktree before proceeding.

### Step 0.3

Do not edit any vendor crate logic files while doing macro recovery.

Allowed edit zones for this phase:

- `crates/pyo3-rustpython-derive/src/pymethods.rs`
- optionally small support changes in `crates/pyo3-rustpython/src/*` if macro output requires existing helpers

## Phase 1: Rebuild `pymethods.rs` to Compilation Parity

The purpose of this phase is simple: restore the lost macro features until `jsonschema-rs` compiles again. Do **not** start fixing runtime subclass behavior yet.

### Step 1.1

Use the current `pyfunction.rs` implementation as the extraction-template reference.

What to copy conceptually:

- FuncArgs-based extraction
- injection of `Python<'_>` from the VM
- explicit `map_err(|e: ::pyo3::PyErr| ::pyo3::err::into_vm_err(e))` annotations
- return-type normalization into `PyResult<PyObjectRef>`

What not to do:

- do not literally duplicate `pyfunction.rs`
- methods need self/cls/getter/setter handling that free functions do not

### Step 1.2

Rebuild method classification in `pymethods.rs`.

The dispatcher should separate methods into these buckets:

- `#[new]`
- `#[getter]`
- `#[setter]`
- `#[staticmethod]`
- `#[classmethod]`
- regular instance methods
- slot dunder methods that RustPython forbids directly on `#[pymethod]`

Deliverable:

- one clear classification function or compact set of helpers
- no ad hoc branching scattered across the whole file

### Step 1.3

Restore wrapper generation for PyO3-style method signatures.

You need a wrapper path whenever a method contains any of the following:

- `Python<'_>`
- `Bound<'_, T>`
- `&Bound<'_, T>`
- `Py<T>`
- `PyRef<T>`
- `PyRefMut<T>`
- borrowed return values that cannot appear in RustPython wrapper signatures
- `PyResult<T>` returns that must be converted through shim helpers

Required helper functions to recreate:

- `generate_pyresult_wrapper`
- `generate_next_pyref_wrapper`
- classmethod wrapper generator
- iter-self wrapper generator
- any shared “needs wrapper” detection helper

Definition of done:

- `#[pymethods]` no longer relies on RustPython’s direct getter/method signature support for PyO3-only signatures
- it generates RustPython-compatible wrappers around the original methods

### Step 1.4

Restore getter and setter wrappers.

This is mandatory because current compile failures prove the direct method form is not enough.

Cover these cases first:

- `#[getter] fn name(&self) -> &str`
- `#[getter] fn value(&self, py: Python<'_>) -> Py<PyAny>`
- getters with and without an explicit `&VirtualMachine`
- setters that accept PyO3-friendly extracted types

Implementation rule:

- if the original getter/setter signature is not directly accepted by RustPython’s `IntoPyGetterFunc` / setter traits, emit a RustPython-compatible wrapper and attach `#[pygetset]` / `#[pygetset(setter)]` to the wrapper, not the original method

### Step 1.5

Restore `#[new]` constructor handling.

Support these constructor shapes:

- `fn new(...) -> Self`
- `fn new(...) -> PyResult<Self>`
- `fn new(...) -> (Self, Base)`
- `fn new(...) -> PyResult<(Self, Base)>`

Also support the argument patterns already known to matter:

- `Option<T>` with `default=None`
- parameters extracted from `FuncArgs`
- optional keyword values
- `Python<'_>` injection
- `&VirtualMachine` passthrough when present

Important constraint:

In this phase, it is acceptable for tuple-return constructors to compile without fully fixing subclass payload behavior. The goal here is “compiles again,” not “runtime inheritance complete.”

### Step 1.6

Restore slot-dunder aliasing.

Recreate the pattern described in the progress notes:

- emit a safe method name such as `_pyo3_slot___eq__`
- register it as a normal `#[pymethod(name = "...")]`
- alias it back to the real dunder on the class
- let `fixup_dunder_slots()` wire the actual slot

Required target methods include at least:

- `__eq__`
- `__ne__`
- `__lt__`
- `__le__`
- `__gt__`
- `__ge__`
- `__hash__`
- `__iter__`
- `__next__`
- any other dunders already covered by `slots.rs`

### Step 1.7

Restore the lifetime-bounded return fallback.

When the original method has explicit lifetime generics in a way the wrapper cannot name safely, the wrapper should return `PyObjectRef` instead of a lifetime-parameterized Rust type.

This was already identified in the notes as the fix for the `E0261 undeclared lifetime` path. Reintroduce it deliberately instead of waiting for the compiler to rediscover it.

### Step 1.8

Keep reconstruction narrow.

After each group above, re-run:

```bash
cargo test -p jsonschema-rs --no-run
```

Do not batch a full rewrite and only test at the end. The file is large and regression-prone.

## Phase 2: Regression Gate for Earlier Examples

Once `jsonschema-rs` builds again, prove the reconstructed macro still supports the previously working packages.

Run these commands:

```bash
cargo run -p hello
cargo run -p point
cargo run -p phase2-test
cargo test -p blake3
cargo run -p jiter
cargo run -p rpds
```

Expected result:

- the simple examples still run
- `blake3` still passes
- `jiter` still runs
- `rpds` should at least compile and run to its prior level, even if iterator runtime bugs remain

If one of these regresses, fix macro parity before touching `jsonschema-rs` runtime bugs.

## Phase 3: Restore `jsonschema-rs` Runtime to the Prior 20/23 Level

Run the smoke harness exactly as the repo currently expects:

```bash
cargo run -p jsonschema-rs
```

Goal for this phase:

- return to the earlier “mostly working” state before the file loss
- do not yet optimize or clean up

The current harness still includes temporary debug cases:

- `draft7_mro`
- `draft7_has_is_valid`
- `validator_attrs`

Keep them until the subclass-construction fix is done. They are useful instrumentation right now.

## Phase 4: Fix `extends=Validator` Subclass Construction

This is the first real runtime bug to fix after macro parity is back.

### Problem Statement

`Draft7Validator` and the other draft validators extend `Validator`, but their constructors currently create objects with the subclass payload, not the base payload. RustPython method extraction for inherited methods then fails when a base-class method expects `PyRef<Validator>`.

Observed symptom:

- `Draft7Validator(...).is_valid(...)` fails with a type mismatch like:
  - expected `Validator`
  - found `Draft7Validator`

### Step 4.1

Locate constructor generation in the rebuilt `pymethods.rs` and add tuple-return analysis.

You need a helper that can detect and extract the base type from:

- `(Self, Base)`
- `PyResult<(Self, Base)>`

Use a helper name like:

- `extract_base_type_from_return`

Return value should be a parsed `syn::Type` or `syn::Path`, not a string.

### Step 4.2

When a constructor returns `(Self, Base)` or `PyResult<(Self, Base)>`, generate `slot_new`, not only `py_new`.

Reason:

- `slot_new` can create the Python object manually with the desired payload and class
- `py_new` only returns the Rust payload value and is insufficient for this subclass case

### Step 4.3

Inside generated `slot_new`, create the object with the **base payload** using `PyRef::new_ref(base, cls, dict)`.

Desired behavior:

- Python class is `Draft7Validator`
- stored Rust payload is `Validator`

This should let inherited `Validator` methods extract `PyRef<Validator>` successfully while preserving Python-level subclass identity and MRO.

### Step 4.4

Be careful about the existing `extends=` MRO logic in `pyclass.rs`.

Do not rework `pyclass.rs` first. The current MRO/base initialization is already the right foundation:

- it parses `extends=Validator`
- it initializes the base class first
- it populates `bases`
- it populates `mro`

Only touch `pyclass.rs` in this phase if subclass construction reveals a real missing type-size or init hook issue.

### Step 4.5

After the subclass constructor fix, verify:

```python
type(jsonschema_rs.Draft7Validator({"type": "integer"})).__name__
[c.__name__ for c in type(jsonschema_rs.Draft7Validator({"type": "integer"})).__mro__]
hasattr(jsonschema_rs.Draft7Validator({"type": "integer"}), "is_valid")
jsonschema_rs.Draft7Validator({"type": "integer"}).is_valid(42) == True
jsonschema_rs.Draft7Validator({"type": "integer"}).is_valid("not_int") == False
```

## Phase 5: Fix the Remaining Known `jsonschema-rs` Runtime Issues

Only start this phase once subclass construction works.

### Issue 5.1: `ValidationError` catchability

The harness dynamically creates `ValidationError` and `ReferencingError` using `type(...)`. The class can exist without behaving exactly like the raised exception type.

Work items:

- inspect how `ValidationError` objects are raised from Rust-side code
- compare their actual class against the dynamically injected module attribute
- confirm whether identity or inheritance mismatch is the cause of failed `try/except`

Verification:

```python
try:
    jsonschema_rs.validate({"type": "string"}, 42)
    caught = False
except jsonschema_rs.ValidationError:
    caught = True
caught
```

### Issue 5.2: `required_fields`

The prior note says this fails with a `ValueError` related to key type checking. Treat this as either:

- a dict-key conversion bug in the compatibility layer, or
- a RustPython behavioral mismatch triggered by the ffi/serde path

Work items:

- reproduce the exact exception text from the harness
- locate whether the failure originates in `ser.rs`, `types.rs`, or dictionary conversion helpers under `crates/pyo3-rustpython/src/types`
- prefer a compatibility-layer fix over package-local logic changes

### Issue 5.3: remove temporary debug smoke cases

Only after subclass dispatch is stable, remove:

- `draft7_mro`
- `draft7_has_is_valid`
- `validator_attrs`

Leave the real behavioral smoke tests in place.

## Phase 6: Cleanup

### Step 6.1

Remove debug `eprintln!` lines from `crates/pyo3-rustpython-derive/src/pyclass.rs`.

Current debug output is inside the generated `base_init` block and should be deleted after inheritance is validated.

### Step 6.2

Do a narrow warning cleanup only if it helps readability.

Ignore benign warnings such as the vendored `unexpected cfg` warnings unless they block useful output.

### Step 6.3

Do not address leak cleanup in this task unless the current work uncovers a correctness issue.

Examples:

- `Py::bind()` leaking via `Box::leak`
- any other known compat-layer leaks

Those are follow-up tasks, not blockers for jsonschema parity.

## Verification Checklist

The junior engineer must not claim completion without all of these:

### Build Gate A

```bash
cargo test -p jsonschema-rs --no-run
```

Must pass.

### Build Gate B

```bash
cargo run -p hello
cargo run -p point
cargo run -p phase2-test
cargo test -p blake3
cargo run -p jiter
cargo run -p rpds
```

Must match prior working behavior.

### Runtime Gate C

```bash
cargo run -p jsonschema-rs
```

First target:

- get back to roughly `20 passed / 3 failed`

Second target:

- fix the three known failures and remove temporary debug cases

### Runtime Gate D

Explicitly verify subclass dispatch and exception catchability with focused smoke snippets, not just the aggregated harness output.

## Execution Order Summary

Follow this order exactly:

1. Reconstruct `pymethods.rs` until `jsonschema-rs` compiles.
2. Re-run earlier examples to prevent silent macro regressions.
3. Reproduce prior `jsonschema-rs` runtime state.
4. Fix subclass construction using `slot_new` and base-payload objects.
5. Fix exception catchability and `required_fields`.
6. Remove debug scaffolding.

If you reverse steps 1 and 4, you will waste time debugging runtime behavior on top of a broken macro layer.

## Stop Conditions

Stop and ask for review if any of these happen:

- `cargo test -p jsonschema-rs --no-run` starts failing in more places after a macro edit
- earlier examples regress after `jsonschema-rs` compiles
- subclass construction needs runtime support outside the derive crate that is not obviously local
- a proposed fix requires changing vendor package logic instead of the compatibility layer

## Final Deliverable

A successful implementation should leave the repo in this state:

- `pymethods.rs` rebuilt to the minimum viable compat feature set
- `jsonschema-rs` compiling again
- earlier example packages still working
- draft validator subclasses dispatching inherited `Validator` methods
- temporary inheritance debug code removed
- smoke harness reduced to real behavioral checks
