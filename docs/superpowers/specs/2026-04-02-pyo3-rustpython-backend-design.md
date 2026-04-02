# PyO3 RustPython Backend — Design Spec

## Goal

Add RustPython as an alternative backend to PyO3, selected at build time via a `runtime-rustpython` feature flag. Any PyO3 crate should be able to compile against RustPython with zero source changes — just `features = ["runtime-rustpython"]` in Cargo.toml.

**Validation targets (in order):**
1. orjson — simple, well-known pyo3 crate
2. pydantic-core — the Big Boss

**Long-term:** upstream as a PR to the official PyO3 project.

## Architecture

### Crate structure

Fork the pyo3 repo. Add `runtime-rustpython` feature. No separate crate — one repo, two backends.

```
pyo3/                              (forked repo)
├── pyo3/                          (main crate)
│   ├── Cargo.toml                 (+ optional dep on rustpython-vm)
│   └── src/
│       ├── lib.rs                 (cfg-gates between backends)
│       ├── impl_/
│       │   ├── cpython/           (existing — untouched)
│       │   └── rustpython/        (new — VM wrappers, trait impls)
│       ├── types/                 (PyDict, PyList, etc.)
│       │   ├── dict.rs            (cfg-gated: cpython vs rustpython impl)
│       │   └── ...
│       └── conversions/           (FromPyObject impls — cfg-gated)
│
├── pyo3-macros/                   (thin proc-macro — unchanged)
├── pyo3-macros-backend/
│   └── src/
│       ├── ...                    (existing parse + IR — unchanged)
│       └── codegen/
│           ├── cpython.rs         (existing codegen, extracted)
│           └── rustpython.rs      (new codegen targeting rustpython-derive macros)
│
├── pyo3-ffi/                      (CPython C API — skipped when runtime-rustpython)
└── pyo3-build-config/             (extended to detect rustpython)
```

### Build-time selection

```toml
# Downstream Cargo.toml — RustPython backend
[dependencies]
pyo3 = { version = "0.24", features = ["runtime-rustpython"] }

# Default — CPython, unchanged behavior
pyo3 = "0.24"
```

### Upstream PR structure

- No modifications to existing CPython code paths — only additions
- All RustPython code behind `#[cfg(feature = "runtime-rustpython")]`
- Existing tests remain unchanged and passing
- New test suite gated on the feature
- Clear module boundaries: `src/impl_/rustpython/`, `codegen/rustpython.rs`
- `// RUSTPYTHON-ASSUMPTION:` markers for grepability on design decisions

## Macro Backend — Codegen Layer

pyo3-macros-backend has a clean pipeline: **parse → IR → codegen**. We keep parse + IR untouched, add a RustPython codegen path alongside the existing CPython codegen.

The RustPython codegen targets **rustpython-derive's macro API** (`#[pyslot]`, `#[pymethod]`, `#[pyproperty]`, `#[pymethod(magic)]`) rather than generating expanded code directly. This keeps us insulated from RustPython internals and gives us a stable abstraction boundary.

**Note:** This is a meta-macro approach — our proc macro emits tokens that include rustpython-derive's proc macro attributes. The Rust compiler handles this correctly: proc macro output is re-parsed and any proc macro attributes in the output are expanded in a subsequent pass.

### `#[pyclass]` codegen mapping

| Concept | CPython codegen (existing) | RustPython codegen (new) |
|---|---|---|
| Type identity | `PyTypeInfo` → ffi type object pointer | `PyPayload` impl → `class_def()` |
| Memory layout | `PyCell<T>` wrapping ffi `PyObject` | Struct implements `PyPayload` |
| Type registration | `LazyTypeObject`, slot arrays | `T::make_class(&vm.ctx)` |
| Constructor (`#[new]`) | `__new__` slot in type object | `#[pyslot] fn slot_new()` |
| Methods | `PyMethodDef` arrays → method table | `#[pymethod]` fns |
| Getters/setters | Descriptor slot functions | `#[pyproperty]` / `#[pyproperty(setter)]` |
| Magic methods | Protocol slot functions | `#[pymethod(magic)]` |

### Example expansion

User writes:
```rust
#[pyclass]
struct Point {
    #[pyo3(get, set)]
    x: f64,
    #[pyo3(get, set)]
    y: f64,
}

#[pymethods]
impl Point {
    #[new]
    fn new(x: f64, y: f64) -> Self { Point { x, y } }
    fn distance(&self) -> f64 { (self.x*self.x + self.y*self.y).sqrt() }
    fn __repr__(&self) -> String { format!("Point({}, {})", self.x, self.y) }
}
```

RustPython codegen produces (conceptually):
```rust
#[rustpython_derive::pyclass(name = "Point")]
impl rustpython_vm::PyPayload for Point {
    fn class(ctx: &rustpython_vm::Context) -> &'static Py<PyType> { ... }
}

#[rustpython_derive::pymethods]
impl Point {
    #[pyslot]
    fn slot_new(cls: PyTypeRef, x: f64, y: f64, vm: &VirtualMachine) -> PyResult<PyRef<Self>> {
        Point { x, y }.into_ref_with_type(vm, cls)
    }

    #[pyproperty]
    fn x(&self) -> f64 { self.x }
    #[pyproperty(setter)]
    fn set_x(&mut self, value: f64) { self.x = value; }

    #[pymethod]
    fn distance(&self) -> f64 { (self.x*self.x + self.y*self.y).sqrt() }

    #[pymethod(magic)]
    fn repr(&self) -> String { format!("Point({}, {})", self.x, self.y) }
}
```

## Runtime Layer — Types, Traits, Conversions

### Core smart pointers

| pyo3 type | RustPython backing | Notes |
|---|---|---|
| `Python<'py>` | ZST (`PhantomData<&'py VirtualMachine>`), VM via thread-local | Already implemented |
| `Bound<'py, T>` | ZST `Python<'py>` + `PyObjectRef` — same layout as `Py<T>` | Already implemented (basic) |
| `Py<T>` | `PyObjectRef` + `PhantomData<T>` | Already implemented (basic) |
| `PyObject` | Type alias for `Py<PyAny>` | Needs adding |

### Conversion traits

```rust
pub trait FromPyObject<'py>: Sized {
    fn extract_bound(obj: &Bound<'py, PyAny>) -> PyResult<Self>;
}

pub trait IntoPyObject<'py> {
    type Target;
    fn into_pyobject(self, py: Python<'py>) -> PyResult<Bound<'py, Self::Target>>;
}
```

Internally delegate to `rustpython_vm::convert::TryFromObject` and `ToPyObject`. Implement for: `i8`–`i64`, `u8`–`u64`, `f32`/`f64`, `bool`, `String`, `&str`, `Vec<T>`, `HashMap<K,V>`, `Option<T>`, `()`, tuples.

### `#[derive(FromPyObject)]`

pyo3 supports deriving `FromPyObject` on enums and structs for automatic extraction from Python objects. pydantic-core uses this heavily. The derive macro inspects field types and generates `extract()` calls for each. The RustPython codegen path generates the same trait impl, but using our `FromPyObject` trait backed by RustPython's type system. Since `#[derive(FromPyObject)]` is purely a trait-impl generator (no FFI), the codegen difference is minimal — it just needs our `FromPyObject` + `Bound` types to be in scope.

### Python type wrappers

Each is a marker type with a `Py*Methods` trait:

| pyo3 type | RustPython backing | Key methods |
|---|---|---|
| `PyDict` | `builtins::PyDict` | `get_item`, `set_item`, `keys`, `values`, `iter` |
| `PyList` | `builtins::PyList` | `get_item`, `append`, `len`, `iter` |
| `PyTuple` | `builtins::PyTuple` | `get_item`, `len`, `iter` |
| `PyString` | `builtins::PyStr` | `to_str`, `to_string_lossy` |
| `PyBytes` | `builtins::PyBytes` | `as_bytes` |
| `PyBool` | `builtins::PyInt` (0/1) | `is_true` |
| `PyFloat` | `builtins::PyFloat` | `value` |
| `PyInt`/`PyLong` | `builtins::PyInt` | `extract` to Rust int types |
| `PyType` | `builtins::PyType` | `name`, `is_subclass` |
| `PyIterator` | Protocol-based | `next` |
| `PyNone` | `vm.ctx.none` | `is_none` |
| `PySet` | `builtins::PySet` | `add`, `contains`, `len` |
| `PyFrozenSet` | `builtins::PyFrozenSet` | `contains`, `len` |
| `PyModule` | `builtins::PyModule` | Already partially done |

### `PyAnyMethods`

The universal object interface on `Bound<'py, PyAny>`:

- Attribute access: `getattr`, `setattr`, `delattr`, `hasattr`
- Calling: `call`, `call0`, `call1`, `call_method`, `call_method0`, `call_method1`
- Type ops: `extract`, `downcast`, `downcast_into`, `is_instance_of`, `get_type`
- Comparison: `eq`, `ne`, `lt`, `le`, `gt`, `ge`, `rich_compare`, `is`
- Conversion: `repr`, `str_`, `hash`, `len`, `is_truthy`, `is_none`
- Arithmetic: `add`, `sub`, `mul`, `div`, `neg`, `pos`, `abs`, etc.

Each delegates to the corresponding `VirtualMachine` or `PyObjectRef` method.

## Exception Hierarchy

~40 exception types in `pyo3::exceptions::*`. Each is a zero-sized struct implementing `PyTypeInfo` that looks up its RustPython counterpart from `vm.ctx.exceptions`:

```rust
macro_rules! impl_exception {
    ($name:ident, $vm_accessor:ident) => {
        pub struct $name;
        impl $name {
            pub fn new_err(msg: impl Into<String>) -> PyErr {
                PyErr::new::<$name, _>(msg)
            }
        }
        impl PyTypeInfo for $name {
            fn type_object(py: Python<'_>) -> Bound<'_, PyType> {
                // vm.ctx.exceptions.$vm_accessor
            }
        }
    };
}

impl_exception!(PyValueError, value_error);
impl_exception!(PyTypeError, type_error);
impl_exception!(PyKeyError, key_error);
// ... ~35 more
```

**`create_exception!` macro** generates a struct + `PyTypeInfo` impl that creates a new exception subclass via `vm.ctx.new_exception_type()`.

**`PyErr`** needs to grow:
- `PyErr::new::<T, A>(args)` — create from exception type + args
- `PyErr::from_value(obj)` — wrap existing Python exception
- `PyErr::matches(py, type)` — check exception type
- `PyErr::value(py)` → `Bound<'_, PyAny>` — get the exception object
- `PyErr::traceback(py)` — get traceback
- `PyErr::is_instance_of::<T>(py)` — type check

## GIL & Threading

// RUSTPYTHON-ASSUMPTION: single-threaded RustPython
//
// RustPython does not have a GIL because it is single-threaded. All GIL-related
// APIs are implemented as no-ops or thin wrappers. If RustPython gains threading
// support in the future, this module will need a real synchronization mechanism.
// Specifically:
//   - `Python::with_gil` would need an actual lock
//   - `Py<T>` Send/Sync impls would need revisiting
//   - The thread-local VM approach would need per-thread VM or shared VM with locking

### `Python::with_gil`

Uses a thread-local to stash the current `&VirtualMachine` during interpreter execution.

**Critical design choice:** `Python<'py>` is a **zero-sized type** (`PhantomData<&'py VirtualMachine>`), not a wrapper around `&VirtualMachine`. The VM is accessed via `py.vm()` which reads the thread-local. This matches upstream pyo3's design where `Python<'py>` is a phantom token, and is essential because it means `Bound<'py, T>` has the same memory layout as `Py<T>`, enabling `Py::bind()` to return `&Bound` via pointer cast — a core pyo3 API pattern.

```rust
thread_local! {
    static CURRENT_VM: Cell<Option<*const VirtualMachine>> = Cell::new(None);
}

#[derive(Copy, Clone)]
pub struct Python<'py>(PhantomData<&'py VirtualMachine>); // ZST!

impl<'py> Python<'py> {
    pub fn vm(self) -> &'py VirtualMachine {
        // TLS read — very cheap
        with_current_vm(|vm| unsafe { &*(vm as *const VirtualMachine) })
    }

    pub fn with_gil<F, R>(f: F) -> R
    where F: for<'p> FnOnce(Python<'p>) -> R {
        with_current_vm(|_vm| f(Python(PhantomData)))
    }
}
```

The VM reference is set when entering user code (module exec, method calls) and cleared on exit via an RAII `VmGuard`.

### Other GIL APIs

- `Python::allow_threads(|| { ... })` — no-op, just runs the closure
- `Py<T>`: `Send + Sync` — safe because single-threaded
- `Ungil` trait — all types satisfy it trivially

## Phased Rollout

| Phase | Goal | Validation |
|---|---|---|
| 0 | Fork & scaffold, cfg-gating skeleton | Hello example compiles and runs with `runtime-rustpython` |
| 1 | `#[pyclass]` / `#[pymethods]` codegen | `Point` class: constructor, getters/setters, `__repr__`, methods |
| 2 | Runtime layer: types, traits, conversions, exceptions, GIL stubs | Increasingly complex examples: collections, error handling, class hierarchies |
| 3 | First real-world target: **orjson** | orjson compiles and passes its test suite in RustPython |
| 4 | **pydantic-core** compilation | Iterative, compile-error-driven. Track "X of Y source files compile" |
| 5 | Pydantic Python test suite | Run pydantic's tests inside RustPython. Track pass/fail/skip |
| 6 | Sister project migration (no rush) | numpy-rust, pandas-rust, etc. switch to pyo3 with `runtime-rustpython` |

## Open Questions

- **pyo3 version to fork:** 0.24.x is current stable. Pin to a specific release.
- **rustpython-derive version coupling:** Our codegen targets rustpython-derive's macro API. If that API changes, our codegen needs updating. Pin the rustpython rev (already done: `f9ca63893`).
- **Features we can stub/skip:** Some pyo3 features are CPython-only (`PyBuffer`, `PyMemoryView`, `PyGcProtocol`). These can return `unimplemented!()` or compile-error behind the rustpython feature for now.
