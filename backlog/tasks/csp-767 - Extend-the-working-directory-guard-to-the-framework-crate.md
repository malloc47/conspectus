---
id: CSP-767
title: Extend the working-directory guard to the framework crate
status: To Do
assignee: []
created_date: '2026-10-28 03:40'
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
- [ ] #1 tests/cwd_hygiene.rs scans crates/tuikit/src as well as src, resolved from the workspace root, or the decision not to is written in the rule table in tests/support/source_tree.rs
- [ ] #2 The scan fails when it visits fewer files than a stated minimum for each tree
- [ ] #3 A working-directory read planted in crates/tuikit/src is reported
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
