---
id: CSP-455
title: Hybrid cwd omnibox for pin create/adopt
status: To Do
assignee: []
created_date: '2026-06-24 20:11'
labels:
  - h-pin-tui
milestone: m-11
dependencies:
  - CSP-454
ordinal: 321000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: replace bare cwd text entry with a composable path
  omnibox. It should accept free-form typing, rank known graph
  paths from the selected row and recent/current workspace before
  filesystem matches, show live existence feedback, and let `Tab`
  complete the highlighted candidate. The widget may incorporate
  `CSP-434`'s `ratatui-explorer` directory picker as an
  alternate browse mode, but the primary flow should work as an
  inline omnibox so create remains keyboard-fast.
- Tests: unit tests for graph-path ranking, filesystem candidate
  matching, tab completion, nonexistent-path validation, selected
  row seeding, and browse-mode handoff if `ratatui-explorer` lands.
  Snapshot tests for empty, matching, no-match, and invalid-path
  states.
- Blockers: `CSP-454`, `CSP-434` if the implementation
  chooses the browse-mode dependency for this slice.
- Follow-up: keep this story open to show multiple ranked
  completions inline or in a stable sidecar area; the first slice
  only shows the best completion remainder next to the cwd field.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added a reusable inline `PathOmniboxState` widget module
with ranked known-path candidates, filesystem prefix matches,
live existence status, and `Tab` completion. Wired the pin create
cwd row to graph-derived candidates from selected/default cwd,
pins, agent sessions, mux sessions, runtime processes, checkouts,
repos, and workspaces. Deferred the `ratatui-explorer` import to
`CSP-434` as an optional browse submode rather than making it a
dependency of the fast inline path.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-PIN-TUI-004`
