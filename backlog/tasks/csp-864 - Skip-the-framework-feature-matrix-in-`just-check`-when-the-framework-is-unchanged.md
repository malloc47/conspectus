---
id: CSP-864
title: >-
  Skip the framework feature matrix in `just check` when the framework is
  unchanged
status: To Do
assignee: []
created_date: '2026-11-13 03:55'
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
- [ ] #1 `just check` skips the matrix, and prints that it did, when neither the working tree nor the commits since the merge base with `main` touch `crates/tuikit`, the root `Cargo.toml`, `Cargo.lock`, or `rust-toolchain.toml`
- [ ] #2 When that comparison cannot be made (no `main`, a shallow clone, a detached checkout without a merge base), `just check` runs the matrix
- [ ] #3 `just tuikit-features` run directly always runs the full matrix, and CI runs it on every push and pull request
- [ ] #4 AGENTS.md, the dev-shell banner, and `crates/tuikit/docs/contributing.md` describe when `just check` runs the matrix
- [ ] #5 An ADR records the condition; it is also recorded in the framework decision log and marked framework-owned in `docs/adr/README.md` if it changes what the framework docs ask contributors to run
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
