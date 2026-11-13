---
id: CSP-788
title: Make the right pane's hint for an unresolved row say what is true
status: Done
assignee:
  - '@claude'
created_date: '2026-10-30 07:23'
updated_date: '2026-10-30 21:58'
labels:
  - tui-framework
milestone: m-25
dependencies: []
priority: low
ordinal: 874000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Enter on an unresolved-evidence row under the right pane's Other header sets the status line to "explorer: unresolved evidence — `o` opens detail", but `o` on that row only says "explorer: row has no truncated value to open": the property grid (ADR 0187) reports no inspect intent for a link row. Found while extracting the property grid (CSP-754). Either the hint points at where the evidence is shown (the Preview, when the cursor is on the row) or `o` opens the evidence whole, as `o` opens a cut field; the open evidence-inspector work in CSP-318 may decide the second.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Enter on an unresolved row shows a hint that matches what the keys it names do
- [x] #2 A test drives Enter and then the named key on an unresolved row and checks the outcome
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Choose the first of the story's two paths: the hint points at the Preview, which shows the unresolved row's evidence (heading, harness_key, native_id, state_scope, path, edge) whenever the cursor is on the row. Opening the evidence whole with `o` is the evidence inspector's (CSP-318) to decide, and would need the property grid to report an inspect intent for a link row.
2. `App::refusal_text`: on `Refusal::NoFarEnd` with the cursor on an unresolved row, say that the evidence leads to no node and that the Preview shows what it records; for any other inert link, that it leads to no node. No key is named, so the hint cannot promise a key that does something else.
3. Test (ui_tests): a session with an unresolved parent link; focus right, open `Other`, put the cursor on the unresolved row; Enter sets the new hint (not naming `o`), the rendered Preview shows the evidence fields, and `o` on the row still refuses (the key the old hint named).
4. Snapshot the right pane on the showcase's unresolved row at 100x30 and 60x20; record the choice in operator-review.md; CHANGELOG if operator-visible.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented in d402bef0. Chose the story's first path (the hint points at where the evidence is shown); `o` opening the evidence whole is the evidence inspector's (CSP-318), and would need `tuikit::property_grid::LinkRow` to report an inspect intent. First wording (`explorer: unresolved evidence leads to no node — the Preview shows what it records`) was cut before "Preview" on a 60-column bar, so it became `explorer: evidence names no node — the Preview shows it` (55 cells). Other inert links (a link that left the view) say `explorer: this link leads to no node`. Test `enter_on_unresolved_evidence_points_at_the_preview_that_shows_it` (ui_tests.rs). Frames: /tmp/tuifw-baseline/frames-788-after (showcase fork `alpha`, first unresolved row under Other, at 100x30 and 60x20; Enter then `o`). Checks: fmt, clippy -D warnings, full isolated suite 4768 passed / 0 failed / 1 ignored, cargo doc -D warnings, examples, git diff --check.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`Enter` on an unresolved-evidence row under the right pane's `Other` header now says `explorer: evidence names no node — the Preview shows it` instead of "`o` opens detail", which `o` did not do (it refuses with "row has no truncated value to open"). The Preview shows the evidence (the `unresolved <node type>` heading, `harness_key`, `native_id`, `state_scope`, `path`, and the edge) whenever the cursor is on the row, so the hint names a place that shows it and no key. Any other inert link says `explorer: this link leads to no node`. Opening the evidence whole with `o` is left to the evidence inspector (CSP-318). Operator guide (right-pane `Enter` row), CHANGELOG (Fixed), and operator-review updated.

Verification: `enter_on_unresolved_evidence_points_at_the_preview_that_shows_it` drives Enter on the row, checks the hint, renders the screen and checks the Preview lists the evidence fields, then drives `o` (the key the old hint named) and checks it opens nothing. `conspectus tui --snapshot` on the showcase fork's unresolved rows at 100x30 and 60x20 (the hint fits whole at 60 columns). fmt, clippy, full isolated suite (4768 passed, 0 failed, 1 ignored), cargo doc, examples, and `git diff --check` pass.

**Regressions**
- None.

**Consistency wins**
- The status line after `Enter` on an unresolved row names where the evidence is (the Preview) rather than a key that refuses; it fits a 60-column status bar whole.
<!-- SECTION:FINAL_SUMMARY:END -->
