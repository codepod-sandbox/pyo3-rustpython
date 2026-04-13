# RustPython: spawned-thread imports recurse in importlib `_blocking_on`

## Summary

On current RustPython tip, imports executed on a spawned VM thread fail with:

- `RecursionError: in comparison`

The failure occurs in pure RustPython, outside the PyO3 fork. This blocks the
correct RustPython backend design for PyO3 because any backend model that relies
on spawned-thread interpreter execution will hit the same runtime bug.

## Environment

- RustPython checkout: local cargo git checkout pinned at `d201c48e1`
- Reproduced with bare RustPython embedding, not only through PyO3

## Minimal Rust repro

```rust
use rustpython::InterpreterBuilderExt;
use rustpython_vm::InterpreterBuilder;

fn main() {
    let interpreter = InterpreterBuilder::new().init_stdlib().interpreter();
    interpreter.enter(|vm| {
        vm.start_thread(|vm| {
            let result = vm.import("collections", 0);
            match result {
                Ok(_) => println!("collections ok"),
                Err(err) => {
                    let mut rendered = String::new();
                    let _ = vm.write_exception(&mut rendered, &err);
                    println!(
                        "import err class={} msg={}",
                        err.class().name(),
                        rendered.trim_end()
                    );
                }
            }
        })
        .join()
        .unwrap();
    });
}
```

## Observed failure

Typical traceback:

```text
Traceback (most recent call last):
  File ".../Lib/collections/__init__.py", line 29, in <module>
    import _collections_abc
  File ".../Lib/_collections_abc.py", line 35, in <module>
    from abc import ABCMeta, abstractmethod
  File ".../Lib/abc.py", line 85, in <module>
    from _abc import ...
  File "_frozen_importlib", line 1368, in _find_and_load
  File "_frozen_importlib", line 421, in __enter__
  File "_frozen_importlib", line 311, in acquire
  File "_frozen_importlib", line 170, in __enter__
RecursionError: in comparison
```

## Narrowing done

These succeed on a spawned VM thread:

- direct `dict.setdefault(...)`
- raw `_weakref.ref(...)`
- `import abc`
- `import _collections_abc`

These fail on a spawned VM thread:

- `import weakref`
- `import collections`
- `import array` (because it pulls in `collections`)

The failure is inside:

- `Lib/importlib/_bootstrap.py`
- `_BlockingOnManager.__enter__()`
- `_ModuleLock.acquire()`
- `_blocking_on` bookkeeping

## Important negative findings

- Using `vm.start_thread(...)` does **not** avoid the bug.
- A dedicated non-main thread is therefore not a valid workaround.
- Replacing `_blocking_on` with a plain dict in monkeypatch experiments did not
  eliminate the recursion, so this is not just a `_WeakValueDictionary` bug.

## Current hypothesis

RustPython's spawned-thread import/deadlock bookkeeping path is broken for
nested pure-Python imports. The bad interaction appears around `_blocking_on`
state for `_ModuleLock.acquire()`, not around raw weakrefs or simple dict ops.

## Extra quick investigation

Monkeypatching `_frozen_importlib._BlockingOnManager` in the standalone repro
changes the failure mode but does not fix it:

- if `__enter__` bypasses bookkeeping, the original recursion in
  `_BlockingOnManager.__enter__` disappears
- import then fails later because importlib still expects the bookkeeping state
- even bypassing both `__enter__` and `__exit__` only moves the recursion later
  into `_find_spec` / `_find_and_load_unlocked`

So `_BlockingOnManager` is part of the bad path, but not the full fix by itself.

This likely needs a RustPython-side fix before the PyO3 RustPython backend can
use a correct threaded runtime model.

## Impact on PyO3 backend work

This is an upstream blocker, not a PyO3-frontend/backend-split bug.

The right response is:

1. patch or report this in RustPython
2. keep the PyO3 fork design honest
3. only resume the RustPython backend runtime implementation once spawned-thread
   imports are reliable
