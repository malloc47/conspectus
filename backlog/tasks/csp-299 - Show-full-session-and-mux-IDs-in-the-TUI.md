---
id: CSP-299
title: Show full session and mux IDs in the TUI
status: Done
assignee: []
created_date: '2026-05-30 04:47'
labels:
  - t8
milestone: m-13
dependencies:
  - CSP-164
  - CSP-166
ordinal: 415000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: replace the TUI's agent-session id-suffix display with
  the full harness-native session id and the full mux-native session
  name anywhere the operator needs an identifier they can copy and
  use outside Conspectus, especially the selected-row detail pane.
  In the selected entity's own detail section, label these rows as
  `id` for agent sessions and `name` for mux sessions, and omit the
  redundant `harness:` / `backend:` prefix because those fields are
  shown separately.
  If a compact label is still needed in the left row tree, prefer a
  leading-prefix abbreviation over a trailing suffix and keep the
  full id visible in detail. Preserve alias/title-first display
  labels from ADR 0029; this story is about the explicit id field,
  not the human-readable session name.
- Tests: pure detail/row view-model tests proving the full
  `AgentSessionId` value is available for selected agent rows and
  mux-attached agent rows; Ratatui buffer snapshots covering a long
  Codex-style id so the detail pane shows a copyable full id and does
  not regress to `...<suffix>`.
- Manual checks: run `cargo run -- tui --view sessions`, select a
  Codex or Claude session with a long native id, and confirm the
  right pane exposes the whole id in display order from the beginning
  of the id.
- Blockers: `CSP-164`, `CSP-166` v1 slices.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
The TUI detail pane now renders full `session_key`
values in the explicit `id` row for selected agent sessions, full
mux session names in the explicit `name` row, and typed full
linked-entity labels in mux-attached session rows, session mux
rows, and parent-session lineage fields. The detail renderer no
longer uses the bold compact title line as the copyable identifier.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `T8-025`
