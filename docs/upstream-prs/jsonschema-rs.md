# jsonschema-rs Upstream Patch Tracking

## Local checkout

- path: `third_party/jsonschema-rs`
- upstream: `https://github.com/Stranger6667/jsonschema-rs`
- fork: pending

## Candidate patches

### Patch 1: Replace raw `PyDictObject.ma_used` access

- local file: `third_party/jsonschema-rs/crates/jsonschema-py/src/ser.rs`
- reason: assumes CPython object layout through `pyo3::ffi`, outside the portable PyO3 abstraction we want to support on RustPython
- proposed change: use `PyDict_Size` instead of raw field access
- status: validated locally; this patch takes the upstream-subrepository harness from `25 passed, 1 failed` to `26 passed, 0 failed`
- PR: pending

### Patch 2: RustPython-only bootstrap stubs carried by the old copied snapshot

- local files:
  - `third_party/jsonschema-rs/crates/jsonschema-py/Cargo.toml`
  - `third_party/jsonschema-rs/crates/jsonschema-py/src/lib.rs`
- reason: local RustPython integration currently needs package-side wiring changes that are not suitable for an upstream `jsonschema-rs` PR as-is
- status: local-only fork changes; not PR candidates in current form
- PR: pending

## Notes

- The current local `examples/jsonschema-rs` tree is a stale copied snapshot and should be removed once the harness points at `third_party/jsonschema-rs`.
- Package-source fixes must live in the sub-repository branch history, not under `examples/`.
