---
id: CSP-625
title: Re-extract regrown inline test modules and guard the threshold
status: Done
assignee:
  - '@claude'
created_date: '2026-10-17 03:09'
updated_date: '2026-10-18 01:15'
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
- [x] #1 An ADR records the sibling test-file convention and its threshold
- [x] #2 No production file has an inline test module at or over the threshold
- [x] #3 A test under `tests/` fails when an inline test module crosses the threshold
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Re-measure inline `mod tests` blocks (same eight files over 100 lines on 2026-10-17).
2. Move each to a sibling `<stem>_tests.rs` (`<dir>_tests.rs` for `mod.rs`) via `#[cfg(test)] #[path = "..."] mod tests;`, without dedenting the body so multi-line string fixtures keep their exact content; `cargo fmt` fixes code indentation.
3. Add `tests/test_module_hygiene.rs` failing on an inline test module of 100+ lines.
4. Record the convention in ADR 0124 and index it.
5. `just check`; test count must only grow by the guard's own tests.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- Moved to siblings: `cli/mux.rs` (245 lines), `tui/preview/git.rs` (126), `tui/viewer_bridge.rs` (119), `tui/preview/mod.rs` (117, to `preview_tests.rs`), `viewer/mod.rs` (112, to `viewer_tests.rs`), `tui/widgets/path_omnibox.rs` (107), `discovery/providers.rs` (105), `tui/widgets/mux_launch.rs` (101). Largest inline module left: `tui/widgets/launch_spec_form.rs` at 92 lines.
- `tests/test_module_hygiene.rs` fails on an inline `mod tests { ... }` block of 100+ lines (header to closing brace). Probed by restoring the old inline `mux_launch.rs`: the guard failed naming `src/tui/widgets/mux_launch.rs:403` at 103 lines; restored.
- ADR 0124 records the convention, the naming rule, and why smaller modules may stay inline; indexed under Process, tooling, and distribution.
- Validation: `just check` green with 2413 tests, the previous 2411 plus the guard's two, so every moved unit test still runs.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
The eight inline test modules that had regrown past 100 lines now live in sibling `_tests.rs` files, and `tests/test_module_hygiene.rs` fails if any inline test module reaches 100 lines again (verified by restoring one). ADR 0124 records the convention. Verified with `just check`: 2413 tests, exactly the previous count plus the two guard tests, so nothing was lost in the moves.
<!-- SECTION:FINAL_SUMMARY:END -->
