---
id: CSP-718
title: Guard the framework's independence from Conspectus
status: Done
assignee:
  - '@claude'
created_date: '2026-10-27 20:26'
updated_date: '2026-10-28 03:26'
labels:
  - tui-framework
  - tuifw-workspace
milestone: m-25
dependencies:
  - CSP-717
documentation:
  - docs/adr/0150-tui-framework-workspace-crate.md
priority: high
type: task
ordinal: 804000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
ADR 0150 lists independence rules and says each is enforced by a test, not by convention: the framework's manifest names no `conspectus` crate and its dependency set is an allowlist; no source, doc, or test file uses Conspectus vocabulary; the crate starts no processes and writes no files; no domain type leaks through its public API. Without these tests, a convenient `Msg` or harness color creeps back in the first time a control is awkward to make generic.

Scope: Add tests in `crates/tuikit` (or a workspace-level test) that: parse `crates/tuikit/Cargo.toml` and fail on a `conspectus` dependency or a dependency outside a documented allowlist; scan framework source, docs, and tests for a documented list of domain words (session, mux, harness, pin, worktree, PR, graph node, and the like) with a small, reviewed allowlist for exceptions; scan non-test source for process spawning and file writes (`std::process`, `Command`, `File::create`, `OpenOptions`, `fs::write`, `remove_file`), allowing read-only filesystem access for path completion; and fail when a scan visits fewer files than a stated minimum. Add a self-test that feeds each scan a deliberately leaking fixture and expects a failure.

TUI changes: none expected.

Tests and checks: Each guard fails on its deliberate-violation fixture and passes on the real tree. The documented allowlists are short and each entry has a reason.

Blockers: The workspace conversion.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 A test fails when `crates/tuikit/Cargo.toml` depends on `conspectus` or on a crate outside the documented allowlist
- [x] #2 A test fails when framework source, docs, or tests use a listed Conspectus domain word outside the reviewed allowlist
- [x] #3 A test fails when framework non-test source spawns a process or writes a file
- [x] #4 Each scan has a tripwire that fails if it visits fewer files than a stated minimum
- [x] #5 A deliberate-violation fixture proves each guard can fail
- [x] #6 The guard rules and how to request an allowlist entry are written in the framework's contribution guide
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Add `toml` (workspace-shared, already in Cargo.lock) as a dev-dependency of `tuikit` so the manifest guard parses TOML instead of guessing; no new package enters the lockfile. Record the choice in the Cargo.toml comment and the guard's module docs.
2. Write `crates/tuikit/tests/independence.rs`: module docs carry the guard rules and the allowlist-request process (CSP-720 moves them into `docs/contributing.md`); one documented allowlist constant per guard, each entry with a reason.
3. Manifest guard: `check_manifest(text, allowlist)` parses every dependency table (plain, dev, build, target-specific) and fails on `conspectus` (by key or `package =` rename) or any name outside the allowlist.
4. Vocabulary guard: `scan_vocabulary(files, words, exceptions)` tokenizes case-insensitively on word boundaries (camelCase and snake_case split, plural tolerant, multi-word terms), over every file in the crate except this guard file; reviewed exceptions are (path, word, reason) and stale exceptions fail.
5. Process/write guard: `scan_capabilities(files, exceptions)` strips comments, string literals, and `#[cfg(test)]` blocks, then flags process, filesystem-write, and rename/delete needles while allowing read-only filesystem access; test-only files (`*_tests.rs`, `tests.rs`) are skipped.
6. Each scan returns its visited-file count; a tripwire asserts a stated minimum (1 for now). Self-tests feed every guard an in-memory leaking snippet and assert it fails, plus clean and exception cases, so no leaking fixture lives on disk.
7. Verify: fmt, clippy --workspace, isolated full test run, doc build with -D warnings, `git diff --check`; temporarily plant a violation on disk to prove the real scans catch it, then remove it.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented `crates/tuikit/tests/independence.rs` (manifest, vocabulary, and process/write guards as pure functions over in-memory sources, plus self-tests and a tripwire per scan) and added `toml` as a tuikit dev-dependency via `[workspace.dependencies]` (already in Cargo.lock; the lockfile gains only the edge). Guard rules and the allowlist-request process live in the file's module docs until the contribution guide exists. Planted real on-disk leaks (a host dependency in a nested manifest, an unlisted dev-dependency, a source file with process/write calls and a domain word, a README with domain words) and confirmed each real scan failed, then removed them.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added `crates/tuikit/tests/independence.rs`: three guards, each a pure function over in-memory sources that returns a visited-file count, plus a tripwire per scan (minimum 1 each for now).

- Manifest guard: parses every `Cargo.toml` under the crate with `toml`; fails on a `conspectus*` dependency (by key or `package =` rename, in any dependency table including target-specific) and on anything outside the documented allowlists (runtime: ratatui, crossterm, unicode-width, tui-input, ansi-to-tui, tui-markdown, comfy-table; dev: toml, insta, tui-pantry; build: none). Each entry carries a reason.
- Vocabulary guard: whole-word, case-insensitive, camelCase/snake_case/plural aware, multi-word terms (`pull request`, `graph node`). Scans every file in the crate except `tests/independence.rs` (excluded by exact name). The word list covers the host's name, work-graph objects, mux and orchestrator names, and harness names. Reviewed exceptions are per file and per term; a stale exception fails.
- Process and write guard: strips comments, literals, and `#[cfg(test)]` items, skips test files, then flags process spawning, file creation/writes/copies/renames/links/deletions, `OpenOptions`, `fs::*`, and `use std::fs::{write, copy, rename}`; read-only filesystem access passes.

Choices to review: (1) `toml` is a tuikit dev-dependency via `[workspace.dependencies]`; it is already in Cargo.lock, so the lockfile gains only the edge, but ADR 0152 item 6 says framework dependencies need an ADR first, so this is flagged for confirmation. (2) The word list includes "harness", which also catches the test-rig sense; CSP-732's "conformance harness" will need a different name or a reviewed exception. (3) The guard rules and the allowlist-request process live in the module docs of `independence.rs` because the contribution guide does not exist yet; CSP-720 should move them into `docs/contributing.md` (that guide is itself scanned, so it must point at `DOMAIN_TERMS` rather than quote it). The execution brief now points at the guard file.

Verification: 35 new tests, all passing; each guard fails on in-memory leaking snippets (every listed word, every needle, every manifest spelling) and on empty scans. Planted real on-disk leaks (host dependency in a nested manifest, an unlisted dev-dependency, a source file with process/write calls and a domain word, a README with domain words) and confirmed each real scan failed; planted files removed. Checks: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps`, `git diff --check` all pass; the isolated full run (`--workspace`) is 3460 passed, 0 failed, 1 ignored (baseline 3425 + 35 new); `cargo nextest run -p tuikit` runs 36 tests, all passing.

Regressions: none (no TUI change). Consistency wins: none (no TUI change).
<!-- SECTION:FINAL_SUMMARY:END -->
