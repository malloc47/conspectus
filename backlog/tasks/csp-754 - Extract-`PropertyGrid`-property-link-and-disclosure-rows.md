---
id: CSP-754
title: 'Extract `PropertyGrid`: property, link, and disclosure rows'
status: To Do
assignee: []
created_date: '2026-10-27 20:27'
labels:
  - tui-framework
  - tuifw-data
milestone: m-25
dependencies:
  - CSP-723
  - CSP-727
  - CSP-713
  - CSP-746
  - CSP-710
documentation:
  - docs/plans/tui-framework/inventory/07-chrome-and-explorer.md
priority: medium
type: enhancement
ordinal: 840000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The right pane's node fields (label column, value, placeholder, annotation, glyph, copy and inspect) and Related rows (validated, quiet, alert, and inert tones) are a property grid. Highlight extent differs in four ways (the explorer ignores `theme.selection_*`, highlights only the value span on field rows, and shows no cursor when unfocused), 9 glyph sites bypass the glyph table, and `core_fields.get(index)` is the copy defect. Disclosure rows exist in both panes (inventory slice 07).

Scope: Create `PropertyGrid` with `PropertyRow` (label, value, dim, hint, annotation, glyph), a `LinkRow` family (indent, prefix, verb column, glyph, middle-truncated label, meta, tone), and `DisclosureRow`, with copy and inspect outcomes (inspect opens the value in the pager modal). Use the shared selection pair. Domain content (`FieldValue`, link semantics, provider chips) stays in Conspectus.

TUI changes:
- Consistency wins: one highlight extent; the unfocused cursor is visible; glyphs come from the glyph table
- Regressions or behavior changes: the highlight on field rows covers the whole line instead of only the value

Tests and checks: The explorer tests (`explorer_tests.rs`) and right-pane tests pass or are updated with a note. `--snapshot` of a node with many fields, `F` on and off, focused and unfocused.

Blockers: Text, selection, the copy defect, and the pager modal.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 `PropertyGrid` provides property, link, and disclosure rows with copy and inspect outcomes
- [ ] #2 Highlight extent follows the shared selection rule in both focus states
- [ ] #3 The 9 hard-coded glyph sites use the glyph table
- [ ] #4 Domain value types and link semantics stay in Conspectus
- [ ] #5 Every visible change is listed in the final summary under **Regressions** and **Consistency wins**, and was checked with `conspectus tui --snapshot` at a normal and a narrow width
- [ ] #6 The framework crate names no Conspectus type, string, theme field, or domain word, and the independence guard passes
- [ ] #7 The control has a reference page in the framework docs: purpose, usage, key table, theme tokens used, and the house rules it follows
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
