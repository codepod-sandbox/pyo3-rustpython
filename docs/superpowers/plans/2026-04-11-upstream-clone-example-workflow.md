# Upstream Clone Example Workflow Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace copied vendor example packages with upstream checkouts plus local harness/config overlays, and establish a strict fork/PR workflow for any package-source changes.

**Architecture:** Keep `pyo3-rustpython` responsible only for the compatibility layer and harnesses. Keep upstream packages in their own git repositories inside this repo as sub-repositories, and route all package-source fixes through fork branches and upstream PRs instead of silently editing copied source.

**Tech Stack:** Cargo workspaces, git submodules or nested git clones, RustPython, PyO3 compatibility shim, GitHub forks/PRs, markdown process docs.

---

## File Structure

### Existing Files To Modify
- `Cargo.toml`
  - Stop treating copied package sources as first-class workspace code once the harnesses point at upstream checkouts.
- `.gitignore`
  - Replace the current ad hoc vendor ignore rules with explicit rules for sub-repositories or remove ignores if using tracked submodules.
- `examples/jsonschema-rs/Cargo.toml`
  - Convert dependencies to point at the upstream checkout instead of copied `examples/jsonschema-dep` paths.
- `examples/jsonschema-rs/src/main.rs`
  - Keep only the harness logic that boots RustPython and runs smoke tests.
- `examples/blake3/src/lib.rs`
  - Verify this remains a thin include wrapper into the upstream checkout, or replace it with a cleaner path if the directory layout changes.
- `docs/superpowers/plans/2026-04-11-recover-pymethods-and-resume-jsonschema.md`
  - Update references that assume copied `examples/jsonschema-*` source is the long-term model.

### Existing Files To Remove
- `examples/jsonschema-rs/src/canonical.rs`
- `examples/jsonschema-rs/src/clone.rs`
- `examples/jsonschema-rs/src/email.rs`
- `examples/jsonschema-rs/src/http.rs`
- `examples/jsonschema-rs/src/lib.rs`
- `examples/jsonschema-rs/src/regex.rs`
- `examples/jsonschema-rs/src/retriever.rs`
- `examples/jsonschema-rs/src/registry.rs`
- `examples/jsonschema-rs/src/ser.rs`
- `examples/jsonschema-rs/src/types.rs`
- `examples/jsonschema-dep/**`
- `examples/jsonschema-referencing-dep/**`
  - These files are copied vendor source today. They should disappear from the main repo once upstream checkouts are wired in.

### New Files To Create
- `.gitmodules`
  - Track upstream repositories as sub-repositories if you choose submodules.
- `third_party/jsonschema-rs/`
  - Upstream checkout of the package repository.
- `third_party/blake3-py/`
  - Upstream checkout of the package repository.
- `third_party/jiter/`
  - Upstream checkout of the package repository.
- `third_party/rpds-py/`
  - Upstream checkout of the package repository.
- `docs/upstream-packages.md`
  - Single source of truth for package provenance, local checkout paths, allowed edits, and upstream PR links.
- `docs/upstream-prs/jsonschema-rs.md`
  - Running log for package-level fixes that must be proposed upstream.

### Existing Files To Preserve As Local-Only Harness Code
- `examples/hello/**`
- `examples/point/**`
- `examples/phase2-test/**`
- `examples/blake3/src/main.rs`
- `examples/jiter/**`
- `examples/rpds/**`
- `examples/jsonschema-rs/src/main.rs`
  - These are local validation harnesses and should stay in this repo.

---

### Task 1: Document The Package Boundary

**Files:**
- Create: `docs/upstream-packages.md`
- Modify: `docs/superpowers/plans/2026-04-11-recover-pymethods-and-resume-jsonschema.md`
- Test: none

- [ ] **Step 1: Write the process document**

Create `docs/upstream-packages.md` with this exact starting structure:

```md
# Upstream Package Workflow

## Rule

Package source code under upstream checkouts is not edited directly in this repository unless the change is being prepared as an upstreamable patch and tracked in the package's own git history.

## Allowed Local Changes

- `pyo3-rustpython` compatibility layer code under `crates/`
- local harness crates under `examples/`
- package configuration overlays such as local `Cargo.toml` path overrides when they live in harness crates
- one-line interpreter-builder import swaps in local harnesses when needed
- the already-approved jsonschema stubs until they can be upstreamed or removed

## Disallowed Local Changes

- editing copied package source in `examples/`
- silent hotfixes to package internals with no upstream branch or PR
- mixing compatibility-layer fixes with package-fork fixes in the same commit

## Upstream Package Inventory

| Package | Local checkout | Upstream repo | Fork repo | Validation harness | Notes |
| --- | --- | --- | --- | --- | --- |
| blake3-py | `third_party/blake3-py` | `oconnor663/blake3-py` | TBD | `examples/blake3` | |
| jiter | `third_party/jiter` | `pydantic/jiter` | TBD | `examples/jiter` | |
| rpds-py | `third_party/rpds-py` | `crate-py/rpds` | TBD | `examples/rpds` | |
| jsonschema-rs | `third_party/jsonschema-rs` | `Stranger6667/jsonschema-rs` | TBD | `examples/jsonschema-rs` | |

## PR Tracking

- Every package-source change must have:
  - a branch in the package checkout
  - a matching tracking note in `docs/upstream-prs/<package>.md`
  - an upstream PR URL once opened
```

- [ ] **Step 2: Update the recovery plan to point at the new workflow**

Edit `docs/superpowers/plans/2026-04-11-recover-pymethods-and-resume-jsonschema.md` and replace copied-source assumptions with this short note near the top:

```md
## Package Source Boundary

`jsonschema-rs` and its dependency packages are moving to upstream checkouts under `third_party/`. Any package-source change must happen in the upstream checkout and be tracked as an upstream PR candidate; local harnesses remain under `examples/`.
```

- [ ] **Step 3: Review the document manually**

Check:
- it clearly separates shim changes from package changes
- it names every currently relevant package
- it makes the `ser.rs`-style patch policy explicit

- [ ] **Step 4: Commit**

Run:

```bash
git add docs/upstream-packages.md docs/superpowers/plans/2026-04-11-recover-pymethods-and-resume-jsonschema.md
git commit -m "docs: define upstream package workflow"
```

---

### Task 2: Introduce Sub-Repositories For Upstream Packages

**Files:**
- Create: `.gitmodules`
- Modify: `.gitignore`
- Test: `git submodule status`

- [ ] **Step 1: Add sub-repository entries**

If using git submodules, create `.gitmodules` with entries like:

```ini
[submodule "third_party/blake3-py"]
	path = third_party/blake3-py
	url = https://github.com/oconnor663/blake3-py.git
[submodule "third_party/jiter"]
	path = third_party/jiter
	url = https://github.com/pydantic/jiter.git
[submodule "third_party/rpds-py"]
	path = third_party/rpds-py
	url = https://github.com/crate-py/rpds.git
[submodule "third_party/jsonschema-rs"]
	path = third_party/jsonschema-rs
	url = https://github.com/Stranger6667/jsonschema-rs.git
```

If nested clones are chosen instead, skip `.gitmodules` and record the clone commands in `docs/upstream-packages.md`. Prefer submodules unless there is a concrete operational reason not to.

- [ ] **Step 2: Clean up ignore rules**

Update `.gitignore` from:

```gitignore
target
blake3-vendor/
jiter-vendor/
```

to either:

```gitignore
target
```

if submodules are tracked, or:

```gitignore
target
third_party/
```

if nested clones are intentionally untracked.

- [ ] **Step 3: Initialize the checkouts**

Run one of:

```bash
git submodule add https://github.com/oconnor663/blake3-py.git third_party/blake3-py
git submodule add https://github.com/pydantic/jiter.git third_party/jiter
git submodule add https://github.com/crate-py/rpds.git third_party/rpds-py
git submodule add https://github.com/Stranger6667/jsonschema-rs.git third_party/jsonschema-rs
```

or, if using nested clones:

```bash
git clone https://github.com/oconnor663/blake3-py.git third_party/blake3-py
git clone https://github.com/pydantic/jiter.git third_party/jiter
git clone https://github.com/crate-py/rpds.git third_party/rpds-py
git clone https://github.com/Stranger6667/jsonschema-rs.git third_party/jsonschema-rs
```

- [ ] **Step 4: Verify checkout state**

Run:

```bash
git submodule status
```

Expected:
- one line per package checkout
- no copied-package source still needed for validation

- [ ] **Step 5: Commit**

Run:

```bash
git add .gitmodules .gitignore third_party
git commit -m "build: add upstream package checkouts"
```

---

### Task 3: Convert Harnesses To Depend On Upstream Checkouts

**Files:**
- Modify: `examples/blake3/Cargo.toml`
- Modify: `examples/blake3/src/lib.rs`
- Modify: `examples/jiter/Cargo.toml`
- Modify: `examples/rpds/Cargo.toml`
- Modify: `examples/jsonschema-rs/Cargo.toml`
- Modify: `Cargo.toml`
- Test: `cargo check -p blake3-pyo3`, `cargo check -p jiter-pyo3`, `cargo check -p rpds-pyo3`, `cargo check -p jsonschema-rs`

- [ ] **Step 1: Point `blake3` harness at the upstream checkout**

Update `examples/blake3/src/lib.rs` from:

```rust
include!("../../../blake3-vendor/src/lib.rs");
```

to:

```rust
include!("../../../third_party/blake3-py/src/lib.rs");
```

Then review `examples/blake3/Cargo.toml` and replace any locally duplicated dependency declarations with path or version choices that match the upstream package where possible.

- [ ] **Step 2: Point `jiter` and `rpds` at standardized checkout paths**

Update `examples/jiter/Cargo.toml`:

```toml
jiter = { path = "../../third_party/jiter/crates/jiter", default-features = false, features = ["num-bigint"] }
```

Update `examples/rpds/Cargo.toml` only if it currently depends on ad hoc local copies instead of upstream checkout layout.

- [ ] **Step 3: Point `jsonschema-rs` harness at the upstream repository**

Update `examples/jsonschema-rs/Cargo.toml` from:

```toml
jsonschema = { path = "../jsonschema-dep", features = ["arbitrary-precision"] }
```

to the real path inside the upstream checkout, for example:

```toml
jsonschema = { path = "../../third_party/jsonschema-rs/crates/jsonschema", features = ["arbitrary-precision"] }
```

Also update any path dependencies for the referencing crate to point into `third_party/jsonschema-rs`.

- [ ] **Step 4: Stop listing copied vendor crates as workspace members**

Edit `Cargo.toml` and remove:

```toml
    "examples/jsonschema-dep",
    "examples/jsonschema-referencing-dep",
```

Also remove any no-longer-needed copied-package members that were only temporary local mirrors.

- [ ] **Step 5: Verify compilation**

Run:

```bash
cargo check -p blake3-pyo3
cargo check -p jiter-pyo3
cargo check -p rpds-pyo3
cargo check -p jsonschema-rs
```

Expected:
- harness crates still compile
- failures, if any, should now be real compat issues or upstream package assumptions, not broken local path wiring

- [ ] **Step 6: Commit**

Run:

```bash
git add Cargo.toml examples/blake3/Cargo.toml examples/blake3/src/lib.rs examples/jiter/Cargo.toml examples/rpds/Cargo.toml examples/jsonschema-rs/Cargo.toml
git commit -m "build: point harnesses at upstream package checkouts"
```

---

### Task 4: Remove Copied jsonschema Source From The Main Repo

**Files:**
- Delete: `examples/jsonschema-rs/src/{canonical.rs,clone.rs,email.rs,http.rs,lib.rs,regex.rs,retriever.rs,registry.rs,ser.rs,types.rs}`
- Delete: `examples/jsonschema-dep/**`
- Delete: `examples/jsonschema-referencing-dep/**`
- Modify: `examples/jsonschema-rs/src/main.rs`
- Test: `cargo run -p jsonschema-rs`

- [ ] **Step 1: Reduce `examples/jsonschema-rs` to a harness crate**

Refactor `examples/jsonschema-rs/src/main.rs` so it imports the extension module from the upstream checkout crate rather than a copied local `lib.rs`. The target state should look conceptually like:

```rust
use jsonschema_py::jsonschema_rs_module_def;
use pyo3::interp::InterpreterBuilder;
```

Use the real crate/module names from the upstream checkout. If the upstream crate does not export the module cleanly for this harness, solve that through dependency wiring or a tiny harness-side shim, not by copying source back into `examples/`.

- [ ] **Step 2: Delete the copied source trees**

Remove the copied `jsonschema-rs`, `jsonschema-dep`, and `jsonschema-referencing-dep` Rust sources from `examples/` once the harness is proven to build against `third_party/jsonschema-rs`.

- [ ] **Step 3: Verify the smoke harness still runs**

Run:

```bash
cargo run -p jsonschema-rs
```

Expected:
- the harness boots RustPython
- the same smoke tests run as before
- remaining failures are now clearly attributable to shim gaps or upstream package assumptions

- [ ] **Step 4: Commit**

Run:

```bash
git add examples/jsonschema-rs examples/jsonschema-dep examples/jsonschema-referencing-dep
git commit -m "refactor: replace copied jsonschema sources with upstream checkout"
```

---

### Task 5: Add Fork-And-PR Tracking For Package Fixes

**Files:**
- Create: `docs/upstream-prs/jsonschema-rs.md`
- Modify: `docs/upstream-packages.md`
- Test: none

- [ ] **Step 1: Create the tracking file**

Create `docs/upstream-prs/jsonschema-rs.md` with this exact template:

```md
# jsonschema-rs Upstream Patch Tracking

## Local checkout

- path: `third_party/jsonschema-rs`
- upstream: `https://github.com/Stranger6667/jsonschema-rs`
- fork: `TBD`

## Candidate patches

### Patch 1: Replace raw `PyDictObject.ma_used` access

- local file: upstream checkout `crates/.../ser.rs` or the actual path in the package
- reason: assumes CPython object layout through `pyo3::ffi`, which is outside the portable PyO3 abstraction
- proposed change: use `PyDict_Size` instead of raw field access
- status: not started
- PR: TBD

## Rules

- Do not land package-source edits in the main repo without recording them here.
- Open the upstream PR before relying on the patch long-term.
- If the patch is rejected upstream, keep it in a clearly named fork branch and record the reason.
```

- [ ] **Step 2: Cross-link the inventory**

Add a `Tracking` column or note in `docs/upstream-packages.md` that points `jsonschema-rs` to `docs/upstream-prs/jsonschema-rs.md`.

- [ ] **Step 3: Commit**

Run:

```bash
git add docs/upstream-packages.md docs/upstream-prs/jsonschema-rs.md
git commit -m "docs: track upstream package patches"
```

---

### Task 6: Reclassify The Current `required_fields` Fix

**Files:**
- Modify: `examples/jsonschema-rs/src/main.rs` if needed for diagnostics only
- Modify: `docs/upstream-prs/jsonschema-rs.md`
- Modify or revert package-source patch in `third_party/jsonschema-rs`
- Test: `cargo run -p jsonschema-rs`

- [ ] **Step 1: Remove the boundary-violating main-repo package edit**

Revert the `dict_len()` change from the copied `examples/jsonschema-rs/src/ser.rs` path by deleting the copied source entirely as part of Task 4. Do not keep package-source fixes under `examples/`.

- [ ] **Step 2: Reapply the patch only inside the upstream checkout if still needed**

Inside `third_party/jsonschema-rs`, create a branch and apply the narrow fix there:

```rust
pub(crate) unsafe fn dict_len(object: *mut pyo3::ffi::PyObject) -> usize {
    let len = pyo3::ffi::PyDict_Size(object);
    if len < 0 { 0 } else { len as usize }
}
```

Only do this if the failure is still present after Tasks 2-4 and only after recording it in `docs/upstream-prs/jsonschema-rs.md`.

- [ ] **Step 3: Verify the classification**

Run:

```bash
cargo run -p jsonschema-rs
```

Expected:
- if the upstream-checkout patch is absent, the known `required_fields` failure should reproduce
- if the patch is present in the upstream checkout, the harness should return to the previously observed all-pass state or close to it

- [ ] **Step 4: Commit**

Commit the tracking docs in the main repo separately from any package-fork branch commit.

Main repo:

```bash
git add docs/upstream-prs/jsonschema-rs.md
git commit -m "docs: classify jsonschema ffi layout patch"
```

Upstream checkout branch:

```bash
git -C third_party/jsonschema-rs add .
git -C third_party/jsonschema-rs commit -m "Use PyDict_Size instead of raw dict layout access"
```

---

### Task 7: Prepare The Upstream Narrative For PyO3 And RustPython

**Files:**
- Modify: `docs/upstream-packages.md`
- Create: `docs/superpowers/specs/2026-04-11-pyo3-rustpython-upstreaming-notes.md`
- Test: none

- [ ] **Step 1: Capture the upstream message**

Create `docs/superpowers/specs/2026-04-11-pyo3-rustpython-upstreaming-notes.md` with:

```md
# PyO3 RustPython Upstreaming Notes

## Position

`pyo3-rustpython` should be proposed as a PyO3 backend-extensibility effort for the portable PyO3 surface, not as full CPython ABI emulation.

## Evidence

- RustPython issue `#3016` explicitly invites collaboration on PyO3 support.
- RustPython PR `#7562` shows active interest in a minimal C-API layer that is thin and ABI3-oriented.
- Real package breakages split into:
  - PyO3-surface gaps that belong in the shim
  - CPython-layout assumptions that belong in upstream package cleanup or a narrowly scoped low-level compatibility layer

## Deliverables Before Opening The PyO3 PR

- stable harness workflow based on upstream package checkouts
- package patch inventory with PR links
- passing or mostly passing representative packages
- a written boundary statement about unsupported CPython-ABI assumptions
```

- [ ] **Step 2: Add the final milestone to the package workflow doc**

Append this section to `docs/upstream-packages.md`:

```md
## Final Upstreaming Sequence

1. Conquer representative packages through the upstream-checkout workflow.
2. Open upstream package PRs for any package-source fixes.
3. Open the PyO3 PR for RustPython backend support.
4. Comment on RustPython issue `#3016` with the PyO3 PR link and package evidence.
5. Let PyO3 and RustPython maintainers decide integration shape from there.
```

- [ ] **Step 3: Commit**

Run:

```bash
git add docs/upstream-packages.md docs/superpowers/specs/2026-04-11-pyo3-rustpython-upstreaming-notes.md
git commit -m "docs: capture upstreaming narrative"
```

---

## Self-Review

### Spec coverage
- The user wanted cloned upstream packages instead of copied examples: covered by Tasks 2-4.
- The user wanted package fixes routed through forks and upstream PRs: covered by Tasks 1, 5, and 6.
- The user wanted the eventual PyO3 PR and RustPython issue follow-up reflected in process: covered by Task 7.

### Placeholder scan
- No `TODO` or `TBD` implementation steps were used as substitutes for action, except where external fork URLs are not yet knowable and are explicitly marked as pending metadata.

### Type consistency
- `third_party/` is used consistently as the target checkout root throughout the plan.
- `examples/` is used consistently for harness crates only.

