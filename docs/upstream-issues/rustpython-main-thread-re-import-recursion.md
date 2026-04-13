`runtime-rustpython` currently hits a RustPython importlib recursion bug even on the main thread when embedded through the PyO3 fork.

Upstream issue: `RustPython/RustPython#7587`

## Minimal repro

In `third_party/pyo3-fork/tests/test_rustpython_runtime.rs`:

- `main_thread_can_import_re`
- `main_thread_warnings_filterwarnings_works`

Both fail under:

```bash
cargo test --manifest-path third_party/pyo3-fork/Cargo.toml \
  --no-default-features --features macros,runtime-rustpython \
  -p pyo3 --test test_rustpython_runtime
```

## Observed behavior

Direct `py.import("re")` fails with:

```text
RecursionError
Traceback (most recent call last):
  File ".../Lib/re/__init__.py", line 125, in <module>
  File ".../Lib/enum.py", line 3, in <module>
  File ".../Lib/types.py", line 11, in <module>
  File "_frozen_importlib", line 1368, in _find_and_load
  File "_frozen_importlib", line 421, in __enter__
  File "_frozen_importlib", line 311, in acquire
  File "_frozen_importlib", line 170, in __enter__
  File "_frozen_importlib", line 132, in setdefault
```

`warnings.filterwarnings(...)` fails because `_py_warnings.py` lazily imports `re`, which hits the same recursion path.

## Why this matters

This is not a PyO3 warning-API bug. It is a broader embedded RustPython import regression affecting standard-library modules on the main thread.

It blocks:

- `pyo3::err::tests::warnings`
- any RustPython backend path which relies on `re`
- lazy stdlib behavior inside `warnings`

## Current handling

The PyO3 fork should treat these as upstream RustPython expected failures for now and keep moving on other backend work.
