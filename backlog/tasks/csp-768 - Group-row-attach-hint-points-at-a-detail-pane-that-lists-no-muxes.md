---
id: CSP-768
title: Group-row attach hint points at a detail pane that lists no muxes
status: Done
assignee:
  - '@claude'
created_date: '2026-10-28 04:37'
updated_date: '2026-10-30 22:17'
labels:
  - tui
  - tui-framework
milestone: m-25
dependencies: []
priority: low
ordinal: 854000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Pressing `a` on a workspace, repo, or checkout row whose sessions are ambiguous about two or more muxes says "attach: N ambiguous muxes here — pick one in the detail pane". The right pane no longer lists them: the sectioned detail model that emitted an `Ambiguous muxes` section was replaced by the graph explorer (and deleted by CSP-710), and a group node has no explorer link to those muxes. The operator is told to pick from a list that does not exist.

Decide the intended path: either surface the group's ambiguous muxes in the explorer (for example as rows the operator can drill into or attach from), or reword the hint to send the operator to a session or mux row. `ambiguous_muxes_for_group` in `src/tui/rows/mod.rs` already computes the set. ADR 0071 decision 2 is marked superseded; a new ADR is needed if the explorer gains the rows.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 The hint on a group row with several ambiguous muxes names a place that shows them, or the explorer shows them
- [x] #2 Tests cover the chosen behavior for workspace, repo, and checkout rows
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Decide the path: reword, not new explorer rows. ADR 0074 replaced ADR 0071's derived `Ambiguous muxes` section with an explorer that lists a node's own links (validated, then `Other` for what could be), and the ambiguity is already on the links that carry it: each ambiguous session's right pane lists its candidate muxes under `Other`, and its status bar names `Tab inspect candidates`. Group rows appear in the Sessions, Mux, and PRs views; the session rows are the place that shows the muxes in all of them.
2. `attach_disabled_why(AmbiguousGroupMuxes(n))`: `{n} ambiguous muxes here — select a session to see its candidates` (no key named; ADR 0071's status notes the new words).
3. Tests: for a workspace, a repo, and a checkout group with two ambiguous muxes, `a` posts the new hint, and every mux `ambiguous_muxes_for_group` returns is listed under `Other` in the right pane of one of the group's sessions.
4. Snapshot the showcase's `project` repo group (`a`) and an ambiguous session's right pane at 100x30 and 60x20; operator guide, CHANGELOG, operator-review.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented in bfd9cafb. Path: reword, not explorer rows. ADR 0074 superseded ADR 0071's derived `Ambiguous muxes` section with an explorer of a node's own links; the ambiguity lives on the sessions' `LinkedToMux` links, which each ambiguous session's right pane lists under `Other`. New hint: `attach: N ambiguous muxes here — select a session to see its candidates`. Test `a_on_a_group_with_several_ambiguous_muxes_sends_the_operator_to_its_sessions` (actions_tests.rs) builds a workspace, a repo with two checkouts (so checkout rows render), and two ambiguous sessions; for the workspace, repo, and checkout rows it checks `a`'s toast, then for each session under the group focuses the right pane, opens `Other`, puts the cursor on each candidate, and checks `resolve_attach_target` attaches it; every mux `ambiguous_muxes_for_group` returns is reached. `--snapshot` skips the attach action, so the frames (/tmp/tuifw-baseline/frames-768-after, 100x30 and 60x20) show the place the hint names: the showcase's ambiguous session with `Other` open listing `tmux:ambiguous` and `tmux:project`. Checks: fmt, clippy -D warnings, full isolated suite 4769 passed / 0 failed / 1 ignored, cargo doc -D warnings, examples, git diff --check.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`a` on a workspace, repo, or checkout row whose sessions are ambiguous about two or more muxes now says `attach: N ambiguous muxes here — select a session to see its candidates` instead of pointing at a detail-pane list that no longer exists. The story's reword path was taken: the explorer lists a node's own links (ADR 0074 replaced ADR 0071's derived `Ambiguous muxes` section), and the ambiguity is on the sessions' links, so each ambiguous session's right pane lists its candidate muxes under `Other`, and `a` there attaches the one under the cursor. No explorer rows were added, so no new ADR; ADR 0071's status records the change. Operator guide (status bar section), CHANGELOG (Fixed), and operator-review updated.

Verification: `a_on_a_group_with_several_ambiguous_muxes_sends_the_operator_to_its_sessions` covers a workspace, a repo, and a checkout group row: the hint, and that the group's sessions list every mux `ambiguous_muxes_for_group` returns in the right pane, each attachable there with `a`. `conspectus tui --snapshot` at 100x30 and 60x20 of the showcase's ambiguous session with `Other` open (the snapshot driver skips attaching, so the message itself is checked by the test). fmt, clippy, full isolated suite (4769 passed, 0 failed, 1 ignored), cargo doc, examples, and `git diff --check` pass.

**Regressions**
- None.

**Consistency wins**
- The message after `a` on a group with several ambiguous muxes names a place that lists them (an ambiguous session, whose right pane shows its candidates under `Other`) instead of a detail-pane list that is gone.
<!-- SECTION:FINAL_SUMMARY:END -->
