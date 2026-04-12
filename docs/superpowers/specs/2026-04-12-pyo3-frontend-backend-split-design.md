# PyO3 Frontend/Backend Split With RustPython Backend — Design Spec

## Status

Accepted for implementation planning.

This spec supersedes the direction in [2026-04-02-pyo3-rustpython-backend-design.md](/Users/sunny/work/codepod/pyo3-rustpython/docs/superpowers/specs/2026-04-02-pyo3-rustpython-backend-design.md). The project is no longer "build a compatibility fork here and upstream later." The correct path is to fork PyO3 itself, perform the architectural split there, and add RustPython as a first-class motivating backend on top of that split.

The existing `pyo3-rustpython` implementation in this repository remains useful as reference material only. It is a source of experiments, working ideas, and code we may port deliberately into the fork, but it is not part of the target dependency graph for the upstreamable architecture.

## Goal

Refactor PyO3 so its user-facing semantics and proc-macro frontend are separated from runtime-specific backend implementation details, then prove the split by keeping the existing CPython backend and adding a RustPython backend in the same fork.

The end state should support this claim:

- PyO3 frontend semantics are backend-independent.
- CPython remains the reference backend.
- RustPython is a real backend, not a sidecar compatibility layer.
- The design is generic enough that additional backends such as PyPy or GraalPy can fit the same architecture, even if they continue to share CPython-oriented paths initially.

## Motivation

The current local work has surfaced structural issues that are not "missing API surface" bugs. They are architecture bugs.

Concrete evidence:

- Plain `#[pyclass]` types are not self-sufficient in the current local design because `PyClassImpl` is effectively coupled to method-side expansion.
- `#[pymethods]`, `#[pyfunction]`, class creation, slot wiring, exception state, and ffi behavior are entangled with runtime-specific assumptions.
- Getting upstream PyO3 tests to compile has repeatedly exposed boundaries where frontend semantics and backend execution are mixed together.
- Real packages such as `jsonschema-rs` also revealed a second boundary problem: some crates use portable PyO3 surface area, while others rely on CPython-specific ffi and ABI assumptions. The architecture should make that boundary explicit instead of hiding it.

This is why a major change in PyO3 itself is justified. Without the split, a RustPython backend remains a forked compatibility experiment. With the split, it becomes a coherent extension of PyO3.

This direction is also consistent with RustPython maintainers' prior interest in PyO3 interoperability:

- RustPython issue `#3016` explicitly invited collaboration around PyO3 support.

## Non-Goals

- Full CPython ABI emulation on RustPython.
- Making every existing `pyo3::ffi` layout-dependent crate work unchanged.
- Replacing PyO3's public user-facing syntax.
- Requiring new dependencies for the core architectural split unless later proven unavoidable.
- Solving packaging, wheel building, or distribution concerns in the first architectural PR beyond what is needed to validate the new backend model.

Clarification:

- preserving current PyO3 package compatibility on the CPython backend is a goal
- guaranteeing that all CPython-coupled crates work unchanged on RustPython from day one is not
- continuing to validate the fork through the old `pyo3-rustpython` crate is not acceptable once fork-backed validation exists

## Validation Reset

Validation for this project must measure the fork directly.

That means:

- unchanged upstream PyO3 tests must compile and run directly in `third_party/pyo3-fork`
- `pyo3-rustpython` may still be consulted for ideas or prior fixes, but it must not sit on the validation path for the new architecture

This reset is important because otherwise the project can appear to make progress while still testing the legacy shim instead of the new PyO3 fork.

## Design Principles

### 1. Frontend Semantics Must Be Backend-Neutral

The meaning of:

- `#[pyclass]`
- `#[pymethods]`
- `#[pyfunction]`
- conversions such as `FromPyObject` / `IntoPyObject`
- exception matching and display semantics
- method signatures, defaults, getters/setters, slot naming

must be defined once at the frontend level.

Backend code may affect how those semantics are realized at runtime, but it must not redefine what the macros mean.

### 2. Class Definition Must Not Depend On Method Blocks

`#[pyclass]` must always produce a complete class definition at the semantic layer, even when no `#[pymethods]` block exists.

`#[pymethods]` must contribute metadata and behavior to an existing class definition. It must not be the mechanism that causes the class definition to exist.

This is the direct architectural fix for the current upstream test failures around plain `#[pyclass]`.

### 3. Backend Responsibilities Must Be Explicit

Backends own runtime realization, not frontend meaning.

That includes:

- interpreter attachment and lifecycle
- object and type allocation
- subclass/base payload behavior
- attribute access and call dispatch
- slot installation
- exception state storage and retrieval
- ffi binding implementation

### 4. Preserve User-Facing PyO3 Syntax

The project may significantly change internal contracts between macros and runtime support, but external user syntax should remain the same unless a deviation is absolutely required and clearly justified.

### 5. Avoid New Global Dependencies

The split should avoid adding new always-on dependencies that affect PyO3 as a whole unless they are clearly necessary. In particular, the design should not assume `inventory` or similar registration crates as the first solution.

### 6. Preserve Existing CPython-Backed PyO3 Behavior

The split must not reduce compatibility for the existing PyO3 ecosystem on the CPython backend.

That includes not only the idealized documented surface, but also important de facto behavior that real packages rely on today because PyO3 is currently implemented against CPython semantics.

This means:

- existing PyO3 crates that work on the CPython backend today should continue to work after the split
- CPython-coupled behavior may remain CPython-backend-specific
- RustPython may initially support a narrower subset
- the architecture split itself must not turn current CPython-compatible crates into regressions

## Target Architecture

## Frontend Layer

The frontend layer lives in PyO3 proper and owns:

- proc-macro parsing
- semantic validation of attributes and signatures
- normalized metadata models for classes, methods, properties, constructors, functions, and slots
- frontend-owned wrapper generation logic
- public Rust API shape where the semantics are backend-independent

The frontend layer must be able to answer questions like:

- Is this method a getter, a classmethod, or a slot alias?
- Does this constructor return `Self` or `(Self, Base)`?
- Is this argument optional, extracted with `from_py_with`, or defaulted to `None`?
- What is the semantic method/function/class name exposed to Python?

without reference to CPython or RustPython.

## Backend Layer

The backend layer owns the runtime-specific realization of frontend semantics.

The backend interface should cover at least:

- interpreter token and interpreter lifecycle
- backend object handle and backend type handle
- type creation and class finalization
- instance creation, including exact-type and subclass-type construction
- method/getset/class attribute installation
- slot installation and backend slot hooks
- exception creation, fetch, restore, matching, and display
- conversions at the backend edge
- ffi support, isolated as backend-specific low-level code

The backend interface must be expressive enough that:

- CPython can remain the reference implementation
- RustPython can implement the same frontend semantics without frontend macro changes

## CPython Backend

The current PyO3 behavior should be re-expressed as the CPython backend.

This is not just for preservation. It is the reference backend that validates the split:

- existing PyO3 behavior should continue to work through the backend contract
- the backend contract is incomplete if the CPython path requires bypasses everywhere

## RustPython Backend

RustPython should be implemented as a real backend using the same frontend semantics.

That backend must cover enough surface to validate the architecture, including:

- `#[pyclass]`
- `#[pymethods]`
- `#[pyfunction]`
- conversions
- exceptions
- callable/module/type behavior
- enough ffi to support meaningful real packages and upstream tests

The backend should not attempt to paper over raw CPython ABI layout assumptions. Function-level ffi compatibility belongs in scope. ABI layout emulation does not.

## Internal Representation

The split requires a normalized internal model between macros and backend realization.

At minimum, the frontend should lower into data structures conceptually equivalent to:

- `ClassSpec`
- `MethodSpec`
- `PropertySpec`
- `ConstructorSpec`
- `FunctionSpec`
- `SlotSpec`
- `ArgumentSpec`
- `ReturnSpec`

These names are illustrative, not prescriptive.

What matters is that the frontend emits a complete semantic description which the backend consumes.

Example implications:

- `#[pyclass]` without `#[pymethods]` still yields a full `ClassSpec`.
- `#[pymethods]` yields one or more `MethodSpec` / `PropertySpec` / `ConstructorSpec` entries attached to that class.
- `#[pyfunction]` yields a `FunctionSpec` plus a generated frontend wrapper body that performs semantic extraction and return handling before backend-specific callable creation.

## Macro Contract Changes

The internal contract between `#[pyclass]` and `#[pymethods]` should change substantially.

### Current Bad Shape

- `#[pyclass]` establishes only part of the class story.
- `#[pymethods]` effectively materializes the usable class definition in practice.
- backend-specific assumptions leak into method-generation logic.

### Required Shape

- `#[pyclass]` defines the class semantics completely.
- `#[pymethods]` enriches the class with methods, properties, constructors, and slots.
- backend-specific code is called only through backend hooks or backend-owned realization helpers.

This likely implies a major rewrite of the internal lowering path for these macros.

## Backend API Boundaries

The backend API should be designed around stable responsibilities, not around one-to-one mirroring of current CPython implementation details.

Suggested boundary areas:

### Interpreter

- enter/attach interpreter
- exit/detach interpreter
- current exception state access
- backend context access

### Objects And Types

- opaque object/type handles
- clone/borrow/owned semantics
- attribute access
- method invocation
- type lookup and subclass checks

### Class Realization

- create class/type from frontend class metadata
- apply bases and inheritance information
- register methods/getsets/class attrs
- finalize slots and backend-specific type initialization

### Instance Construction

- allocate instance from class/type
- support constructors returning `Self`
- support constructors returning `(Self, Base)` or equivalent subclass/base split
- support exact payload vs base payload decisions

### Exceptions

- construct backend exception objects
- fetch/restore current exception
- match exception type
- render exception text

### FFI

- backend-specific module for low-level ffi entry points
- isolated from frontend macros
- explicitly scoped to portable function-level compatibility where possible

## Compatibility Scope

The architecture should support:

- unchanged PyO3 user syntax for portable PyO3 features
- unchanged behavior for existing CPython-backed PyO3 crates on the CPython backend
- meaningful portions of PyO3's own upstream tests against unchanged test sources
- progressively more complex real packages running on RustPython

The architecture should not claim:

- unchanged support for crates that depend on CPython object layout or undocumented ABI details through `pyo3::ffi`

Those crates remain part of PyO3's effective compatibility surface on the CPython backend and must not be regressed there by the split.

Those cases should be explicitly treated as outside the first-class portable surface unless PyO3 later chooses to formalize them.

## Implementation Strategy

### Phase 1: Spec And Fork

- fork PyO3 into a working branch owned by this project
- keep this repo as design notes, package validation, and migration evidence
- no new local compatibility hacks should be introduced here that fight the intended PyO3 split

### Phase 2: Introduce Backend Interface In The PyO3 Fork

- define backend traits/modules in the PyO3 fork
- move CPython-specific logic behind the backend boundary
- keep behavior stable for existing CPython users as much as possible

### Phase 3: Move Macro Lowering To Frontend-Owned Semantics

- refactor `#[pyclass]`, `#[pymethods]`, and `#[pyfunction]`
- make plain `#[pyclass]` complete at the semantic layer
- make `#[pymethods]` additive rather than class-defining

### Phase 4: Add RustPython Backend

- implement backend hooks for RustPython
- bring up the minimal feature set needed to validate the split
- run unchanged PyO3 upstream tests wherever they exercise portable frontend semantics

### Phase 5: Validation Ladder

- PyO3 upstream compile and runtime tests
- regression checks for existing CPython-backed package behavior
- existing package ladder already proven useful:
  - `blake3`
  - `rpds`
  - `jiter`
  - `jsonschema-rs`
- future packages chosen to de-risk Pydantic-class workloads

## Validation Strategy

The architecture is only successful if it proves itself in three ways.

### 1. PyO3 Upstream Tests

Use unchanged upstream test sources.

The suite should be classified as:

- portable/frontend tests that must pass through the backend split
- backend-specific or ABI-specific tests that may remain CPython-only and must be documented as such

### 2. CPython-Backend Regression Validation

Use existing PyO3 behavior as a compatibility bar for the CPython backend, including de facto behavior used by real crates.

The refactor is not acceptable if it makes previously working CPython-backed PyO3 crates stop working.

### 3. Existing Real Packages

Continue validating against real upstream packages using sub-repositories and forks for legitimate upstream portability fixes.

### 4. Architectural Hygiene

Review whether new code respects the split:

- no frontend macro logic should directly depend on CPython or RustPython internals
- backend-specific code should remain localized
- class/function semantic lowering should be reusable across backends

## Risks

### Risk 1: The Split Is Too Shallow

If CPython behavior still leaks through most frontend code paths, the architecture is not really split and RustPython support will remain brittle.

Mitigation:

- force key frontend paths to compile against backend abstractions only
- use plain `#[pyclass]` and imported `wrap_pyfunction!` failures as canaries for bad coupling

### Risk 2: The Split Is Too Deep Or Too Abstract

If the backend contract is over-generalized too early, implementation cost will explode and the PR will be hard to review.

Mitigation:

- define the smallest backend surface that cleanly supports CPython and RustPython
- grow only when real tests or packages require it

### Risk 3: CPython Compatibility Regressions

The refactor touches central PyO3 machinery.

Mitigation:

- preserve the CPython backend as the reference implementation
- keep CPython-path validation in every phase
- explicitly treat existing package compatibility on CPython as a release-blocking requirement for the architectural PR

### Risk 4: ABI-Dependent Packages Blur The Goal

Some packages will fail for reasons unrelated to the frontend/backend split.

Mitigation:

- explicitly classify CPython-ABI-dependent behavior as out of the portable surface
- upstream package portability fixes where appropriate

## Deliverables

The architecture effort should produce:

- a PyO3 fork with an explicit frontend/backend split
- a CPython backend preserved through that split
- a RustPython backend implemented enough to justify the architecture
- a documented classification of portable vs ABI-dependent PyO3 behavior
- proof via unchanged upstream PyO3 tests and real-package validation

## Success Criteria

This design is successful when all of the following are true:

- PyO3 fork compiles and runs with CPython through the new backend boundary
- existing CPython-backed PyO3 package behavior is not regressed by the split
- RustPython works as a real backend rather than a sidecar compat layer
- unchanged PyO3 upstream tests cover meaningful frontend semantics on the RustPython backend
- plain `#[pyclass]` works without requiring `#[pymethods]`
- imported-name `wrap_pyfunction!(foo, py)` works without local test edits
- real packages continue to provide evidence beyond synthetic tests

## Immediate Next Step

Write an implementation plan for the fork-first execution model:

- create and publish a PyO3 fork
- map the first backend boundary extraction steps
- decide the initial slice for CPython backend preservation and RustPython backend bring-up
