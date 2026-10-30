---
id: CSP-791
title: >-
  Extract `StackedMasterDetail` and the section rule from the right pane's
  layout
status: To Do
assignee: []
created_date: '2026-10-30 08:09'
labels:
  - tui-framework
  - tuifw-data
milestone: m-25
dependencies:
  - CSP-755
documentation:
  - docs/plans/tui-framework/explorer-design.md
  - docs/plans/tui-framework/inventory/07-chrome-and-explorer.md
priority: medium
type: enhancement
ordinal: 877000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
`draw_right_panel` (`src/tui/ui/right_panel.rs`, about 100 lines) lays the right pane out inline: it lays the explorer's lines out one column narrower than the pane so a rule cannot wrap when the scrollbar appears, wraps them, sizes the explorer zone to its rows (at least three) while keeping six rows for the Preview when the pane allows, draws the one-row `Preview` rule, and keeps the zone's scroll window with the shared gutter. Only whole-frame tests reach that arithmetic (`right_pane_preview_keeps_minimum_height_when_related_full`). `chip_divider_line`, which draws the `Related` and `Preview` rules, lives in the host although it names nothing of the host. The explorer design (docs/plans/tui-framework/explorer-design.md, section 4.2) specifies `StackedMasterDetail` and a `SectionRule` for this; this is step 2 of its migration, and the explorer's zones then name their headings as section rules.

Scope: Add `tuikit::master_detail::StackedMasterDetail` (a list zone sized to its wrapped rows within a minimum and the room a minimum detail area leaves, a one-row divider, the detail area; the list drawn in a kept window with the shared gutter; the window and the areas returned) and `tuikit::frame::SectionRule` (a muted rule, a label chip in `accent` under `badge`, an optional summary before or after it, a two-cell closing rule), each with a control page. The right pane draws through both. Conspectus keeps what the zones hold: the explorer's lines, the search hit block above them, the Preview body, and the rules' words.

TUI changes:
- Consistency wins: one layout rule and one rule line, testable without the App
- Regressions or behavior changes: none; frames are byte-identical

Tests and checks: framework tests for the layout (the detail keeps its minimum when the list is long, the fallback on a short pane, the list's minimum, the layout width) and for both summary placements of the rule; the host's right-pane layout and scrollbar tests pass unchanged; before and after `conspectus tui --snapshot` frames of the right pane at 160x40, 100x30, and 60x20, focused and unfocused, compared byte for byte.

Blockers: the design (CSP-755).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 `tuikit::master_detail::StackedMasterDetail` and `tuikit::frame::SectionRule` are in the framework as the explorer design specifies, and the right pane draws through them
- [ ] #2 Framework tests cover the layout's minimum detail rows, its short-pane fallback, the list's minimum, the layout width, and both summary placements of the rule
- [ ] #3 Before and after `conspectus tui --snapshot` frames of the right pane are byte-identical at 160x40, 100x30, and 60x20, focused and unfocused
- [ ] #4 The framework crate names no Conspectus type, string, theme field, or domain word, and the independence guard passes
- [ ] #5 Each control has a reference page in the framework docs: purpose, usage, key table, theme tokens used, and the house rules it follows
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
