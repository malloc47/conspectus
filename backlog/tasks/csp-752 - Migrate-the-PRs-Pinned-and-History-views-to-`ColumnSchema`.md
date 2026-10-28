---
id: CSP-752
title: 'Migrate the PRs, Pinned, and History views to `ColumnSchema`'
status: To Do
assignee: []
created_date: '2026-10-27 20:27'
labels:
  - tui-framework
  - tuifw-data
milestone: m-25
dependencies:
  - CSP-751
documentation:
  - docs/plans/tui-framework/inventory/02-left-pane-tree.md
  - docs/tui-columns.md
priority: medium
type: enhancement
ordinal: 838000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The PRs view carries a tree-wide label width that shrinks to `#N` through a post-hoc rewrite, and its PR state-to-glyph logic exists in six places. Pinned is the only view with status words and drops the chip first at narrow widths. Group headers pack the label and the secondary text into one `display_path` string split on two spaces. History rows are all ghosts (inventory slice 02, section 5).

Scope: Migrate PRs, Pinned, and History to schemas using the one narrow-width policy. Replace the `display_path` packing with structured cells. Use the single PR state function from the left-pane defect task. Regenerate the frames in `docs/tui-columns.md`.

TUI changes:
- Consistency wins: one narrow-width policy across the first five views; group headers carry structured cells
- Regressions or behavior changes: which cells drop first at narrow widths changes for PRs, Pinned, and History; frames in `docs/tui-columns.md` change

Tests and checks: The Pinned snapshots at 70, 110, and 160 columns are regenerated and reviewed. The rows tests and doc-sync tests pass.

Blockers: The first schema migration.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 PRs, Pinned, and History render from schemas and `PrAlign` is removed
- [ ] #2 Group headers use structured cells and `display_path` no longer packs two values into one string
- [ ] #3 `docs/tui-columns.md` and its doc-sync tests are updated
- [ ] #4 Every visible change is listed in the final summary under **Regressions** and **Consistency wins**, and was checked with `conspectus tui --snapshot` at a normal and a narrow width
- [ ] #5 The framework crate names no Conspectus type, string, theme field, or domain word, and the independence guard passes
- [ ] #6 The control has a reference page in the framework docs: purpose, usage, key table, theme tokens used, and the house rules it follows
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
