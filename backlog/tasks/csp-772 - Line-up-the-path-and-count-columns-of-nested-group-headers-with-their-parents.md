---
id: CSP-772
title: Line up the path and count columns of nested group headers with their parents
status: To Do
assignee: []
created_date: '2026-10-28 07:24'
updated_date: '2026-10-30 02:31'
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
- [ ] #1 Nested group headers' path and count columns line up with their parents' at every depth, or docs/tui-columns.md records the offset as intended
- [ ] #2 A test covers headers at two depths
- [ ] #3 The frames in docs/tui-columns.md are regenerated with just tui-columns and its doc-sync tests pass
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
From CSP-751: group headers are laid out from a column schema now (`src/tui/ui/left_panel/columns.rs`). The header's path column absorbs the row's indent (`Column::absorbs_lead`), so the counts already line up at every depth; the label column does not, so the path still drifts two cells per depth. Adding `.absorbs_lead()` to the `HeaderLabel` column is the fix (then regenerate the frames with `just tui-columns`).
<!-- SECTION:NOTES:END -->
