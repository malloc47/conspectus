---
id: CSP-392
title: Command palette overlay (first deliverable)
status: To Do
assignee: []
created_date: '2026-06-05 18:40'
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
