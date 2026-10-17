---
id: CSP-625
title: Re-extract regrown inline test modules and guard the threshold
status: To Do
assignee: []
created_date: '2026-10-17 03:09'
labels:
  - h-hyg
milestone: m-11
dependencies: []
references:
  - docs/code-hygiene-audit.md
priority: low
type: chore
ordinal: 683000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
CSP-472 moved every inline `mod tests` of 100 or more lines into a sibling `*_tests.rs` file (65 pairs) so a file's production code reads at a glance. Nothing enforces it, and eight files have grown past the threshold since: `cli/mux.rs` (246 test lines), `tui/preview/git.rs` (127), `tui/preview/mod.rs` (118), `viewer/mod.rs` (113), `tui/viewer_bridge.rs` (108), `tui/widgets/path_omnibox.rs` (107), `discovery/providers.rs` (104), and `tui/widgets/mux_launch.rs` (101). The convention is written down only in CSP-472's notes, not in an ADR or contributor doc, so a contributor has no way to learn it. Audit finding B3.

- Scope: record the sibling test-file convention and its threshold in an ADR, move the eight modules to siblings using the existing `#[path = "<name>_tests.rs"] mod tests;` pattern, and add a guard test under `tests/`, in the style of `tests/comment_hygiene.rs`, that fails when an inline test module crosses the threshold.
- Tests: the guard test itself; all suites pass unchanged after the moves.
- Blockers: none.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 An ADR records the sibling test-file convention and its threshold
- [ ] #2 No production file has an inline test module at or over the threshold
- [ ] #3 A test under `tests/` fails when an inline test module crosses the threshold
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
