---
id: CSP-711
title: Retire the hidden Union and Forks views
status: To Do
assignee: []
created_date: '2026-10-27 20:26'
labels:
  - tui-framework
  - tuifw-cleanup
milestone: m-25
dependencies: []
documentation:
  - docs/plans/tui-framework/inventory/02-left-pane-tree.md
priority: medium
type: chore
ordinal: 797000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
`View::Union` and `View::Forks` are reachable only through `--view`. Both accept groupings that their builders ignore, Union shows `—` for every age because it passes `now=None`, and Forks has no tests (inventory slice 02, section 5). They would otherwise have to be carried into the column schema migration for no user benefit. The operator chose to retire them rather than repair them.

Scope: Remove the two view variants, their row builders (`rows/union.rs`, `rows/forks.rs`), their grouping enums, and their `--view` and config values. A stale `--view union` or config value reports a clear diagnostic that lists the valid views instead of failing obscurely. Update `docs/operations.md` and `docs/tui-columns.md` if they mention either view.

TUI changes:
- Consistency wins: the view switcher, `--view`, and the column reference all describe the same set of views
- Regressions or behavior changes: `--view union` and `--view forks` stop working (hidden, never in the view switcher)

Tests and checks: The `tests/` CLI tests and `consistency_tests/docs.rs` (view and row-kind headings in `docs/tui-columns.md`) pass. A test asserts the diagnostic text for each removed value.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 `View::Union`, `View::Forks`, their builders, grouping enums, and tests are removed
- [ ] #2 `--view union`, `--view forks`, and the equivalent config values produce a diagnostic that names the valid views
- [ ] #3 `docs/operations.md` and `docs/tui-columns.md` no longer mention the two views, and the doc-sync tests pass
- [ ] #4 `RowKind` variants used only by the two views are removed
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
