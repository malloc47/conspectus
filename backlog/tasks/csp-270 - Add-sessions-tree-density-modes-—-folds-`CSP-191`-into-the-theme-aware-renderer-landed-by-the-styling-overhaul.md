---
id: CSP-270
title: >-
  Add sessions-tree density modes — folds `CSP-191` into the theme-aware
  renderer landed by the styling overhaul
status: To Do
assignee: []
created_date: '2026-05-25 02:00'
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
