---
id: CSP-633
title: 'Remove the dead `RowKind::Pin` row variant'
status: Done
assignee:
  - '@claude'
created_date: '2026-10-18 01:27'
updated_date: '2026-10-18 01:33'
labels:
  - h-hyg
milestone: m-11
dependencies: []
references:
  - docs/code-hygiene-audit.md
priority: low
type: chore
ordinal: 691000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
`tui::rows::RowKind::Pin(PinRow)` is never constructed outside tests. Pins render as placeholder session and mux rows (keyed `RowId::Pin`), so nothing builds a `PinRow`, yet 13 match arms in 9 files still handle the variant (`keymap.rs`, `search.rs`, `runtime/overlay_keys.rs`, `actions.rs`, `app/pins.rs`, `app/tree.rs`, `ui/left_panel.rs`, `ui/preview_pane.rs`, `ui/header.rs`), and tests exercise those arms with hand-built rows that cannot occur at runtime. Found while fixing CSP-618.

- Scope: remove the `RowKind::Pin` variant and `PinRow`, delete the arms and the tests that build the dead row, and keep `RowId::Pin` and the placeholder rows unchanged. If any arm turns out to be reachable, keep it and record why instead.
- Tests: `just check`; TUI snapshot suites and `tui --snapshot` renders of pin placeholders are unchanged.
- Blockers: none.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 `RowKind::Pin` and `PinRow` no longer exist
- [x] #2 Pin placeholder rows render and behave as before
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Confirm no production code constructs `RowKind::Pin` / `PinRow` (pins render as placeholder session and mux rows keyed `RowId::Pin`).
2. Delete the variant and `PinRow`, then every arm the compiler flags (13 in 9 files), the stale doc links, and the test assertions that hand-built the impossible row.
3. Verify with `just check` and before/after `tui --snapshot` renders of a fixture that contains an unbound pin.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- Removed `RowKind::Pin`, `PinRow`, and their arms in `keymap.rs`, `search.rs`, `actions.rs` (2), `runtime/overlay_keys.rs`, `app/pins.rs` (2), `app/tree.rs` (2), `ui/left_panel.rs` (2), `ui/preview_pane.rs`, and `ui/header.rs` (the pin-row status hint block). No arm was reachable, so none was kept. `RowId::Pin` and the placeholder rows are untouched.
- Tests: dropped the hand-built pin row from `ui_tests::row_kind_glyph_span_dispatches_per_row_kind` and a dead arm in a mux test; updated two doc comments and one test comment that named the variant. Test count unchanged (2413).
- Validation: a scratch copy of the showcase fixture with an unbound pin added renders identically before and after in the Sessions view, the Mux view, search for the pin, and the pins menu (6 states, the pin visible in each); `just check` green.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Removed the dead `RowKind::Pin` row variant and `PinRow`, with the 13 match arms in 9 files that handled a row nothing could create; pins keep rendering as placeholder session and mux rows. Verified with `just check` (2413 tests) and identical before/after renders of a fixture containing an unbound pin across the sessions, mux, search, and pins-menu states.
<!-- SECTION:FINAL_SUMMARY:END -->
