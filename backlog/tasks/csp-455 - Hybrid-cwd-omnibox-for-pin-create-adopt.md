---
id: CSP-455
title: Hybrid cwd omnibox for pin create/adopt
status: To Do
assignee: []
created_date: '2026-06-24 20:11'
updated_date: '2026-10-31 08:15'
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

Legacy ID: `H-PIN-TUI-004`

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
TUI framework update (2026-10-31, CSP-766): `PathOmniboxState` (`path_omnibox.rs`) became the framework's `tuikit::path_field::PathField` behind a host `PathSource` (CSP-735, ADR 0167): a `PathKind` (a directory field takes no file), `~` expanded on commit, and completion cached so drawing does no I/O. Completion now only extends the typed text (known directories by priority, then directories on disk that start with the last component); the old substring jump is gone (operator review, CSP-735). The ranked list this story keeps open is the `PathField::candidates()` seam. The pin create and adopt forms are gone (pins come from `p` on any row, CSP-682); the path fields are in the pin edit, new mux, and mux launch forms (`src/tui/widgets/pin_edit.rs`, `new_mux.rs`, `mux_launch.rs`), all `Form<M>` fields (ADR 0168).
<!-- SECTION:NOTES:END -->

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
