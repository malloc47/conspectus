---
id: CSP-180
title: Fill out the TUI empty/loading/error frame matrix
status: To Do
assignee: []
created_date: '2026-05-19 23:23'
labels:
  - t8
milestone: m-13
dependencies:
  - CSP-166
ordinal: 407000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: render the full set of empty/loading/error frames
  documented in
  `docs/implementation/phase-08-interactive-tui.md` —
  `--no-live-preview` preview-zone message, tmux-disabled
  mux-view banner, tmux-unavailable mux-view banner, provider
  error chips on the status bar, refresh-failure stale marker.
- Tests: Ratatui buffer snapshots for each state. Reuse the
  `render_to_buffer` / `buffer_to_string` helpers already in
  `src/tui/ui.rs`.
- Blockers: `CSP-166` v1 slice (the render shell is there); a
  `Msg::SetError` reducer addition may be needed.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `T8-003`
