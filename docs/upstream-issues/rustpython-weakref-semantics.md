# RustPython weakref semantics blocker

Upstream issue: https://github.com/RustPython/RustPython/issues/7589

Current state in the PyO3 RustPython backend:
- switching from stdlib `weakref` to built-in `_weakref` removes the embedded import-recursion blocker
- `PyWeakrefReference::upgrade()` works in isolated cases
- the remaining weakref failures are now semantic mismatches in RustPython weakref behavior rather than generic backend bring-up

Observed gaps:
- `ReferenceType.__callback__` behavior does not match CPython / PyO3 expectations
- generic `PyWeakref` upgrade paths do not uniformly recover referents across reference/proxy cases
- proxy weakref behavior still diverges for both Python classes and PyO3 pyclasses

The affected PyO3 tests are currently marked ignored under `PyRustPython` and should be revisited once RustPython/RustPython#7589 is fixed.
