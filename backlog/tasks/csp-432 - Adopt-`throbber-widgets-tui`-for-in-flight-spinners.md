---
id: CSP-432
title: Animate the TUI's loading placeholders with the framework spinner
status: To Do
assignee: []
created_date: '2026-06-19 00:59'
updated_date: '2026-10-31 08:15'
labels:
  - h-widg
milestone: m-11
dependencies:
  - CSP-425
ordinal: 371000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Motivation: pin launch, attach round-trip, refresh, and
  mux-capture refresh are operations where a momentary spinner
  would communicate "working" without inventing visible state.
  `throbber-widgets-tui` 0.11 (Zlib, ratatui 0.30) is the
  standard pick.
- Scope: replace any in-tree "..." status placeholders with the
  upstream throbber; integrate into the status bar.
- Open questions: license is Zlib (permissive but unusual).
  Verify it doesn't conflict with the MIT-only posture.
  Recommend an ADR-tier ack if Zlib is the only blocker.
- Blockers: `CSP-425`.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-WIDG-008`

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
TUI framework update (2026-10-31, CSP-766): retargeted. The framework ships the spinner without a dependency: `tuikit::chip::spinner_frame` and `StatusChip::busy` draw `marks::SPINNER` frames, vetted in the glyph table, from how long the work has run (CSP-725, CSP-722; ADR 0154, 0157), and the status bar already shows one busy chip per in-flight discovery, launch, or content search (ADR 0127, `push_in_flight_chips` in `src/tui/ui/header.rs`). Refresh and launch are covered; an attach suspends the TUI, so nothing can draw during it. What is left are the static loading placeholders: `Loading discovery…` (`src/tui/ui/left_panel.rs`), `Loading…` (`src/tui/ui/right_panel.rs`), and `loading…`, `loading transcript…`, and `loading mux preview…` (`src/tui/ui/preview_pane.rs`), which can lead with the same spinner frame (the runtime redraws at least every 100 ms or so through its poll timeout, so the frame advances with no tick of its own). `throbber-widgets-tui` is not needed, so the Zlib license question is moot. The dependency on CSP-425 is satisfied.
<!-- SECTION:NOTES:END -->
