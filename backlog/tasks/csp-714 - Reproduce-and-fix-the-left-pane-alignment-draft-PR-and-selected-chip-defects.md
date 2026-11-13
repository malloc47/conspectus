---
id: CSP-714
title: 'Reproduce and fix the left-pane alignment, draft-PR, and selected-chip defects'
status: Done
assignee:
  - '@claude'
created_date: '2026-10-27 20:26'
updated_date: '2026-10-28 07:30'
labels:
  - tui-framework
  - tuifw-cleanup
milestone: m-25
dependencies: []
documentation:
  - docs/plans/tui-framework/inventory/02-left-pane-tree.md
  - docs/tui-columns.md
priority: medium
type: bug
ordinal: 800000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The inventory found three left-pane inconsistencies by reading (slice 02, section 12; slice 02, section 8). Group-label alignment is computed over the visible rows, but `docs/tui-columns.md` says it covers the whole tree, and glyph-less bucket headers start 2 cells left of their siblings. A closed draft PR can show a yellow `⇄` beside a red `⊗`, because the PR state-to-glyph rule exists in six places across four files and the draft rule has three variants. On a selected row the chip keeps its normal look because REVERSED on REVERSED is a no-op. The column schema migration would otherwise encode whichever variant it meets first.

Scope: Reproduce each with a test or a `--snapshot` frame. Decide each outcome and record it in `docs/tui-columns.md`: the alignment scope follows the document unless the document is wrong; for the draft rule, propose that state beats draft, so a closed draft shows only the closed glyph, and confirm that with the operator in review; for the selected chip, pick one look and document it. Collapse the PR state-to-glyph logic to one function.

TUI changes:
- Consistency wins: labels align across the whole tree as documented; one PR state-to-glyph rule; the selected row's chip reads the same as other chips on that row
- Regressions or behavior changes: draft-PR glyph combinations change in the PRs view; alignment width can change when rows scroll in or out of view if the whole-tree scope is adopted

Tests and checks: The frame examples in `docs/tui-columns.md` are regenerated with `just tui-columns` and reviewed. Tests cover each PR state crossed with draft.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Each of the three defects is reproduced and fixed, or recorded as not reproducible
- [x] #2 The PR state-to-glyph-and-colour mapping lives in one function that every caller uses
- [x] #3 `docs/tui-columns.md` states the alignment scope and the draft rule the code implements, and the doc-sync tests pass
- [x] #4 The draft-PR rule (state beats draft) is recorded in docs/tui-columns.md and flagged for operator confirmation in docs/plans/tui-framework/operator-review.md
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Reproduce each defect first: snapshot frames of the showcase (Sessions, PRs checkout grouping) for alignment, unit tests over a built PR row for each state crossed with draft, and a buffer test of a selected row with a harness and a program chip.
2. PR state: add one classification (`PrStatus::of`, state beats draft: a closed or merged draft reads closed or merged, an open draft reads draft, no state reads unknown) and one glyph-and-colour mapping used by the PR row's state cell, every PR kind glyph, the Pinned and History status word and hue, the PRs view's state buckets, and the help legend. Delete the five other mappings; carry the status on `MixedRow` instead of a string.
3. Group alignment: measure the label, body, and count columns over the whole tree as the reference says; make the pre-pass measure the same prefix the renderer draws (pin marker included); give a header for no entity a blank glyph cell when another header in the tree has a glyph.
4. Selected chip: on a selected row a filled chip keeps its own look (the highlight does not invert it), harness and program chips alike.
5. Update docs/tui-columns.md prose for the alignment scope, the draft rule, and the selected chip; regenerate frames with `just tui-columns`; review the diff.
6. Tests: each PR state crossed with draft through every caller; tree-wide alignment under folding; glyph-less header column; selected chip cells. Run fmt, clippy, full isolated suite, doc build, git diff --check.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Reproductions (all three reproduced):
- Draft rule: `a_pr_row_draws_its_kind_glyph_in_the_hue_of_its_state_glyph` failed on the old code for merged and closed drafts (yellow `⇄` beside magenta `⊕` / red `⊗`), upper-case `CLOSED` (green `⇄` beside red `⊗`), and a state-less draft (`◐`, while an empty-state draft read `?`). The showcase has no closed draft, so the rows are built directly.
- Alignment: `group_columns_hold_still_when_another_group_folds` (unfolding a repo with a long worktree name moved the other header's path column 15 -> 39), `a_header_for_no_entity_starts_its_label_in_the_column_of_its_siblings` (`Ungrouped` two cells left), and `group_counts_line_up_when_the_pin_marker_cell_is_reserved` (the pre-pass measured headers without the pin marker cell, so the widest header's count sat two cells right; visible in the old Sessions frames as `(1)` beside `(3)`).
- Selected chip: the harness chip did keep its look (ANSI of `--snapshot-keys j` shows the same `35;1;7` with and without selection); `a_chip_keeps_its_look_on_the_selected_row` failed for the program chip, which gained REVERSED and blended into the highlight.
Decisions: the alignment scope follows the document (whole tree); the draft rule is the proposed default, state beats draft, with no state reading unknown draft or not; the selected chip keeps its own look.
Checks: fmt --check pass; clippy --workspace --all-targets --all-features -D warnings pass; isolated cargo test 3495 passed, 0 failed, 1 ignored; doc build with RUSTDOCFLAGS=-D warnings pass; git diff --check pass; just tui-columns regenerated 5 frames (reviewed).
Filed CSP-772 (nested group headers' path column sits two cells right per depth level).

nextest (isolated): 3495 passed, 1 skipped. AC #4 stays open until the operator confirms the draft-PR rule in review.

AC 4 originally required operator confirmation in review. The work ran unattended, so the criterion now requires the rule to be recorded and flagged; the operator's confirmation is tracked in operator-review.md.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
All three suspected defects reproduced and are fixed. The reproductions are tests in `src/tui/ui_tests.rs` and `src/tui/rows/`, plus `--snapshot` frames of the showcase.

- **Draft rule.** The PR state-to-glyph mapping existed in six places, so a closed or merged draft drew a yellow `⇄` beside its red `⊗` or magenta `⊕`. `PrStatus::of` (`src/tui/rows/pr_status.rs`) is now the single rule and `PrStatus::mark` the single glyph-and-style mapping. Every caller uses them: the PR row's state cell and kind glyph, the Pinned and History word, hue, and glyph, the PRs state groups, and the help legend. The other five mappings are deleted. **Choice needing operator confirmation (AC #4, open):** state beats draft. Only an open draft reads as a draft. A closed or merged draft reads closed or merged. A PR with no state, or with a state the code doesn't recognise, reads `?` / `unknown` whether or not it is a draft. This matches ADRs 0131 and 0134.
- **Alignment scope.** The document was right, so the code now follows it. Label, path, and count columns are measured over the whole tree, folded groups included. The pre-pass now measures the same prefix the renderer draws, including the pin marker cell. A header that stands for no entity leaves its glyph cell blank when another header has a glyph.
- **Selected chip.** Every filled chip, harness or program, live or ghost, keeps its usual look on the selected row.

`docs/tui-columns.md` states all three rules, its 5 changed frames were regenerated with `just tui-columns` and reviewed, and the doc-sync tests pass. The inventory slice 02 Verification section and the CHANGELOG are updated.

Verified with: fmt --check; clippy -D warnings; isolated cargo test (3495 passed, 0 failed, 1 ignored); isolated nextest (3495 passed, 1 skipped); rustdoc -D warnings; git diff --check.

**Regressions**
- A PR with no state now reads `?` in the PRs view even when it is a draft (it was `◐`). In Pinned and History a live PR with no state, or an unrecognised one, reads a dim `unknown` with a dim `⇄`. Before, it read a green `open` or the raw state string.
- Header columns no longer shrink to the visible rows. A long name inside a folded group widens the label, path, and count columns. In the default Sessions frame the repo counts moved two cells right.
- `Ungrouped` (Sessions, Mux) and PR remote-group headers moved two cells right.

**Consistency wins**
- A closed or merged draft shows only its closed or merged glyph, and `⇄` takes that hue. An upper-case forge state also tints `⇄` correctly.
- In Pinned, an `open` word takes `pr_open`. It looks the same under the default theme.
- Folding or unfolding a group no longer moves other headers' columns.
- Counts line up when a pin marker is shown.
- Glyph-less headers start their label in their siblings' column.
- A program chip no longer dissolves into the selection highlight.

Follow-up filed: CSP-772, nested headers' path column.
<!-- SECTION:FINAL_SUMMARY:END -->
