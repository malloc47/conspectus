---
id: CSP-434
title: '`ratatui-explorer` cwd picker for pin `create` / `adopt`'
status: To Do
assignee: []
created_date: '2026-06-19 00:59'
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
