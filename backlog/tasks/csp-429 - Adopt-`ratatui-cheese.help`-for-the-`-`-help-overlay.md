---
id: CSP-429
title: Adopt `ratatui-cheese.help` for the `?` help overlay
status: Done
assignee: []
created_date: '2026-06-19 00:59'
labels:
  - h-widg
milestone: m-11
dependencies:
  - CSP-425
  - CSP-428
ordinal: 368000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Motivation: 598 LOC of mostly static keybinding rendering.
  `ratatui-cheese` ships a Bubbletea-style `help` widget that
  handles the keymap → display layout. Domain-specific content
  (the icon legend, the kind-glyph blurbs) stays in-tree as data
  that feeds the upstream widget.
- Story-vs-reality calibration: the motivation framed
  `ratatui_cheese::help::Help` as the keymap display primitive
  we'd swap onto. Pre-swap finding recorded the categorical
  mismatch — `cheese.Help` renders a short-mode flat line
  (Bubbletea status-bar style) or a full-mode multi-column
  grid where each column is `max_key_w + max_desc_w` wide.
  Conspectus's overlay is a long sectioned-vertical cheat sheet
  with descriptions running 200+ chars (the `v` binding alone
  lists 9 sub-actions). `cheese.Help` would have either
  overflowed off-screen or required truncating descriptions.
  The user picked the scoped-down path ("Adopt Binding as a
  data type only").
- Followups: future refactors that motivated picking this path
  (filter to a view-specific set, disable a binding under
  feature flag, sort by section, expose a search-bindings
  primitive later) are now operations on `Vec<Binding>` rather
  than on rendered lines. Theme glue unchanged from CSP-428
  (border via `themed_popup`).
- Tests: 1666 pass byte-identical; existing key-dispatch tests
  (Esc / q / `?` close, scroll) keep shape.
- Blockers: `CSP-425`, `CSP-428` (so the framing layer is
  already on `tui-popup`). [both met]
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Outcome (2026-06-19): landed as `fe20afd`. Pulls in
`ratatui_cheese::help::Binding` as the structured
key/description type. Restructures the help overlay's body
builder so the keymap lives as data (`Vec<HelpSection>`
where each section owns a `Vec<Binding>`) rather than as a
long sequence of imperative `bind(&mut lines, "f", "Open
...")` calls. The renderer walks the data and produces
byte-identical output: bold section headers, key column in
`theme.panel_focus_accent`, descriptions in default fg.
Net source delta: `+192 / -188` (LOC-neutral — the win is
structural). Visual verification: byte-identical.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-WIDG-005`
