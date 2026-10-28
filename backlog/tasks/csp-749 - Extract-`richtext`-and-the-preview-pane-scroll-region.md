---
id: CSP-749
title: Extract `richtext` and the preview-pane scroll region
status: To Do
assignee: []
created_date: '2026-10-27 20:27'
labels:
  - tui-framework
  - tuifw-reading
milestone: m-25
dependencies:
  - CSP-745
  - CSP-723
documentation:
  - docs/plans/tui-framework/inventory/05-search-help-viewers.md
  - docs/adr/0106-preview-pane-wrap-modes.md
priority: medium
type: enhancement
ordinal: 835000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The preview pane combines a generic scroll region (anchoring at the top, the bottom, or a match) and ANSI wrap modes with domain sources and loaders. The markdown, table, and syntax rendering (`tui-markdown`, `syntect`, `comfy-table`) and the GFM segmenter sit in `src/viewer/render.rs` and `table.rs`. ADR 0152 puts the heavy dependencies behind a cargo feature (inventory slice 05).

Scope: Create the `richtext` feature with the ANSI captured-pane layout and wrap modes (`preview_wrap.rs`), markdown and table rendering, and syntax highlighting, and a `ScrollRegion` for the preview pane with Top, Bottom, and Match anchoring. Preview sources, loaders, the store, and the banner stay in Conspectus, which enables the feature.

TUI changes: none expected. The preview wrap modes and the transcript and pane previews render as before.

Tests and checks: The `preview_wrap` tests and the preview-pane tests pass. A build without the `richtext` feature compiles and tests the rest of the framework.

Blockers: `Pager` and text.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The `richtext` feature owns ANSI layout, wrap modes, markdown, tables, and syntax highlighting, and `syntect`, `tui-markdown`, and `comfy-table` are optional dependencies
- [ ] #2 The preview pane's scroll region lives in the framework with Top, Bottom, and Match anchoring
- [ ] #3 `tuikit` builds and passes its tests with the feature off
- [ ] #4 Preview rendering is unchanged: the preview tests pass without edits to expectations
- [ ] #5 Every visible change is listed in the final summary under **Regressions** and **Consistency wins**, and was checked with `conspectus tui --snapshot` at a normal and a narrow width
- [ ] #6 The framework crate names no Conspectus type, string, theme field, or domain word, and the independence guard passes
- [ ] #7 The control has a reference page in the framework docs: purpose, usage, key table, theme tokens used, and the house rules it follows
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
