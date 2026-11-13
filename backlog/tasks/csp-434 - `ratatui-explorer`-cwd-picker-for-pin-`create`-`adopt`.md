---
id: CSP-434
title: '`ratatui-explorer` cwd picker for pin `create` / `adopt`'
status: To Do
assignee: []
created_date: '2026-06-19 00:59'
updated_date: '2026-10-31 08:15'
labels:
  - h-widg
milestone: m-11
dependencies: []
ordinal: 373000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Motivation: pin `create` and `adopt` forms currently take
  typed paths. A real file/directory picker would be a UX
  upgrade with no domain risk. `ratatui-explorer` 0.3 (MIT,
  ratatui 0.30) is the canonical drop-in.
- Scope: deferred until the pin-form UX is on the agenda.
  Verify event-loop ownership (this crate's `handle()` API may
  couple more tightly than the others); flag during impl.
- Blockers: `CSP-455` if that story chooses a browse-mode
  picker instead of an inline-only omnibox.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-WIDG-010`

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
TUI framework update (2026-10-31, CSP-766): re-scoped. Path entry is the framework's `tuikit::path_field::PathField` behind a host `PathSource` (ADR 0167), and the framework reads no file system of its own (`crates/tuikit/docs/controls/path_field.md`). `ratatui-explorer` reads directories itself and draws with its own look and keys, so it cannot sit in the framework, and in the host it would bypass the field's source, the theme, and the key tables. What is left: a browse mode beside `PathField` that lists directories through the same `PathSource` (the roadmap's Tier 3 "A file browser"), reading `ratatui-explorer` for ideas only; any new crate still needs an ADR. The pin create and adopt forms are gone (CSP-682); the path fields are in the pin edit, new mux, and mux launch forms.
<!-- SECTION:NOTES:END -->
