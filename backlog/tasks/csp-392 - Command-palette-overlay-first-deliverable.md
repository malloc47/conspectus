---
id: CSP-392
title: Command palette overlay (first deliverable)
status: To Do
assignee: []
created_date: '2026-06-05 18:40'
updated_date: '2026-10-31 08:15'
labels:
  - h-cmd
milestone: m-17
dependencies:
  - CSP-391
ordinal: 517000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: on a single key chord (`Ctrl+P` or `M-x`-style `Alt+X`,
  decided by the ADR), open a centered or top-anchored overlay
  listing every registered action. The input line at the top accepts
  a fuzzy filter query; the result list scrolls. `Enter` executes
  the selected action by routing it through the existing
  `Action`→`Msg` dispatch; `Esc` dismisses. Reuse
  `src/tui/widgets/input.rs` for the text field. The palette
  should feel like a discoverability surface, not a replacement for
  the existing direct keybindings — those continue to work
  unchanged. Show the keybinding beside each result so the operator
  learns shortcuts organically. Initial query is empty (show all);
  typing refines. Category headers (`Navigation`, `Session`, …) in
  the result list when results span multiple categories.
- Tests: buffer snapshot tests for the empty-query "show all" state,
  a filtered result set, the no-matches state, and palette dismissal
  without side effects. Reducer tests for action dispatch through the
  palette (confirm, cancel, arrow-navigate, re-filter-on-type).
- Manual checks: open the palette in a live TUI, type a partial
  command name, confirm the filter works, execute a command, verify
  the TUI state changes correctly.
- Blockers: `CSP-391`.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-CMD-003`

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
TUI framework update (2026-10-31, CSP-766): the palette is a `tuikit::palette::Palette<R>` instance (ADR 0174), as `crates/tuikit/docs/controls/palette.md` ("Seams left open") describes: rows built from the CSP-391 source (`Item::new(verb, action).detail(keys)`), filtered with `every_word`, and an `on_pick` that commits the binding's action. Its query line is the palette's own `LineEdit` (`src/tui/widgets/input.rs` is gone). Category headers are not a palette feature: carry the help section in a cell (palette rows can have several cells laid out by a column schema) or in the detail. A commit routes through `Command::action` today, and through `Msg` once CSP-802 lands. Register the overlay with the conformance suite and `src/tui/consistency_tests/`.
<!-- SECTION:NOTES:END -->
