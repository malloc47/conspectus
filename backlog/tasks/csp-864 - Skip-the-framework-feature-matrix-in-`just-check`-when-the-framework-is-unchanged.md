---
id: CSP-864
title: >-
  Skip the framework feature matrix in `just check` when the framework is
  unchanged
status: In Progress
assignee:
  - '@claude'
created_date: '2026-11-13 03:55'
updated_date: '2026-11-13 23:26'
labels:
  - h-devloop
milestone: m-11
dependencies: []
references:
  - justfile
  - crates/tuikit/docs/contributing.md
  - docs/adr/0199-tui-framework-separation-is-rehearsed-in-ci.md
priority: medium
type: chore
ordinal: 894000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
`just tuikit-features` (added to `just check` by CSP-785) builds and tests the framework alone: four clippy builds, plus two `cargo test -p tuikit` runs with its features off and with `richtext` only. The clippy legs come from cache. The two test runs, however, re-run the framework's 1,185 unit tests and recompile its doctests every time. Measured 2026-11-12, that costs 56 s per `just check` after a host-only edit, and 116 s after a framework edit.

The framework imports nothing from the host (`crates/tuikit/tests/independence.rs` guards this), so a change that touches neither the framework nor what its build reads cannot change those results. Framework tests that read host files, such as `crates/tuikit/tests/doc_sync.rs` against `docs/adr/README.md`, still run in the workspace test run, so they are not lost by the skip.

CI keeps running the full matrix on every push and pull request, so a skip only moves when a mistake surfaces, never whether it does.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 `just check` skips the matrix, and prints that it did, when neither the working tree nor the commits since the merge base with `main` touch `crates/tuikit`, the root `Cargo.toml`, `Cargo.lock`, or `rust-toolchain.toml`
- [x] #2 When that comparison cannot be made (no `main`, a shallow clone, a detached checkout without a merge base), `just check` runs the matrix
- [x] #3 `just tuikit-features` run directly always runs the full matrix, and CI runs it on every push and pull request
- [ ] #4 AGENTS.md, the dev-shell banner, and `crates/tuikit/docs/contributing.md` describe when `just check` runs the matrix
- [x] #5 An ADR records the condition; it is also recorded in the framework decision log and marked framework-owned in `docs/adr/README.md` if it changes what the framework docs ask contributors to run
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. `scripts/tuikit-unchanged.sh`: exits 0 with the reason when neither the working tree nor a commit since the merge base with `main` touches `crates/tuikit`, the root `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`, the root cargo/clippy/rustfmt configuration, or the `justfile`; exits 1 with the reason otherwise, and when the comparison cannot be made (not a work tree, shallow clone, no `main`, no merge base).
2. `justfile`: `check` calls a new `tuikit-features-if-changed` recipe that prints the skip or runs `just tuikit-features`; `just tuikit-features` itself and the CI steps are unchanged.
3. ADR 0225 records the condition. The framework's contribution guide gains a description of when the host's `just check` runs the matrix; what it asks contributors to run is unchanged, so no framework record (said in the ADR's status).
4. Banner and README describe when `just check` runs the matrix; the AGENTS.md text goes to the operator as a patch.
5. Verify each condition in scratch clones (skip, working-tree edit, committed edit, untracked file, no main, shallow clone, detached without merge base, detached with one), time `just check` with and without the skip, run the suite, checks, `just tuikit-features`, and the separation rehearsal; commit.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Landed: `scripts/tuikit-unchanged.sh` decides; `just check` calls the new `tuikit-features-if-changed` recipe, which prints `tuikit-features: skipped: <why>; `just tuikit-features` runs it` or `tuikit-features: running: <why>` and runs `just tuikit-features`. The inputs compared with the merge base of HEAD and `main` (committed, staged, unstaged, and untracked): `crates/tuikit`, `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`, plus the root `.cargo/`, `clippy.toml`, `.clippy.toml`, `rustfmt.toml`, `.rustfmt.toml` (none exist today) and the `justfile` that holds the matrix: a superset of the criterion's four, so a skip is never wider than it allows. `just tuikit-features` and the CI steps are unchanged.
Verified in scratch clones (`/tmp/csp-fu/t864.sh`, all 17 cases as expected): skip on `main` clean, on a branch with a host-only commit, after a framework edit was undone, after a Cargo.lock commit was reverted, detached at `main`; run on an unstaged and a staged framework edit, an untracked file under the crate, a committed Cargo.lock edit, a committed Cargo.toml edit, an unstaged rust-toolchain.toml edit, a detached HEAD with no merge base, no `main` branch (fresh clone, and `main` deleted), a shallow clone, and outside a git work tree. In this worktree it runs (`Cargo.toml differs from the merge base with main (e74fa4ee)`) and passed.
Measured: whole `just check` after a one-line host edit in a scratch clone whose branch touched neither the framework nor the root build files: 125 s at load 6 to 8 (nextest step 87 s with the build, matrix skipped), against 233 s with the matrix (after CSP-865) and 330 s at the start of this batch. The matrix itself took 51 to 69 s in steady state, and 164 s the first time in a worktree.
Framework record: none. What `crates/tuikit/docs/contributing.md` asks contributors to run is unchanged (the crate-alone commands before finishing a change to the crate, which always counts as touching it); the guide only gains a paragraph on when the host's `just check` runs them. ADR 0225's status says so. ADR 0225 is listed with the host's build records in the framework log's table of host records not included.
Checks: run-tests.sh 5,681 passed, 0 failed; checks.sh clean; `just tuikit-features-if-changed` ran the matrix in this worktree and passed.
WAITS ON THE OPERATOR (AC #4 for AGENTS.md, DoD #2): AGENTS.md is the agent's own instruction file, not edited on another agent's request. Proposed: append to the pre-main bullet that ends "`just tuikit-features` (CI runs the same commands)." the sentence: "`just check` runs it only when the working tree or a commit since the merge base with `main` touches `crates/tuikit` or a root file its build reads, and says so when it skips it."
<!-- SECTION:NOTES:END -->
