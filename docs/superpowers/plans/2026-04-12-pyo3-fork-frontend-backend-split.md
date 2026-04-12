# PyO3 Fork Frontend/Backend Split Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Refactor a fork of PyO3 so frontend macro semantics are separated from backend runtime realization, preserve CPython-family compatibility, and add a RustPython backend in the same fork.

**Architecture:** Work in a dedicated PyO3 fork checkout under `third_party/pyo3-fork`. Introduce a backend boundary in PyO3 itself, move macro lowering toward backend-neutral semantic specs, preserve a CPython-family backend as the reference backend, and add RustPython as the motivating backend. Use unchanged upstream PyO3 tests as the primary guardrail and package-level validation as the secondary guardrail. Validation must target the fork directly; `pyo3-rustpython` is reference material only and not part of the new dependency path.

**Tech Stack:** Rust, Cargo workspaces, PyO3 fork, `pyo3-macros-backend`, `pyo3-ffi`, RustPython, unchanged upstream PyO3 tests, local package ladder (`blake3`, `rpds`, `jiter`, `jsonschema-rs`).

---

## File Structure

### Fork Workspace

- Create: `third_party/pyo3-fork/`
- Create: `third_party/pyo3-fork/docs/backend-architecture.md`
- Create: `third_party/pyo3-fork/src/backend/mod.rs`
- Create: `third_party/pyo3-fork/src/backend/traits.rs`
- Create: `third_party/pyo3-fork/src/backend/cpython.rs`
- Create: `third_party/pyo3-fork/src/backend/rustpython.rs`
- Create: `third_party/pyo3-fork/src/backend/spec.rs`
- Create: `third_party/pyo3-fork/src/backend/tests.rs`

### Fork PyO3 Runtime Files

- Modify: `third_party/pyo3-fork/src/lib.rs`
- Modify: `third_party/pyo3-fork/src/impl_/pyclass.rs`
- Modify: `third_party/pyo3-fork/src/impl_/pyclass_init.rs`
- Modify: `third_party/pyo3-fork/src/impl_/pyfunction.rs`
- Modify: `third_party/pyo3-fork/src/impl_/pymethods.rs`
- Modify: `third_party/pyo3-fork/src/err/mod.rs`
- Modify: `third_party/pyo3-fork/src/types/module.rs`
- Modify: `third_party/pyo3-fork/src/type_object.rs`
- Modify: `third_party/pyo3-fork/src/instance.rs`
- Modify: `third_party/pyo3-fork/src/interpreter_lifecycle.rs`

### Fork Macro Files

- Modify: `third_party/pyo3-fork/pyo3-macros-backend/src/lib.rs`
- Modify: `third_party/pyo3-fork/pyo3-macros-backend/src/pyclass.rs`
- Modify: `third_party/pyo3-fork/pyo3-macros-backend/src/pyfunction.rs`
- Modify: `third_party/pyo3-fork/pyo3-macros-backend/src/pyimpl.rs`
- Modify: `third_party/pyo3-fork/pyo3-macros-backend/src/pymethod.rs`
- Create: `third_party/pyo3-fork/pyo3-macros-backend/src/backend_spec.rs`

### Fork ffi / Feature Wiring

- Modify: `third_party/pyo3-fork/Cargo.toml`
- Modify: `third_party/pyo3-fork/pyo3-ffi/Cargo.toml`
- Modify: `third_party/pyo3-fork/pyo3-ffi/src/lib.rs`
- Modify: `third_party/pyo3-fork/pyo3-ffi/src/modsupport.rs`
- Modify: `third_party/pyo3-fork/pyo3-build-config/src/lib.rs`

### Validation Files

- Modify: `third_party/pyo3-fork/Cargo.toml`
- Modify: `docs/upstream-packages.md`
- Modify: `docs/superpowers/specs/2026-04-12-pyo3-frontend-backend-split-design.md`
- Create: `docs/superpowers/plans/2026-04-12-pyo3-fork-frontend-backend-split.md`

---

### Task 1: Create The PyO3 Fork Workspace

**Files:**
- Create: `third_party/pyo3-fork/`
- Create: `third_party/pyo3-fork/docs/backend-architecture.md`
- Modify: `Cargo.toml`
- Test: `cargo metadata --format-version 1`

- [ ] **Step 1: Write the failing workspace expectation**

Create a one-off check command description in the terminal notes and use it as the first failing gate:

```text
Expected initial failure:
- `cargo metadata --format-version 1` does not mention `third_party/pyo3-fork`
- `test -d third_party/pyo3-fork/.git` fails
```

- [ ] **Step 2: Run the failing check**

Run: `test -d third_party/pyo3-fork/.git`
Expected: exit code `1`

- [ ] **Step 3: Add the fork checkout and document its role**

Create the fork checkout and add a short architecture note:

```bash
git submodule add https://github.com/codepod-sandbox/pyo3.git third_party/pyo3-fork
```

Write `third_party/pyo3-fork/docs/backend-architecture.md`:

```md
# PyO3 Backend Architecture Notes

This fork hosts the frontend/backend split work.

- `src/backend/` owns backend contracts and backend implementations
- `pyo3-macros-backend/` owns backend-neutral semantic lowering
- CPython remains the reference backend
- RustPython is the motivating backend for the split
```

Update the root workspace comments only if needed; do not add the fork as a Cargo member yet.

- [ ] **Step 4: Run the check to verify the fork exists**

Run: `test -d third_party/pyo3-fork/.git`
Expected: exit code `0`

- [ ] **Step 5: Verify cargo workspace stability**

Run: `cargo metadata --format-version 1 >/tmp/pyo3-rustpython-metadata.json`
Expected: exit code `0`

- [ ] **Step 6: Commit**

```bash
git add .gitmodules third_party/pyo3-fork Cargo.toml
git commit -m "chore: add PyO3 fork workspace"
```

### Task 2: Add Failing Upstream Gates In The Fork

**Files:**
- Test: `cargo test --manifest-path third_party/pyo3-fork/Cargo.toml -p pyo3 --test test_inheritance --no-run`
- Test: `cargo test --manifest-path third_party/pyo3-fork/Cargo.toml -p pyo3 --test test_pyfunction --no-run`

- [ ] **Step 1: Record the two failing upstream gates**

Use these exact gates as red tests:

```text
Gate A:
cargo test --manifest-path third_party/pyo3-fork/Cargo.toml -p pyo3 --test test_inheritance --no-run

Gate B:
cargo test --manifest-path third_party/pyo3-fork/Cargo.toml -p pyo3 --test test_pyfunction --no-run
```

The expected failing diagnostics at this stage include:

```text
E0277: the trait bound `SubclassAble: PyClassImpl` is not satisfied
E0277: the trait bound `MyClass: PyClassImpl` is not satisfied
```

- [ ] **Step 2: Run the failing inheritance gate**

Run: `cargo test --manifest-path third_party/pyo3-fork/Cargo.toml -p pyo3 --test test_inheritance --no-run --message-format short`
Expected: FAIL with `SubclassAble: PyClassImpl`

- [ ] **Step 3: Run the failing pyfunction gate**

Run: `cargo test --manifest-path third_party/pyo3-fork/Cargo.toml -p pyo3 --test test_pyfunction --no-run --message-format short`
Expected: FAIL with plain `#[pyclass]` `PyClassImpl` errors

- [ ] **Step 4: Record that validation is direct on the fork**

Use the fork's own unchanged upstream tests as the source of truth. Do not route these gates through `pyo3-rustpython` or a shim-backed local harness.

- [ ] **Step 5: Commit**

```bash
git add docs/superpowers/specs/2026-04-12-pyo3-frontend-backend-split-design.md docs/superpowers/plans/2026-04-12-pyo3-fork-frontend-backend-split.md
git commit -m "docs: reset validation to run directly on PyO3 fork"
```

### Task 3: Introduce Backend Contracts In The Fork

**Files:**
- Create: `third_party/pyo3-fork/src/backend/mod.rs`
- Create: `third_party/pyo3-fork/src/backend/traits.rs`
- Create: `third_party/pyo3-fork/src/backend/spec.rs`
- Modify: `third_party/pyo3-fork/src/lib.rs`
- Test: `cargo check --manifest-path third_party/pyo3-fork/Cargo.toml -p pyo3`

**Prerequisite:** if `third_party/pyo3-fork` is still an empty fork repository, first seed it from upstream `PyO3/pyo3` so the fork checkout contains the real PyO3 source tree before adding backend modules.

- [ ] **Step 1: Write the failing compile hook**

Create a minimal reference in `third_party/pyo3-fork/src/lib.rs` before the modules exist:

```rust
pub mod backend;
```

Expected compile failure:

```text
file not found for module `backend`
```

- [ ] **Step 2: Run the failing fork compile**

Run: `cargo check --manifest-path third_party/pyo3-fork/Cargo.toml -p pyo3`
Expected: FAIL with `file not found for module 'backend'`

- [ ] **Step 3: Create the backend module skeleton**

Create `third_party/pyo3-fork/src/backend/mod.rs`:

```rust
pub mod cpython;
pub mod rustpython;
pub mod spec;
pub mod traits;

pub use traits::{Backend, BackendClassBuilder, BackendFunctionBuilder, BackendInterpreter};
```

Create `third_party/pyo3-fork/src/backend/traits.rs`:

```rust
pub trait Backend {
    type Interpreter: BackendInterpreter;
    type ClassBuilder<'py>: BackendClassBuilder<'py>
    where
        Self: 'py;
    type FunctionBuilder<'py>: BackendFunctionBuilder<'py>
    where
        Self: 'py;
}

pub trait BackendInterpreter {}

pub trait BackendClassBuilder<'py> {
    type ClassHandle;
}

pub trait BackendFunctionBuilder<'py> {
    type FunctionHandle;
}
```

Create `third_party/pyo3-fork/src/backend/spec.rs`:

```rust
#[derive(Debug, Clone)]
pub struct ClassSpec<'a> {
    pub name: &'a str,
    pub module: Option<&'a str>,
}

#[derive(Debug, Clone)]
pub struct FunctionSpec<'a> {
    pub name: &'a str,
}
```

Create backend placeholders:

```rust
// third_party/pyo3-fork/src/backend/cpython.rs
pub struct CpythonBackend;

// third_party/pyo3-fork/src/backend/rustpython.rs
pub struct RustPythonBackend;
```

- [ ] **Step 4: Re-export the backend module from `src/lib.rs`**

Add near other top-level modules:

```rust
pub mod backend;
```

- [ ] **Step 5: Run the fork compile to verify the skeleton passes**

Run: `cargo check --manifest-path third_party/pyo3-fork/Cargo.toml -p pyo3`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git -C third_party/pyo3-fork add src/lib.rs src/backend
git -C third_party/pyo3-fork commit -m "refactor: add backend contract skeleton"
```

### Task 4: Add Backend-Neutral Macro Specs

**Files:**
- Create: `third_party/pyo3-fork/pyo3-macros-backend/src/backend_spec.rs`
- Modify: `third_party/pyo3-fork/pyo3-macros-backend/src/lib.rs`
- Modify: `third_party/pyo3-fork/pyo3-macros-backend/src/pyclass.rs`
- Modify: `third_party/pyo3-fork/pyo3-macros-backend/src/pyfunction.rs`
- Modify: `third_party/pyo3-fork/pyo3-macros-backend/src/pyimpl.rs`
- Test: `cargo check --manifest-path third_party/pyo3-fork/Cargo.toml -p pyo3-macros-backend`

- [ ] **Step 1: Write the failing macro-backend import**

Add this to `pyo3-macros-backend/src/lib.rs` before creating the file:

```rust
mod backend_spec;
```

Expected: `file not found for module 'backend_spec'`

- [ ] **Step 2: Run the failing macro crate check**

Run: `cargo check --manifest-path third_party/pyo3-fork/Cargo.toml -p pyo3-macros-backend`
Expected: FAIL with `file not found for module 'backend_spec'`

- [ ] **Step 3: Create normalized semantic spec types**

Create `third_party/pyo3-fork/pyo3-macros-backend/src/backend_spec.rs`:

```rust
#[derive(Debug, Clone)]
pub struct ClassSpec {
    pub rust_ident: syn::Ident,
    pub python_name: String,
    pub module: Option<String>,
    pub has_methods: bool,
}

#[derive(Debug, Clone)]
pub struct MethodSpec {
    pub python_name: String,
    pub is_classmethod: bool,
    pub is_staticmethod: bool,
    pub is_getter: bool,
    pub is_setter: bool,
    pub is_constructor: bool,
}

#[derive(Debug, Clone)]
pub struct FunctionSpec {
    pub python_name: String,
}
```

- [ ] **Step 4: Thread the spec types into the macro entry points**

Add imports and construction points like:

```rust
use crate::backend_spec::{ClassSpec, FunctionSpec, MethodSpec};
```

Then, in `pyclass.rs`, create a `ClassSpec` as soon as class attributes are parsed.

In `pyfunction.rs`, create a `FunctionSpec` as soon as the final Python-visible function name is known.

In `pyimpl.rs`, collect `MethodSpec` values instead of immediately assuming backend-specific lowering.

- [ ] **Step 5: Run the macro crate check**

Run: `cargo check --manifest-path third_party/pyo3-fork/Cargo.toml -p pyo3-macros-backend`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git -C third_party/pyo3-fork add pyo3-macros-backend/src
git -C third_party/pyo3-fork commit -m "refactor: add backend-neutral macro specs"
```

### Task 5: Make `#[pyclass]` Complete Without `#[pymethods]`

**Files:**
- Modify: `third_party/pyo3-fork/pyo3-macros-backend/src/pyclass.rs`
- Modify: `third_party/pyo3-fork/src/impl_/pyclass.rs`
- Modify: `third_party/pyo3-fork/src/impl_/pyclass_init.rs`
- Modify: `third_party/pyo3-fork/src/pyclass.rs`
- Test: `cargo test --manifest-path third_party/pyo3-fork/Cargo.toml -p pyo3 --test test_inheritance --no-run`
- Test: `cargo test --manifest-path third_party/pyo3-fork/Cargo.toml -p pyo3 --test test_pyfunction --no-run`

- [ ] **Step 1: Keep the failing upstream class gates visible**

Run:

```bash
cargo test --manifest-path third_party/pyo3-fork/Cargo.toml -p pyo3 --test test_inheritance --no-run --message-format short
cargo test --manifest-path third_party/pyo3-fork/Cargo.toml -p pyo3 --test test_pyfunction --no-run --message-format short
```

Expected:

```text
E0277: the trait bound `SubclassAble: PyClassImpl` is not satisfied
E0277: the trait bound `MyClass: PyClassImpl` is not satisfied
```

- [ ] **Step 2: Add a minimal frontend class-definition path**

In `third_party/pyo3-fork/pyo3-macros-backend/src/pyclass.rs`, ensure struct-side `#[pyclass]` generates a complete class-definition record, including "no methods yet" support.

Add a shape like:

```rust
impl ClassSpec {
    pub fn minimal_class_impl_tokens(&self, pyo3_path: &syn::Path) -> proc_macro2::TokenStream {
        let ident = &self.rust_ident;
        quote::quote! {
            impl #pyo3_path::impl_::pyclass::FrontendClassSpec for #ident {
                fn class_spec() -> #pyo3_path::backend::spec::ClassSpec<'static> {
                    #pyo3_path::backend::spec::ClassSpec {
                        name: stringify!(#ident),
                        module: None,
                    }
                }
            }
        }
    }
}
```

- [ ] **Step 3: Add a minimal runtime-side class implementation trait**

In `third_party/pyo3-fork/src/impl_/pyclass.rs`, add a frontend-facing trait that can synthesize a method-less class definition:

```rust
pub trait FrontendClassSpec {
    fn class_spec() -> crate::backend::spec::ClassSpec<'static>;
}
```

Then add a minimal "no methods" path that class creation can use without a `#[pymethods]` block.

- [ ] **Step 4: Change class creation to stop assuming methods are present**

In `third_party/pyo3-fork/src/pyclass.rs` and `src/impl_/pyclass_init.rs`, make the class-definition path consume frontend class metadata even when the method inventory is empty.

The intended code shape is:

```rust
let class_spec = <T as FrontendClassSpec>::class_spec();
let method_specs = <T as FrontendMethodInventory>::method_specs();
backend.create_class(class_spec, method_specs)
```

Where `FrontendMethodInventory` returns `&[]` for method-less classes.

- [ ] **Step 5: Run inheritance compile gate**

Run: `cargo test --manifest-path third_party/pyo3-fork/Cargo.toml -p pyo3 --test test_inheritance --no-run --message-format short`
Expected: PASS or move past `SubclassAble: PyClassImpl`

- [ ] **Step 6: Run pyfunction compile gate**

Run: `cargo test --manifest-path third_party/pyo3-fork/Cargo.toml -p pyo3 --test test_pyfunction --no-run --message-format short`
Expected: only non-plain-`#[pyclass]` failures remain

- [ ] **Step 7: Commit**

```bash
git add third_party/pyo3-fork/pyo3-macros-backend/src/pyclass.rs third_party/pyo3-fork/src/impl_/pyclass.rs third_party/pyo3-fork/src/impl_/pyclass_init.rs third_party/pyo3-fork/src/pyclass.rs
git commit -m "refactor: make pyclass independent of pymethods"
```

### Task 6: Make `#[pymethods]` Additive Instead Of Class-Defining

**Files:**
- Modify: `third_party/pyo3-fork/pyo3-macros-backend/src/pyimpl.rs`
- Modify: `third_party/pyo3-fork/pyo3-macros-backend/src/pymethod.rs`
- Modify: `third_party/pyo3-fork/src/impl_/pymethods.rs`
- Modify: `third_party/pyo3-fork/src/impl_/pyclass.rs`
- Test: `cargo test --manifest-path third_party/pyo3-fork/Cargo.toml -p pyo3 --test test_inheritance --no-run`

- [ ] **Step 1: Write the failing additive inventory expectation**

Add a one-off assertion helper in `src/impl_/pymethods.rs`:

```rust
#[cfg(test)]
fn _frontend_inventory_is_additive<T: super::pyclass::FrontendMethodInventory>() {
    let _ = T::method_specs();
}
```

Expected initial compile friction until the trait exists everywhere.

- [ ] **Step 2: Define additive method inventory**

In `third_party/pyo3-fork/src/impl_/pyclass.rs`, add:

```rust
pub trait FrontendMethodInventory {
    fn method_specs() -> &'static [crate::backend::spec::MethodSpec<'static>];
}
```

Provide the default empty implementation path via macro output for method-less classes.

- [ ] **Step 3: Make `#[pymethods]` generate only inventory**

Refactor `pyo3-macros-backend/src/pyimpl.rs` and `src/pymethod.rs` so they emit method inventory/spec data and wrapper bodies, but not the existence of the class itself.

Target output shape:

```rust
impl ::pyo3::impl_::pyclass::FrontendMethodInventory for MyClass {
    fn method_specs() -> &'static [::pyo3::backend::spec::MethodSpec<'static>] {
        &[/* generated methods */]
    }
}
```

- [ ] **Step 4: Consume additive inventory during class realization**

In `src/impl_/pymethods.rs` and `src/impl_/pyclass.rs`, attach methods/getters/setters/constructors by iterating inventory collected from `FrontendMethodInventory`.

- [ ] **Step 5: Run inheritance compile gate**

Run: `cargo test --manifest-path third_party/pyo3-fork/Cargo.toml -p pyo3 --test test_inheritance --no-run --message-format short`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add third_party/pyo3-fork/pyo3-macros-backend/src/pyimpl.rs third_party/pyo3-fork/pyo3-macros-backend/src/pymethod.rs third_party/pyo3-fork/src/impl_/pymethods.rs third_party/pyo3-fork/src/impl_/pyclass.rs
git commit -m "refactor: make pymethods additive over pyclass"
```

### Task 7: Preserve Imported `wrap_pyfunction!` Through Frontend-Lowered Symbols

**Files:**
- Modify: `third_party/pyo3-fork/pyo3-macros-backend/src/pyfunction.rs`
- Modify: `third_party/pyo3-fork/src/impl_/pyfunction.rs`
- Modify: `third_party/pyo3-fork/src/types/module.rs`
- Modify: `third_party/pyo3-fork/src/lib.rs`
- Test: `cargo test --manifest-path third_party/pyo3-fork/Cargo.toml -p pyo3 --test test_pyfunction --no-run`

- [ ] **Step 1: Keep the imported-name wrapping gate red**

Use the unchanged upstream case:

```text
third_party/pyo3/tests/test_pyfunction.rs:599
let f = wrap_pyfunction!(foo, py).unwrap();
```

If the error is not currently present, keep this task focused on preserving the symbol-based frontend lowering in the forked implementation.

- [ ] **Step 2: Emit a backend-neutral global wrapper symbol**

In `third_party/pyo3-fork/pyo3-macros-backend/src/pyfunction.rs`, emit:

```rust
#[doc(hidden)]
#[unsafe(no_mangle)]
pub extern "Rust" fn __pyo3_wrap_symbol_foo(
    py: ::pyo3::Python<'_>,
) -> ::pyo3::PyObject {
    __pyo3_fn_foo(py).into_any().unbind()
}
```

Generalize this over the generated function name.

- [ ] **Step 3: Make `wrap_pyfunction!` use the symbol path**

In `third_party/pyo3-fork/src/lib.rs`, change the macro to:

```rust
unsafe {
    extern "Rust" {
        fn __pyo3_wrap_symbol_foo(py: $crate::Python<'_>) -> $crate::Py<$crate::types::PyAny>;
    }
    let obj = __pyo3_wrap_symbol_foo(py);
    Ok(obj.into_bound(py))
}
```

Generalize this with macro-generated names.

- [ ] **Step 4: Run the pyfunction compile gate**

Run: `cargo test -p pyo3-tests --test test_pyfunction --no-run --message-format short`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add third_party/pyo3-fork/pyo3-macros-backend/src/pyfunction.rs third_party/pyo3-fork/src/impl_/pyfunction.rs third_party/pyo3-fork/src/lib.rs third_party/pyo3-fork/src/types/module.rs
git commit -m "refactor: lower wrap_pyfunction through global wrapper symbols"
```

### Task 8: Preserve CPython Backend Behavior Through The New Boundary

**Files:**
- Modify: `third_party/pyo3-fork/src/backend/cpython.rs`
- Modify: `third_party/pyo3-fork/src/err/mod.rs`
- Modify: `third_party/pyo3-fork/src/instance.rs`
- Modify: `third_party/pyo3-fork/src/type_object.rs`
- Test: `cargo test --manifest-path third_party/pyo3-fork/Cargo.toml`

- [ ] **Step 1: Add a failing CPython backend adapter smoke test**

Create `third_party/pyo3-fork/src/backend/tests.rs`:

```rust
#[test]
fn cpython_backend_smoke() {
    let _ = crate::backend::cpython::CpythonBackend;
}
```

Then wire it from `src/backend/mod.rs` under `#[cfg(test)]`.

- [ ] **Step 2: Run the fork test target**

Run: `cargo test --manifest-path third_party/pyo3-fork/Cargo.toml backend::tests::cpython_backend_smoke`
Expected: FAIL until backend types are wired fully

- [ ] **Step 3: Implement the CPython backend adapter**

In `third_party/pyo3-fork/src/backend/cpython.rs`, define the reference backend around existing PyO3 internals:

```rust
pub struct CpythonBackend;

impl crate::backend::traits::Backend for CpythonBackend {
    type Interpreter = crate::Python<'static>;
    type ClassBuilder<'py> = CpythonClassBuilder<'py>;
    type FunctionBuilder<'py> = CpythonFunctionBuilder<'py>;
}
```

Use existing CPython code paths rather than inventing new semantics.

- [ ] **Step 4: Route exception and type-object code through the backend**

Adjust `src/err/mod.rs`, `src/instance.rs`, and `src/type_object.rs` so backend-specific operations are called through `CpythonBackend` rather than directly embedded throughout frontend-owned logic.

- [ ] **Step 5: Run the full fork test suite**

Run: `cargo test --manifest-path third_party/pyo3-fork/Cargo.toml`
Expected: PASS for the CPython fork suite or fail only on explicitly unfinished RustPython-gated code

- [ ] **Step 6: Commit**

```bash
git -C third_party/pyo3-fork add src/backend/cpython.rs src/backend/tests.rs src/err/mod.rs src/instance.rs src/type_object.rs
git -C third_party/pyo3-fork commit -m "refactor: preserve CPython backend through backend boundary"
```

### Task 8.5: Preserve PyPy / GraalPy Design Constraints

**Files:**
- Modify: `third_party/pyo3-fork/Cargo.toml`
- Modify: `third_party/pyo3-fork/pyo3-build-config/src/lib.rs`
- Test: `cargo check --manifest-path third_party/pyo3-fork/Cargo.toml -p pyo3-build-config`

- [ ] **Step 1: Record the design intent in code comments or docs near runtime selection**

Clarify that the reference backend is CPython-family, not CPython-only, and that existing `PyPy` / `GraalPy` cfg handling remains in scope for that backend.

- [ ] **Step 2: Verify existing config support still compiles**

Run: `cargo check --manifest-path third_party/pyo3-fork/Cargo.toml -p pyo3-build-config`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git -C third_party/pyo3-fork add Cargo.toml pyo3-build-config/src/lib.rs
git -C third_party/pyo3-fork commit -m "docs: clarify CPython-family backend scope"
```

### Task 9: Add The RustPython Backend Skeleton In The Fork

**Files:**
- Modify: `third_party/pyo3-fork/src/backend/rustpython.rs`
- Modify: `third_party/pyo3-fork/Cargo.toml`
- Modify: `third_party/pyo3-fork/src/lib.rs`
- Test: `cargo check --manifest-path third_party/pyo3-fork/Cargo.toml --features runtime-rustpython -p pyo3`

- [ ] **Step 1: Write the failing backend feature gate**

Add to `third_party/pyo3-fork/Cargo.toml`:

```toml
[features]
default = []
runtime-rustpython = []
runtime-cpython = []
```

Then, in `src/lib.rs`, add a missing import:

```rust
#[cfg(feature = "runtime-rustpython")]
pub use crate::backend::rustpython::RustPythonBackend;
```

Expected initial failure until the backend type is fully implemented.

- [ ] **Step 2: Run the failing RustPython backend check**

Run: `cargo check --manifest-path third_party/pyo3-fork/Cargo.toml --features runtime-rustpython -p pyo3`
Expected: FAIL on incomplete RustPython backend wiring

- [ ] **Step 3: Implement the backend shell**

In `third_party/pyo3-fork/src/backend/rustpython.rs`:

```rust
pub struct RustPythonBackend;

impl crate::backend::traits::Backend for RustPythonBackend {
    type Interpreter = crate::Python<'static>;
    type ClassBuilder<'py> = RustPythonClassBuilder<'py>;
    type FunctionBuilder<'py> = RustPythonFunctionBuilder<'py>;
}

pub struct RustPythonClassBuilder<'py> {
    _phantom: core::marker::PhantomData<&'py ()>,
}

pub struct RustPythonFunctionBuilder<'py> {
    _phantom: core::marker::PhantomData<&'py ()>,
}
```

- [ ] **Step 4: Run the RustPython backend check**

Run: `cargo check --manifest-path third_party/pyo3-fork/Cargo.toml --features runtime-rustpython -p pyo3`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git -C third_party/pyo3-fork add Cargo.toml src/lib.rs src/backend/rustpython.rs
git -C third_party/pyo3-fork commit -m "feat: add RustPython backend skeleton"
```

### Task 10: Rewire Local Validation To The Fork And Re-Run The Ladder

**Files:**
- Modify: `examples/pyo3-tests/Cargo.toml`
- Modify: `docs/upstream-packages.md`
- Test: `cargo test -p pyo3-tests --test test_inheritance --no-run`
- Test: `cargo test -p pyo3-tests --test test_pyfunction --no-run`
- Test: `cargo run -p jsonschema-rs`

- [ ] **Step 1: Switch local upstream tests to the fork checkout**

Update `examples/pyo3-tests/Cargo.toml`:

```toml
[[test]]
name = "test_pyfunction"
path = "../../third_party/pyo3-fork/tests/test_pyfunction.rs"

[[test]]
name = "test_inheritance"
path = "../../third_party/pyo3-fork/tests/test_inheritance.rs"
```

- [ ] **Step 2: Run the compile gates against the fork**

Run:

```bash
cargo test -p pyo3-tests --test test_inheritance --no-run --message-format short
cargo test -p pyo3-tests --test test_pyfunction --no-run --message-format short
```

Expected: PASS

- [ ] **Step 3: Re-run the package ladder smoke gate**

Run: `cargo run -p jsonschema-rs`
Expected: PASS

- [ ] **Step 4: Update package workflow docs**

Add to `docs/upstream-packages.md`:

```md
## PyO3 Fork Validation

The PyO3 fork in `third_party/pyo3-fork` is now the architectural source of truth for backend work.

- upstream PyO3 tests must be runnable unchanged from the fork
- package examples in this repo remain secondary validation
- CPython-backend compatibility is release-blocking for the fork architecture branch
```

- [ ] **Step 5: Commit**

```bash
git add examples/pyo3-tests/Cargo.toml docs/upstream-packages.md
git commit -m "test: validate local gates against PyO3 fork"
```

## Self-Review

### Spec coverage

- frontend/backend split: covered by Tasks 3, 4, 5, 6, 8, 9
- fork-first execution model: covered by Tasks 1 and 3
- CPython backend preservation: covered by Task 8 and validation steps in Task 10
- RustPython backend motivation: covered by Task 9 and Task 10
- unchanged upstream test sources: covered by Tasks 2, 5, 6, 7, 10
- package-level validation: covered by Task 10

### Placeholder scan

- no `TBD`, `TODO`, or deferred “implement later” steps remain
- each task names exact files and commands
- each code-editing task includes concrete code shapes

### Type consistency

- backend modules are consistently named `src/backend/{mod,traits,cpython,rustpython,spec}.rs`
- frontend semantic traits are consistently named `FrontendClassSpec` and `FrontendMethodInventory`
- validation commands consistently target `third_party/pyo3-fork` or the local `pyo3-tests` harness
