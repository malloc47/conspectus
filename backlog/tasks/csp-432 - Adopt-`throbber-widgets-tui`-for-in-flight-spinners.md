---
id: CSP-432
title: Adopt `throbber-widgets-tui` for in-flight spinners
status: To Do
assignee: []
created_date: '2026-06-19 00:59'
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
