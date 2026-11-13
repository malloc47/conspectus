---
id: CSP-794
title: Let `SectionRule` give way when its chip and summary do not fit
status: Done
assignee:
  - '@claude'
created_date: '2026-10-30 10:18'
updated_date: '2026-10-30 20:40'
labels:
  - tui-framework
milestone: m-25
dependencies:
  - CSP-791
priority: low
type: bug
ordinal: 880000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
`tuikit::frame::SectionRule` (CSP-791) draws a rule, a label chip, an optional summary, and a two-cell closing rule; only the leading rule gives way. Below the width of the chip, the summary, and the closing rule the line is wider than asked, so the explorer's wrap folds the right pane's `Related` rule onto a second row with the `──` stranded under it. Seen at 40 columns (`conspectus tui --snapshot --snapshot-width 40 --snapshot-height 50 --snapshot-pane right --snapshot-keys "jjjjjljjjj<Tab>G"` on the showcase fixture: `2 validated · 2 other · 2 ▲  Related` then `──`), and possible at 100 columns with two-digit counts. CSP-791 kept the behavior so its frames stayed byte-identical; the frame page lists it as a seam. A natural rule: cut the summary first (with `…`, via `text::elide` or `text::fit_spans`), then the label, keeping the closing rule, so the line never exceeds its width.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 A `SectionRule` line is never wider than its width; the summary is cut with `…` before the label is
- [x] #2 The right pane's `Related` rule stays on one row at 40 columns, checked with `conspectus tui --snapshot`
- [x] #3 Framework tests cover the cut summary, the cut label, and widths below the closing rule
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. `SectionRule::line` fits its pieces into the width the closing rule leaves: the label and summary whole when they fit; else the summary cut with `…` (at least one character before the mark, a space before the mark dropped) or gone, the label whole; else the label cut the same way, no summary; else a plain rule of the width (nothing at 0). Cut by `text::truncate_end`, not `text::elide` (which drops a ranked piece whole).
2. Tests: the cut summary, the summary leaving, the cut label, the chip leaving, two-cell labels, widths below the closing rule, and a sweep that a line is exactly its width at 0 to 59 cells for several rules; the type's doc example shows the steps.
3. Docs: the frame page's `SectionRule` section and API row, its seam removed, rule 14; ADR 0188's record, design.md, CHANGELOG, operator review.
4. Verify with `conspectus tui --snapshot` of the right pane at 40 to 160 columns (the task's command among them) before and after.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented the give-way order in SectionRule::line (fitted and cut helpers); 5 of 173 snapshot frames differ, all the 40-column Related rule. Full suite 4759 passed, 0 failed, 1 ignored.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
A `SectionRule` line is never wider than its width (commits 96ff634d, ecdba252). Below the width of its chip, summary, and closing rule the summary is cut with `…` (one character before the mark at least; a cut ending at a space drops it so the mark follows the last word, and the leading rule takes the cell) and leaves with its spaces once nothing would show; then the label is cut the same way; then the chip leaves and the line is a plain rule of its width in `muted`; the closing rule stays throughout. The cut is `text::truncate_end`, not `text::elide`, which drops a ranked piece whole while the summary's first words say the most. Docs: the frame page (the `SectionRule` section with the steps and an example, the API row, rule 14, the seam removed), ADR 0188's record of the rule, design.md, CHANGELOG, operator review.

Verification: framework tests for the cut summary (`3 shown · 1 mo…`, `3…` with the space dropped), the summary leaving, the cut label (`Relat…`, `R…`), the chip leaving for a plain rule, two-cell labels, widths 0 to 2 below the closing rule, and a sweep that the line is exactly its width at every width from 0 to 59 for four rules (one with trailing spaces); the type's doc example walks the steps. `conspectus tui --snapshot --snapshot-width 40 --snapshot-height 50 --snapshot-pane right --snapshot-keys "jjjjjljjjj<Tab>G"` on the showcase fixture now shows `2 validated · 2 other ·…  Related ──` on one row. Before/after whole frames: 11 right-pane selections at 40, 50, 60, 80, 100, 130, and 160 columns by 50 rows, and every view at 160, 100, 60, and 40 by 40 (173 frames): 5 differ, all at 40 columns, all this change. fmt, clippy `-D warnings`, the independence guard, the isolated suite (4759 passed, 0 failed, 1 ignored), rustdoc `-D warnings`, `cargo build --examples`, and `git diff --check` pass. Frames in /tmp/tuifw-baseline/frames-794-{before,after}.

**Regressions**
- At 40 columns the `Related` rule's counts are cut (`2 validated · 2 other ·…`): the last count (`2 ▲`) no longer shows on the rule.

**Consistency wins**
- At 40 columns the `Related` rule stays on one row with its closing `──` beside the chip, as every other rule's is, instead of folding onto a second row with `──` stranded under it (Sessions, Mux, Pinned, and Search views alike); the Preview below gets the row back.
<!-- SECTION:FINAL_SUMMARY:END -->
