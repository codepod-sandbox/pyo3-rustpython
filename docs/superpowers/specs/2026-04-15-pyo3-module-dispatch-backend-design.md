# PyO3 Module-Dispatch Backend Design

> Goal: refactor PyO3 so backend choice is made once at compile time in a centralized dispatcher, while frontend modules in `pyo3` and `pyo3-ffi` have zero backend-specific knowledge.

## Context

The current live RustPython port in `third_party/pyo3-fork` works, but it achieves backend selection largely through scattered `#[cfg(PyRustPython)]` and `#[cfg(not(PyRustPython))]` branches across frontend-facing modules. That proves the backend is viable, but it is not the architectural end state we want to upstream into PyO3.

The desired architecture is:

- compile-time backend selection
- one centralized dispatcher per crate
- frontend modules unaware of CPython, PyPy, GraalPy, RustPython, or backend-private implementation names
- backend-specific realization isolated in dedicated backend modules

This applies equally to `pyo3` and `pyo3-ffi`. `pyo3-ffi` is not exempt from the split; it is part of the same architecture.

`pyo3-build-config` is also part of the backend-selection story. It is allowed to contain backend-selection logic because producing the selected backend configuration is its purpose. The "no backend knowledge outside `backend/`" rule applies to frontend implementation crates, not to the build-config crate that defines the compile-time choice.

## Non-goals

This refactor does not aim to:

- introduce runtime backend selection
- make internal public APIs generic over a backend type
- redesign PyO3 around trait objects or dependency injection
- eliminate all backend differences semantically
- complete all RustPython functionality as part of the first structural refactor

The goal is separation of concerns and centralized backend choice, not a full semantic rewrite in one step.

Semantic differences that remain observable to users are still frontend responsibilities:

- frontend owns whether a semantic difference is documented, exposed, or normalized
- backend owns the mechanical realization of that behavior

Example: if `allow_threads` is effectively a no-op on a backend, frontend owns the public semantic contract and documentation, while backend owns the low-level implementation strategy.

## Requirements

The design must satisfy all of the following:

1. Backend choice is compile-time only.
2. Backend choice happens in exactly one dispatcher boundary per crate.
3. Frontend modules outside `backend/` do not use backend cfgs such as `PyRustPython`, `PyPy`, or `GraalPy`.
4. Frontend modules outside `backend/` do not reference backend-private names like `rustpython_storage`, `PyRustPython_*`, `ObjExt*`, or sidecar-specific helpers.
5. Frontend code owns semantic flow; backend code owns backend-specific realization.
6. The architecture remains natural for PyO3’s existing concrete internal types.
7. Migration is incremental by surface area, not a flag-day rewrite.

## Chosen approach

We will use a module-dispatch architecture.

Each crate gets a backend tree:

- `src/backend/mod.rs`
- `src/backend/current.rs`
- `src/backend/cpython/...`
- `src/backend/rustpython/...`

The same pattern applies in `pyo3-ffi`.

`backend/current.rs` is the only place where compile-time backend selection occurs. Its job is to re-export the chosen backend implementation into stable surface modules.

Example shape:

```rust
pub mod err_state {
    #[cfg(PyRustPython)]
    pub use super::rustpython::err_state::*;
    #[cfg(not(PyRustPython))]
    pub use super::cpython::err_state::*;
}

pub mod runtime {
    #[cfg(PyRustPython)]
    pub use super::rustpython::runtime::*;
    #[cfg(not(PyRustPython))]
    pub use super::cpython::runtime::*;
}

pub mod pyclass {
    #[cfg(PyRustPython)]
    pub use super::rustpython::pyclass::*;
    #[cfg(not(PyRustPython))]
    pub use super::cpython::pyclass::*;
}
```

Frontend modules call only these stable surface modules:

- `crate::backend::current::err_state`
- `crate::backend::current::runtime`
- `crate::backend::current::pyclass`
- and corresponding surfaces in `pyo3-ffi`

No frontend code outside `backend/` is allowed to select a backend directly.

Important constraint: `backend/current.rs` must expose only stable per-surface modules. It must not expose a whole selected backend module alias such as `current_impl`, because that would let frontend code tunnel through the dispatcher and reach backend-private names.

## Why module dispatch instead of type-driven backend parameterization

We considered a type-driven design where internal types become generic over a backend type and compile-time choice is encoded through a selected backend alias.

We are not choosing that approach because:

- it is far more invasive for existing PyO3 internals
- it would force backend generics into places that are currently concrete and stable
- it is a worse fit for upstreaming incrementally
- it does not materially improve the compile-time selection model for this use case

Module dispatch gives us the architecture we want with much lower churn and much better compatibility with current PyO3 structure.

## Backend boundary rules

These rules define the architectural contract.

### Rule 1: no backend cfgs outside backend modules

Outside `backend/`, the codebase should not contain:

- `#[cfg(PyRustPython)]`
- `#[cfg(not(PyRustPython))]`
- `#[cfg(PyPy)]`
- `#[cfg(GraalPy)]`
- equivalent backend-selection branches

Allowed exceptions:

- tests intentionally checking backend-specific behavior
- short-lived migration exceptions tracked in the repository allowlist file `tools/backend-boundary-allowlist.txt`

### Rule 2: no backend-private names outside backend modules

Outside `backend/`, code should not reference:

- `rustpython_storage`
- `PyRustPython_*`
- `ObjExt*`
- sidecar-owner implementation names
- backend-specific VM/runtime helper names

If frontend needs that behavior, it calls a backend surface function.

### Rule 3: frontend owns semantics, backend owns realization

Frontend answers:

- what semantic step is required
- when in the flow it should happen
- what invariants the step must satisfy

Backend answers:

- how that step is achieved for the selected runtime

Example:

- frontend: after pyclass object creation, backend-specific storage hookup may be required
- CPython backend: no-op
- RustPython backend: install sidecar owner

### Rule 4: shared declarations are fine, shared backend branching is not

Some common type declarations and API shapes may remain shared, especially in `pyo3-ffi`. That is acceptable.

What must move out is backend-specific implementation branching.

If a file is mostly shared declarations with backend-specific implementations hidden behind dispatcher calls, that file is still frontend-safe.

For `pyo3-ffi`, backend-specific struct layouts do not count as shared declarations. Backend-owned layouts such as `PyObject`, `PyTypeObject`, or backend-specific header mirrors must come from the dispatcher output, for example by re-exporting backend-owned types through a stable surface:

```rust
pub use crate::backend::current::object::{PyObject, PyTypeObject};
```

What is forbidden is defining those layouts directly in a frontend file with backend cfg branches.

## Surface-oriented migration plan

We will migrate by backend-sensitive surface, not by crate-wide rewrite.

### First wave

These surfaces currently contain the highest-value backend leakage and should move first:

1. Runtime / interpreter lifecycle
   - initialize
   - attach
   - finalize
   - runtime-thread handoff

   RustPython-specific constraint: the live backend currently routes work through a dedicated runtime thread. The runtime surface design must account for cross-thread dispatch as a first-class backend capability rather than treating interpreter lifecycle as purely local state.

2. Error state
   - raised exception storage / fetch / restore
   - backend-specific error-state ownership

3. Object / type realization
   - type creation from spec
   - slot installation
   - heap-type metadata

4. Pyclass storage and layout
   - inline CPython-family layout
   - RustPython sidecar layout
   - storage hookup after construction

5. Synchronization primitives with backend-dependent semantics
   - critical section
   - once-lock / initialization assumptions

6. Datetime import / capsule behavior

### Second wave

Move backend-conditional helpers in frontend container/type modules behind dispatcher surfaces:

- tuple
- dict
- string
- list
- set
- frozenset
- mapping / sequence registration differences
- exception declaration helpers

### Third wave

Collapse remaining backend-specific macros/codegen branching into dedicated backend surfaces where feasible, keeping frontend macro orchestration backend-neutral.

`pyo3-macros-backend` does not get an independent backend dispatcher of its own. The macro backend stays frontend-neutral and emits calls or metadata that target backend-owned realization surfaces in `pyo3` / `pyo3-ffi`. Backend selection remains centralized in the implementation crates, not duplicated inside macro expansion.

## Per-surface structure

For each migrated surface, use this shape:

- frontend-facing orchestration file remains in place
- backend surface lives under `backend/current/<surface>.rs`
- backend implementations live under:
  - `backend/cpython/<surface>.rs`
  - `backend/rustpython/<surface>.rs`

Frontend files should become thin semantic orchestrators.

For each surface, Wave 1 always includes creating both backend sides:

- extracting the existing CPython-family logic into `backend/cpython/<surface>.rs`
- extracting the RustPython logic into `backend/rustpython/<surface>.rs`

The CPython backend is not "free"; it must be created explicitly by extracting the existing default code path out of the frontend.

Example for error state:

- `src/err/err_state.rs`
  - keeps `PyErrState` semantic behavior
  - delegates storage/fetch/restore mechanics to `crate::backend::current::err_state`

Example for pyclass creation:

- `src/pyclass/create_type_object.rs`
  - keeps high-level type creation flow
  - delegates backend-specific type-object realization and storage hookup to `crate::backend::current::pyclass`

Example for FFI datetime:

- `pyo3-ffi/src/datetime.rs`
  - retains common API declarations
  - delegates backend-specific import and capsule behavior to `pyo3_ffi::backend::current::datetime`

## Dispatcher shape

Dispatcher modules should be concrete and surface-specific. We are not introducing a mega-trait that every backend must implement.

Why:

- PyO3’s backend differences are not uniformly shaped
- forcing everything through one trait creates brittle abstractions
- surface-specific modules remain explicit and readable

So the dispatcher layer is a namespace and module routing mechanism, not an object model.

Backends may still use private traits internally within backend modules if that improves code sharing. The rule is about the architectural boundary seen by frontend code, not about forbidding ordinary private implementation techniques inside a backend.

## Relationship to existing `backend::traits`

The current `backend::traits` module in `third_party/pyo3-fork` is not the right architectural backbone. Its marker traits are too weak to enforce or meaningfully represent the split.

Under the new design:

- either these traits become internal helper documentation only
- or they are removed if they do not carry real architectural value

The module dispatcher, not these traits, is the primary architectural boundary.

## Incremental migration policy

The migration will not be completed in one giant commit.

For each surface:

1. create backend dispatcher modules
2. move backend-specific implementation there
3. replace frontend cfg branches with dispatcher calls
4. remove backend-specific names from frontend code
5. run targeted tests for that surface

Temporary coexistence is acceptable if:

- the scope is localized
- there is a clear next step to eliminate the remaining frontend backend knowledge

But the long-term target is strict: no backend selection logic outside dispatcher modules.

## Testing strategy

This refactor is architectural, so testing must prove both correctness and containment.

### Structural tests

Add repository-level checks that fail if frontend modules outside `backend/` introduce:

- `PyRustPython`
- backend-selection cfgs
- known backend-private names

Implementation shape:

- add a repository check command at `cargo xtask check-backend-boundary`
- it scans frontend paths outside `backend/`
- it consults `tools/backend-boundary-allowlist.txt`
- it fails CI with the offending matches

### Semantic verification

For each migrated surface, rerun:

- relevant `pyo3 --lib` and `--tests` slices under `runtime-rustpython`
- relevant CPython-focused validation slices
- downstream package checks where that surface matters

### Regression checks

Track:

- PyO3 own suite on CPython-family
- PyO3 own suite on RustPython
- representative downstream packages on CPython

The module dispatcher refactor must not reduce the downstream CPython confidence already achieved.

## Upstreaming implications

This architecture is upstreamable in principle because:

- it preserves compile-time backend selection
- it keeps frontend semantics backend-neutral
- it centralizes backend knowledge instead of spreading it
- it makes adding non-CPython backends a structured exercise rather than a cfg explosion

It also improves reviewability for PyO3 maintainers:

- backend selection is visible in one place
- surface ownership is clear
- rebases stop depending on hunting scattered cfg branches

## Risks

1. Over-abstraction
   - if we invent interfaces that do not match real backend differences, the code gets worse
   - mitigation: surface-specific module dispatch, not generic mega-traits

2. Partial migration drift
   - mixed old/new patterns could linger
   - mitigation: explicit structural rule and migration checklist

3. `pyo3-ffi` false comfort
   - it is tempting to treat `pyo3-ffi` as “already backend-specific” and leave cfgs there
   - mitigation: apply the same dispatcher rule there too

4. Macro boundary confusion
   - backend-specific codegen can leak back into frontend macros
   - mitigation: macros remain frontend-neutral orchestrators that emit calls into backend-owned realization modules

## Recommended execution order

1. Introduce dispatcher scaffolding in `pyo3` and `pyo3-ffi`
2. Add structural enforcement scaffolding and the explicit allowlist mechanism
3. Migrate runtime/interpreter lifecycle
4. Migrate error state
5. Migrate pyclass storage/layout and type realization
6. Migrate synchronization primitives
7. Migrate container/type helper hotspots
8. Remove obsolete backend marker traits if they no longer help

## Definition of done

This refactor is done when:

- backend selection is centralized in dispatcher modules
- frontend modules outside `backend/` contain no backend-selection cfgs
- frontend modules outside `backend/` reference no backend-private names
- `pyo3` and `pyo3-ffi` both follow the same architectural rule
- CPython and RustPython validation still pass on the existing verified surface
