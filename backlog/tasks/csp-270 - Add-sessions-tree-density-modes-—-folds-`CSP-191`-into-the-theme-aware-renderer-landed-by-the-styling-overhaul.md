---
id: CSP-270
title: >-
  Add sessions-tree density modes — folds `CSP-191` into the theme-aware
  renderer landed by the styling overhaul
status: To Do
assignee: []
created_date: '2026-05-25 02:00'
updated_date: '2026-10-31 08:15'
labels:
  - t8
milestone: m-13
dependencies: []
ordinal: 462000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: now that the renderer reads its palette and structural
  cues from `Theme`, density modes can live in the same shape:
  a `[tui].density` setting plus a runtime toggle that picks one
  of `compact` / `balanced` / `expanded`. `compact` drops the
  per-session same-line preview and tightens the badge column;
  `expanded` enables multi-line previews and the future
  section-pane treatment for inline detail (depends on
  `CSP-212`).
- Tests: row-tree/render snapshots per density at 80×24 and
  160×40; reducer tests for the runtime toggle key.
- Blockers: supersedes `CSP-191`'s open scope. Coordinate with
  `CSP-185` (preview throttle) so the expanded mode's extra
  capture work plays nicely with the cadence story.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `T8-024`

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
TUI framework update (2026-10-31, CSP-766): re-scoped. The sessions rows come from `tuikit::columns::ColumnSchema` (ADR 0183, 0184) in a `tuikit::tree::TreeView` (ADR 0182), and the theme is `tuikit::theme` with host roles (ADR 0153). A density mode is now a choice of schema per mode (`compact` drops the same-line preview column and narrows the harness chip column; the schema's give-way rule already drops the preview when narrow) plus, where rows need spacing, the roadmap's Tier 2 `density` token on the theme. The `expanded` mode's "section-pane treatment for inline detail" is obsolete: the sectioned detail model was deleted (CSP-710; ADR 0033 is superseded), and CSP-212 (`tui-markdown`) has landed. The runtime toggle is a keymap binding (ADR 0192) or a Controls row (`SettingsPanel`, ADR 0176). CSP-191 is folded into this story.
<!-- SECTION:NOTES:END -->
