---
id: CSP-678
title: Render rows of mixed kinds in aligned columns
status: Done
assignee: []
created_date: '2026-10-22 01:11'
updated_date: '2026-10-22 18:12'
labels:
  - history-pins
milestone: m-23
dependencies:
  - CSP-660
  - CSP-667
documentation:
  - docs/plans/history-rebuild-and-pin-slots.md
  - docs/adr/0131-pr-titles-and-the-pr-row-layout.md
priority: medium
type: feature
ordinal: 736000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The Pinned and History views list different kinds together. No current row renderer does that, and the columns must stay aligned across kinds and when groups fold.

- Scope: a renderer for `<kind> <label> <status> <age> <content>`: kind glyph for every kind, label padded and capped at 28, status padded (at most 9), age right-aligned, content flexible and truncated; padding computed over the whole tree; status and content per kind; dimmed gone, missing, and planned rows.
- Tests: buffer snapshot tests at several widths with every kind and state.
- Manual checks: `conspectus tui --snapshot` against a fixture with mixed kinds.
- Blockers: the views ADR and the graph projection ticket.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Rows of every pinnable kind align in the same columns, and folding a group doesn't shift them
- [x] #2 Gone, missing, and planned rows render dimmed with their status word
- [x] #3 Snapshot tests cover each kind and state at narrow and wide widths
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added the mixed-kind row renderer (kind glyph, label, status, age, content; columns padded tree-wide; gone, missing, and planned rows dimmed) in ui/left_panel.rs. Insta snapshots cover every pinnable kind and state at 70, 110, and 160 columns, with an alignment assertion that survives folding. Verified with the full nextest run (2600 tests), clippy, and tui --snapshot.
<!-- SECTION:FINAL_SUMMARY:END -->
