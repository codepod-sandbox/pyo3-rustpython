# RustPython weakref semantics blocker

Upstream history:
- `RustPython/RustPython#7589` was resolved by merged PR `RustPython/RustPython#7590`
- that fixed the missing `ReferenceType.__callback__` property
- that was only part of the full PyO3 weakref surface

Current state:
- the PyO3 RustPython backend no longer has a local weakref blocker
- `types::weakref` is green under `runtime-rustpython`
- the remaining weakref-related ignores in the fork, if any, should be treated as part of the separate embedded import-recursion family, not this issue

Resolution notes:
- switching from stdlib `weakref` to built-in `_weakref` removed the import-recursion coupling
- RustPython proxy support was extended to distinguish plain vs callable proxies
- the RustPython backend now recovers proxy referents through proxy-owned accessors
- RustPython type `repr` handling was adjusted so PyO3-created builtins-rooted heap types keep the expected module-qualified representation without changing ordinary Python heap types
