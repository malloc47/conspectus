---
id: CSP-711
title: Retire the hidden Union and Forks views
status: Done
assignee:
  - '@claude'
created_date: '2026-10-27 20:26'
updated_date: '2026-10-28 05:03'
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
- [x] #1 `View::Union`, `View::Forks`, their builders, grouping enums, and tests are removed
- [x] #2 `--view union`, `--view forks`, and the equivalent config values produce a diagnostic that names the valid views
- [x] #3 `docs/operations.md` and `docs/tui-columns.md` no longer mention the two views, and the doc-sync tests pass
- [x] #4 `RowKind` variants used only by the two views are removed
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Capture before frames (Sessions, Mux, PRs) from a binary built at HEAD.
2. Delete `rows/union.rs` and `rows/forks.rs`, `View::Union`/`View::Forks`, `UnionGrouping`/`ForksGrouping`, and the Grouping variants; delete `RowKind::Fork`, `RowId::Fork`, `ForkRow`, `ViewLabel::{Union,Forks}` and every consumer (left panel, keymap, tree, explorer nav, quick select, header, controls, natural view).
3. Remove the `--view` values; clap then rejects them and lists the valid views. Remove the config view fields; keep `[tui.views.union|forks]` as ignored markers that raise a `ConfigDiagnostic`, and reword the `default_view` diagnostic to name the six valid views (and say a removed view was removed).
4. Tests: diagnostic text for each removed value (config table, default_view, CLI `tui` and `dev scenario tui`), stale persisted state, update doc-sync enumerations.
5. Docs: operations.md, tui-columns.md, design.md, dev-scenarios.md, CHANGELOG, ADR 0031/0114 status notes.
6. Run fmt, clippy, the isolated suite, rustdoc, `git diff --check`; capture after frames and diff against before.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Removed `View::Union`/`View::Forks`, `rows/union.rs`, `rows/forks.rs`, `UnionGrouping`/`ForksGrouping`, `RowKind::Fork`, `RowId::Fork`, `ForkRow`, `ViewLabel::{Union,Forks}`, the two `ViewFlag` values, and the matching consumers (left panel, keymap, tree row match, explorer nav, quick-select corpus and glyph map, header and controls labels, `natural_view`, persisted-state names). `--view union|forks` now gets clap value error listing the valid views. `default_view` names the six valid views and says a removed view was removed. `[tui.views.union]` and `[tui.views.forks]` are kept as ignored markers that raise a `ConfigDiagnostic`. Stale `last_view` and `view_states` entries were already skipped and now have a test. Out of scope, filed as CSP-769: a live fork has no view, so quick select and "Reveal in its view" cannot open one that is not reachable by a relationship path. Validation: fmt, clippy (workspace, all targets, all features, -D warnings), rustdoc -D warnings, the whitespace diff check, and the isolated suite (3465 passed, 0 failed, 1 ignored) all pass; 127 Sessions, Mux, and PRs snapshot frames are byte-identical before and after.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Retired the hidden Union and Forks TUI views. Removed `View::Union`, `View::Forks`, `rows/union.rs`, `rows/forks.rs`, `UnionGrouping`, `ForksGrouping`, `RowKind::Fork`, `RowId::Fork`, `ForkRow`, the `ViewLabel` variants, both `--view` values, and every consumer; the `Fork` graph node and `conspectus table union|forks` are untouched. `--view union|forks` is rejected by clap with the list of valid views; `[tui] default_view = "union"|"forks"` and `[tui.views.union|forks]` raise a config diagnostic that names the six valid views and says the view was removed. Tests assert the text for each removed value (config table, default_view, `tui`, `dev scenario tui`) and that stale saved state loads. Docs updated: operations.md, tui-columns.md, design.md, dev-scenarios.md, CHANGELOG, and ADR 0031 and 0114 status notes. Verified: fmt, clippy (workspace, all targets, all features, -D warnings), rustdoc -D warnings, the whitespace diff check, and the isolated suite (3465 passed, 0 failed, 1 ignored; baseline 3460, so +6 new tests, -1 deleted union test). Before/after `tui --snapshot` frames for Sessions, Mux, and PRs (127 files each: default, narrow, panes, explorer walks, controls overlay, view cycling) are byte-identical.

**Regressions**
- `conspectus tui --view union` and `--view forks` now fail (they were hidden, never in the view switcher); `default_view` of either, or a `[tui.views.union|forks]` table, is ignored with a startup warning.
- Quick select still finds fork entities, but a fork not reachable along a relationship path can no longer be opened (it used to jump to the hidden Forks view), and "Reveal in its view" on a fork pin reports it is not listed. Follow-up CSP-769 asks the operator to pick the fix.

**Consistency wins**
- The view switcher, `--view`, `default_view`, the controls overlay, the persisted-state doc, and `docs/tui-columns.md` now describe the same six views.
- The `default_view` diagnostic used to list five views including the two hidden ones and omit pinned, history, and search; it now lists the six real ones.
<!-- SECTION:FINAL_SUMMARY:END -->
