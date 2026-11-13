---
id: CSP-767
title: Extend the working-directory guard to the framework crate
status: Done
assignee:
  - '@claude'
created_date: '2026-10-28 03:40'
updated_date: '2026-10-28 04:12'
labels:
  - tui-framework
  - tuifw-workspace
milestone: m-25
dependencies:
  - CSP-719
priority: medium
ordinal: 853000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
`tests/cwd_hygiene.rs` fails when code other than `src/cwd.rs` reads the process working directory (ADR 0111), but it walks only `src`. Code that moves into `crates/tuikit` leaves its scope, so a framework control could read the working directory unchecked. CSP-719 added the shared scan plumbing (`tests/support/source_tree.rs`) that the other source-scanning tests now use; this guard was not among the five it covered.

Decide whether the rule applies to the framework (a path-completion control must take its directory from the host rather than read it) and, if so, route the guard through the shared plumbing with the framework tree and a tripwire.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 tests/cwd_hygiene.rs scans crates/tuikit/src as well as src, resolved from the workspace root, or the decision not to is written in the rule table in tests/support/source_tree.rs
- [x] #2 The scan fails when it visits fewer files than a stated minimum for each tree
- [x] #3 A working-directory read planted in crates/tuikit/src is reported
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Decide: the rule applies to the framework. A control that needs a directory (path completion) takes it from the host as an input; the process working directory can vanish while the program runs, so reading it is a defect in any code. The framework has no `cwd` helper module, so it gets no exemption.
2. Rewrite `tests/cwd_hygiene.rs` to scan `HOST_SRC` and `FRAMEWORK_SRC` from the workspace root through `tests/support/source_tree.rs` (tripwire minimums come with the trees), with the two-file exemption applying to the host paths only and findings printed as workspace-relative paths.
3. Add a self-test built on `assert_scan_covers_every_tree` that plants a working-directory read in a scratch copy of both trees, plus a test that an exempt-named file in the framework tree is still reported.
4. Add the rule row to the table in `tests/support/source_tree.rs` module docs and a row in the "Repository-wide hygiene scans" table of `crates/tuikit/docs/contributing.md`, in words the independence guard allows.
5. Run fmt, clippy --workspace, the isolated suite, doc build, `git diff --check`.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Decision: the rule applies to the framework. A path-completion control takes its directory from the host as an input, so no framework file is exempt. Recorded in the rule table in `tests/support/source_tree.rs` and in the contribution guide's hygiene-scan table.
Validation: fmt, clippy --workspace -D warnings, doc build with RUSTDOCFLAGS -D warnings, `git diff --check`, and the isolated suite (3483 passed, 0 failed, 1 ignored; the first run hit a transient "Text file busy" in a fake-gh test, unrelated, and passed on rerun). Also planted a real `current_dir()` read in `crates/tuikit/src` and confirmed the guard reported it, then removed it.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`tests/cwd_hygiene.rs` now scans `HOST_SRC` and `FRAMEWORK_SRC` (`crates/tuikit/src`) through `tests/support/source_tree.rs`, resolved from the workspace root. Decision: the rule applies to the framework, because a control that needs a directory (path completion) takes it from its host rather than reading the process working directory. Only `src/cwd.rs` and `src/cwd_tests.rs` are exempt, matched by workspace-relative path, so a framework file named `cwd.rs` is still scanned. The failure message now also tells framework code to take the directory from its host.

Acceptance criteria: (1) the scan covers both trees; the rule-to-crate row is in the table in `tests/support/source_tree.rs` and in `crates/tuikit/docs/contributing.md` (the guide uses no host vocabulary; the independence guard passes). (2) The tripwire minimums come with the shared `Tree` constants (300 host, 1 framework), covered by `tests/source_scan_roots.rs`. (3) `a_working_directory_read_is_caught_in_every_tree_the_scan_covers` plants a read in a scratch copy of both trees via `assert_scan_covers_every_tree`; `only_the_hosts_own_cwd_files_are_exempt` proves the exemption is host-only. A read planted in the real `crates/tuikit/src` was also reported, then removed.

Verified with fmt, clippy --workspace --all-targets --all-features -D warnings, cargo doc with RUSTDOCFLAGS -D warnings, `git diff --check`, and the isolated suite: 3483 passed, 0 failed, 1 ignored. A first run hit a transient "Text file busy" in a fake-gh forge test, unrelated, and passed on rerun.

Regressions: none (no visible TUI change).
Consistency wins: none (no visible TUI change); the working-directory rule now holds in both crates.
<!-- SECTION:FINAL_SUMMARY:END -->
