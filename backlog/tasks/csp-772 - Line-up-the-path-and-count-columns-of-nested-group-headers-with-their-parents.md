---
id: CSP-772
title: Line up the path and count columns of nested group headers with their parents
status: Done
assignee:
  - '@claude'
created_date: '2026-10-28 07:24'
updated_date: '2026-10-30 04:11'
labels:
  - tui-framework
  - tuifw-cleanup
milestone: m-25
dependencies: []
priority: low
type: bug
ordinal: 858000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found while reproducing CSP-714. A nested group header (a checkout `◇` under a repo `◆` in the Sessions checkout grouping, or under a repo with several worktrees in graph grouping) draws its path two cells right of its parent's, per level of depth: the label cell is padded to one width whatever the row's indent, while the Pinned and History rows pad their label cell from the row's own indent to a common column (`MixedAlign`), so nested rows line up with their parents. Reproduce with `conspectus tui --snapshot --snapshot-keys "<C-g><C-g><C-g>"` on the showcase fixture. The `ColumnSchema` migration (CSP-751) would otherwise encode whichever behavior it meets first.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Nested group headers' path and count columns line up with their parents' at every depth, or docs/tui-columns.md records the offset as intended
- [x] #2 A test covers headers at two depths
- [x] #3 The frames in docs/tui-columns.md are regenerated with just tui-columns and its doc-sync tests pass
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
From CSP-751: group headers are laid out from a column schema now (`src/tui/ui/left_panel/columns.rs`). The header's path column absorbs the row's indent (`Column::absorbs_lead`), so the counts already line up at every depth; the label column does not, so the path still drifts two cells per depth. Adding `.absorbs_lead()` to the `HeaderLabel` column is the fix (then regenerate the frames with `just tui-columns`).

Resolved inside CSP-752: the header label absorbs the indent; the path is measured plainly.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Fixed inside CSP-752 (commit `fix(tui): nested group headers line their paths up with their parents (CSP-772)`; see CSP-752's final summary and ADR 0184 decision 6). The group header's label column now absorbs the row's indent (`Column::cap(Col::HeaderLabel, ..).absorbs_lead()` in `src/tui/ui/left_panel/columns.rs`) and its path is measured plainly, so a nested header's path starts in its parent's column and its count ends in its parent's, at every depth. With the framework rule that rows absorbing their lead give way as a row with no lead would, the counts also stay in line when the path gives way in a narrow pane (they moved 2 cells per level). The drift is fixed, not recorded as intended.

Verification: the reproduction (`conspectus tui --snapshot --snapshot-keys "<C-g><C-g><C-g>"` on the showcase fixture) shows `◆ repo-a` and `◇ repo-a` paths in one column; the before/after capture differs only in the Sessions checkout grouping's depth-0 headers (`sessions-g3-*` at every width). Tests: `nested_group_headers_line_their_paths_and_counts_up_with_their_parents` lays out headers at three depths at 160/110/80/60 (it fails with the old flags); the columns consistency test checks every header's path and count at every depth in every Sessions, Mux, PRs, Pinned, and History grouping at 60/80/110/160. `just tui-columns` added a checkout-grouping frame to `docs/tui-columns.md`; the doc-sync test passes. Checks as in CSP-752.

**Regressions**
- None at wide widths beyond the intended move: a depth-0 header's path moves right to its nested header's column where a nested label is longer (the count column stays where it was in the showcase world).

**Consistency wins**
- Nested headers' paths and counts line up with their parents' at every depth and width.
<!-- SECTION:FINAL_SUMMARY:END -->
