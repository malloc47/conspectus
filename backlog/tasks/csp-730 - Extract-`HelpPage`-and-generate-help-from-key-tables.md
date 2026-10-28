---
id: CSP-730
title: Extract `HelpPage` and generate help from key tables
status: To Do
assignee: []
created_date: '2026-10-27 20:26'
labels:
  - tui-framework
  - tuifw-conventions
milestone: m-25
dependencies:
  - CSP-728
  - CSP-723
  - CSP-724
  - CSP-726
documentation:
  - docs/plans/tui-framework/inventory/05-search-help-viewers.md
  - docs/plans/tui-framework/inventory/06-interaction-conventions.md
priority: medium
type: enhancement
ordinal: 816000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Three help generators exist (`viewer_help.rs:39`, `pane_viewer.rs:563`, and the keymap help), and the pane viewer's page is a copy of the transcript viewer's. The main help is 48 hand-written rows with a one-directional drift test, and its blurb cites Ctrl-L where the modal binds S-Tab. `ratatui-cheese`'s `help::Binding` is used only as an inert struct. The page itself pages by a fixed 8 lines and never clamps its over-scroll (inventory slices 05 and 06).

Scope: Create `tuikit::help` with a page renderer (frame, section headings, key column in the accent colour, wrapped descriptions, scrollbar, generated footer) and a `sections_for<A>` generator over a key table plus per-action descriptions. Delete the pane help copy and `ratatui-cheese::help::Binding`. The main keymap page keeps its hand-written rows as annotations until the scoped keymap story generates them. Page by the viewport and clamp the over-scroll.

TUI changes:
- Consistency wins: three help pages share one generator and one frame; help pages can no longer disagree with their key tables
- Regressions or behavior changes: none expected

Tests and checks: Content and styling parity tests (extend the CSP-703 test) for the three pages. `--snapshot` of each help page at 80 and 40 columns.

Blockers: The key-table, text, scroll, and frame extractions.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 `tuikit::help` provides the page renderer and a generator from a key table and its descriptions
- [ ] #2 The transcript viewer and pane viewer pages come from the generator and the pane copy is deleted
- [ ] #3 The `ratatui-cheese` `help::Binding` use is removed
- [ ] #4 The page pages by the viewport and clamps its over-scroll
- [ ] #5 A test fails when a key in a table has no description in its generated page
- [ ] #6 Every visible change is listed in the final summary under **Regressions** and **Consistency wins**, and was checked with `conspectus tui --snapshot` at a normal and a narrow width
- [ ] #7 The framework crate names no Conspectus type, string, theme field, or domain word, and the independence guard passes
- [ ] #8 The control has a reference page in the framework docs: purpose, usage, key table, theme tokens used, and the house rules it follows
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
