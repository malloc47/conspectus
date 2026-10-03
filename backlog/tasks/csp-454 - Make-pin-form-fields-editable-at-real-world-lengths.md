---
id: CSP-454
title: Make pin form fields editable at real-world lengths
status: Done
assignee: []
created_date: '2026-06-24 20:11'
labels:
  - h-pin-tui
milestone: m-11
dependencies:
  - CSP-452
ordinal: 320000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: fix the current text-entry ergonomics for long cwd,
  display, mux, and launch-argv values. Fields must horizontally
  scroll to keep the cursor visible, expose the hidden left/right
  content with stable indicators, and support conventional editing
  keys (`Left`/`Right`, `Home`/`End`, word movement where already
  available, `Tab` / `Shift-Tab` field navigation). `Enter` should
  have one unambiguous meaning per form state; dropdown-like fields
  such as store selection must render and behave like option sets,
  not ordinary text rows.
- Tests: widget/reducer tests for cursor visibility, horizontal
  offset updates, field navigation, store option toggling, confirm
  behavior, and narrow-modal rendering. Ratatui snapshots for long
  cwd and long launch argv fields.
- Blockers: `CSP-452`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
The create form now renders text fields through a
width-aware horizontal window that keeps the cursor visible and
shows stable hidden-left / hidden-right indicators for long
values with colored, spaced markers. Generated default names are
capped so selected-session titles do not seed a horizontally
scrolling primary name, while exact adopt defaults still preserve
live mux names. `Left`/`Right`, `Home`/`End`, and word movement
continue through the underlying text input; `Tab` advances fields
and `Shift-Tab` moves backward. `Enter` consistently submits the
create form, while mode and store are rendered as option controls
and changed with `Space` / arrow keys. The visible fresh-create
mode label is `new`; the explicit row/edit focus model that would
preserve `j`/`k` navigation is recorded separately in
`CSP-455.01`.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-PIN-TUI-003`
