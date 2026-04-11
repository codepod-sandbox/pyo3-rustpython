# Upstream Package Workflow

## Rule

Package source code under upstream checkouts is not edited directly in this repository unless the change is being prepared as an upstreamable patch and tracked in the package's own git history.

## Allowed Local Changes

- `pyo3-rustpython` compatibility layer code under `crates/`
- local harness crates under `examples/`
- package configuration overlays when they live in local harness crates
- one-line interpreter-builder import swaps in local harnesses when needed
- the already-approved jsonschema stubs until they are replaced by upstream-checkout patches or removed

## Disallowed Local Changes

- editing copied package source in `examples/` as if it were local project code
- silent hotfixes to package internals with no upstream branch or PR trail
- mixing compatibility-layer fixes with package-fork fixes in the same commit

## Upstream Package Inventory

| Package | Local checkout | Upstream repo | Validation harness | Tracking |
| --- | --- | --- | --- | --- |
| blake3-py | `third_party/blake3-py` | `oconnor663/blake3-py` | `examples/blake3` | pending |
| jiter | `third_party/jiter` | `pydantic/jiter` | `examples/jiter` | pending |
| rpds-py | `third_party/rpds-py` | `crate-py/rpds` | `examples/rpds` | `docs/upstream-prs/rpds-py.md` |
| jsonschema-rs | `third_party/jsonschema-rs` | `Stranger6667/jsonschema-rs` | `examples/jsonschema-rs` | `docs/upstream-prs/jsonschema-rs.md` |

## PR Tracking

Every package-source change must have:

- a branch in the package checkout
- a matching tracking note in `docs/upstream-prs/<package>.md`
- an upstream PR URL once opened

## Final Upstreaming Sequence

1. Conquer representative packages through the upstream-checkout workflow.
2. Open upstream package PRs for any package-source fixes.
3. Open the PyO3 PR for RustPython backend support.
4. Comment on RustPython issue `#3016` with the PyO3 PR link and package evidence.
5. Let PyO3 and RustPython maintainers decide integration shape from there.
