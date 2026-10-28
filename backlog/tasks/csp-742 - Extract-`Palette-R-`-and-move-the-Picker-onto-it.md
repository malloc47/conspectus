---
id: CSP-742
title: Extract `Palette<R>` and move the Picker onto it
status: To Do
assignee: []
created_date: '2026-10-27 20:27'
labels:
  - tui-framework
  - tuifw-choice
milestone: m-25
dependencies:
  - CSP-733
  - CSP-729
  - CSP-727
  - CSP-723
documentation:
  - docs/plans/tui-framework/inventory/04-menus-pickers-dialogs.md
  - docs/plans/tui-framework/inventory/05-search-help-viewers.md
priority: medium
type: enhancement
ordinal: 828000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The Picker (retargeting a pin, search history), quick select, and content search are one control: a query line, a ranked list with a cursor, a status row, a generated footer, and an outcome. The Picker draws no caret and a dead height cap; quick select draws no caret either. The backlog's command palette (CSP-392) would be a third instance (inventory slices 04 and 05, section 6).

Scope: Create `Palette<R: PaletteRow>` with a `LineEdit` query, a scope strip in the title, a ranked list using the shared selection, a status row, a generated footer, an options pane hook, and a debounce contract for slow providers. Move the Picker onto it first, as the smallest instance. Ranking, history items, and the retarget candidates stay in Conspectus. Leave the seams for a command palette instance and for a row preview.

TUI changes:
- Consistency wins: the query line shows a caret; one list look; Ctrl-N and Ctrl-P work the same wherever the palette is used
- Regressions or behavior changes: the Picker's height and size follow the shared policy

Tests and checks: Picker tests ported unchanged. `--snapshot` of the Picker at 80x24 and 40x10.

Blockers: `LineEdit`, the overlay contract, selection, and text.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 `Palette<R>` provides the query, scope strip, ranked list, status row, footer, and options hook
- [ ] #2 The Picker is a `Palette` instance and its dead height cap is gone
- [ ] #3 The documented seams for a command palette instance and a row preview exist
- [ ] #4 Every visible change is listed in the final summary under **Regressions** and **Consistency wins**, and was checked with `conspectus tui --snapshot` at a normal and a narrow width
- [ ] #5 The framework crate names no Conspectus type, string, theme field, or domain word, and the independence guard passes
- [ ] #6 The control has a reference page in the framework docs: purpose, usage, key table, theme tokens used, and the house rules it follows
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
