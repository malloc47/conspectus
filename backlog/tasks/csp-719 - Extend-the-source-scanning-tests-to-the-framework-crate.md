---
id: CSP-719
title: Extend the source-scanning tests to the framework crate
status: To Do
assignee: []
created_date: '2026-10-27 20:26'
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
- [ ] #1 Each of the five scans covers `crates/tuikit/src/` as well as `src/`, with paths resolved from the workspace root
- [ ] #2 Each scan has a tripwire that fails when it visits too few files
- [ ] #3 A violation placed in the framework tree is caught by each scan that applies to it
- [ ] #4 The contribution guide states which repo hygiene rules apply to the framework and why the rest do not
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
