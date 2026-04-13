# RustPython weakref semantics blocker

Upstream history:
- `RustPython/RustPython#7589` was resolved by merged PR `RustPython/RustPython#7590`
- that fixed the missing `ReferenceType.__callback__` property
- the remaining weakref tail is narrower and is now mostly proxy / generic-upgrade semantics

Current state in the PyO3 RustPython backend:
- switching from stdlib `weakref` to built-in `_weakref` removes the embedded import-recursion blocker
- `PyWeakrefReference::upgrade()` works in isolated cases
- the remaining weakref failures are now semantic mismatches in RustPython weakref behavior rather than generic backend bring-up

Observed remaining gaps:
- generic `PyWeakref` upgrade paths do not uniformly recover referents across reference/proxy cases
- proxy weakref behavior still diverges for both Python classes and PyO3 pyclasses

After repinning to RustPython `7e637e8cbd37a7ef01c5b0b0152d94ec82f323b2`, Python-class `PyWeakrefReference` behavior is now green again. The remaining ignored PyO3 tests should be revisited as a separate upstream weakref follow-up, not under `#7589`.
