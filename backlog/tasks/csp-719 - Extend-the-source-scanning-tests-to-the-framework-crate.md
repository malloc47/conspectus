---
id: CSP-719
title: Extend the source-scanning tests to the framework crate
status: Done
assignee:
  - '@claude'
created_date: '2026-10-27 20:26'
updated_date: '2026-10-28 03:47'
labels:
  - tui-framework
  - tuifw-workspace
milestone: m-25
dependencies:
  - CSP-717
documentation:
  - docs/plans/tui-framework/workspace-impact.md
  - docs/plans/tui-framework/inventory/06-interaction-conventions.md
priority: high
type: task
ordinal: 805000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Five tests walk fixed `src/` paths: `tests/test_module_hygiene.rs`, `tests/comment_hygiene.rs`, `tests/overlay_keymap_hygiene.rs`, `tests/capabilities_static.rs`, and the string-literal scan in `src/tui/consistency_tests/text.rs` (which also reads `docs/tui-columns.md` through `CARGO_MANIFEST_DIR`). When code moves into `crates/tuikit`, each would keep passing while checking nothing. That is a silent loss of coverage, not a failure, so it has to be closed before any code moves.

Scope: Make each scan cover both `src/` and `crates/tuikit/src/`, resolving paths from the workspace root rather than a single manifest directory. Give each a tripwire that fails when it finds fewer than a stated number of files. Decide for each rule whether it also applies inside the framework (test-module size and comment hygiene do; the developer-artifact text rule does; the capability manifest does not, because the framework spawns and writes nothing, which `w_guard` enforces) and write that down in the framework's contribution guide.

TUI changes: none expected.

Tests and checks: Temporarily moving a file with a violation into `crates/tuikit/src/` makes the relevant scan fail. The tripwires fail when the scanned roots are wrong.

Blockers: The workspace conversion.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Each of the five scans covers `crates/tuikit/src/` as well as `src/`, with paths resolved from the workspace root
- [x] #2 Each scan has a tripwire that fails when it visits too few files
- [x] #3 A violation placed in the framework tree is caught by each scan that applies to it
- [x] #4 The contribution guide states which repo hygiene rules apply to the framework and why the rest do not
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Add tests/support/source_tree.rs: one shared helper for every source-scanning test. It owns the workspace root (CARGO_MANIFEST_DIR of the root package), the named trees (host src, tests, examples; framework src, tests, examples) with a per-tree minimum file count, the walker (directory or single file), the tripwire (Err when a tree has fewer files than its minimum), the test-file name filter, and a planting helper that proves a scan covers every tree. The rule-to-crate table is its module doc.
2. Convert test_module_hygiene, comment_hygiene, overlay_keymap_hygiene, capabilities_static to the helper, each scanning the host trees plus the framework trees where the rule applies; each core scan takes (root, trees) so a scratch workspace can exercise it.
3. Convert the consistency text scan (src/tui/consistency_tests/text.rs) and the docs.rs doc path to the helper via a path-attribute module in consistency_tests.rs.
4. capabilities_static: assert explicitly that the framework tree holds no process, socket, signal, or write path (host rules with no allowance, plus write patterns); the shared lockfile already covers its dependencies.
5. Add tests/source_scan_roots.rs: the root is the workspace root, the tripwire fails on a wrong root, and each real tree exists.
6. Per rule, plant a violation in a scratch copy of every covered tree (permanent self-test) and, by hand, in the real crates/tuikit/src (temporary, never committed); confirm the right test fails.
7. Run fmt, clippy --workspace, the isolated runner, cargo doc, git diff --check. File a follow-up for tests/cwd_hygiene.rs, which also walks only src/.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Shared helper `tests/support/source_tree.rs` (included with `#[path]`, so the hygiene tests do not compile the fixture and replay helpers; the library's consistency tests include it the same way). Minimums: host `src` 300 (395 files), `tests` 30, `examples` 1, `src/tui` 100, `src/cli` 15, `src/tui/widgets` 10; framework `crates/tuikit/src` 1, `tests` 1, `examples` 0 (none yet). The overlay scan treats `src/tui/modal.rs` and `runtime/overlay_keys.rs` as single-file trees with a minimum of 1.
Decisions: the slot-word rule, rendered screens, help pages, and `docs/tui-columns.md` stay host-only (the framework cannot name a pin; the screens exercise the application). The capability check asserts the framework has no process, socket, signal, or write path with no per-file allowance; the shared lockfile already puts its dependencies under the networking-crate check. The overlay scan walks all of `crates/tuikit/src`; the future key-table model file has to be listed in `KEY_MODEL_FILES` with a reason.
Verified: a temporary `crates/tuikit/src/violation.rs` holding a backlog ID in a comment, a hand-matched `KeyCode`, a hand-written key hint, an ADR citation in a string, `Command::new`, `std::fs::write`, and a 107-line `mod tests` failed exactly the matching test in each of the five scans (file removed, never committed). Hiding `lib.rs` (framework tree empty) failed every scan with the tripwire message.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
All five source-scanning tests now cover `crates/tuikit/src` as well as `src`, resolved from the workspace root through one shared helper, `tests/support/source_tree.rs`. The helper holds the named trees with a per-tree minimum file count (the tripwire; host `src` 300, framework `src` 1, each documented with how to raise it), the walker, and `assert_scan_covers_every_tree`, which plants a violation in a scratch copy of every tree a scan names. Every scan has such a permanent self-test, and `tests/source_scan_roots.rs` checks the helper (root is the workspace root; wrong root or short tree trips; single-file trees must exist).

Per rule: test-module size, comment hygiene (src, tests, examples of both crates), the developer-artifact string rule, and the overlay key rule apply to the framework. The overlay scan walks all of `crates/tuikit/src`; a future file that defines the key-table model itself is listed in `KEY_MODEL_FILES` with a reason (CSP-728 will hit this). The capability check does not give the framework a manifest entry; it asserts the framework tree has no process, socket, signal, or write path, with no per-file allowance. The slot-word rule, rendered screens, help pages, and the `tui-columns.md` checks stay host-only.

Verification: a temporary violating file in `crates/tuikit/src` failed the matching test in each of the five scans and was removed; emptying the framework tree failed every scan with the tripwire message. `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps`, `git diff --check` pass; the isolated full run is 3478 passed, 0 failed, 1 ignored (baseline 3460 plus 18 new tests).

For operator review: AC 4 asks the contribution guide to state which rules apply. The guide does not exist until CSP-720, so the rule-to-crate table is the module doc of `tests/support/source_tree.rs` and CSP-720 carries a note to move it into `crates/tuikit/docs/contributing.md`. Follow-up filed: CSP-767 (`tests/cwd_hygiene.rs` also walks only `src`).

Regressions: none (no visible TUI change).
Consistency wins: none (no visible TUI change).
<!-- SECTION:FINAL_SUMMARY:END -->
