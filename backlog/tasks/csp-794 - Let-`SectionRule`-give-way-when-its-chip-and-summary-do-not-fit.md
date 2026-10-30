---
id: CSP-794
title: Let `SectionRule` give way when its chip and summary do not fit
status: To Do
assignee: []
created_date: '2026-10-30 10:18'
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
- [ ] #1 A `SectionRule` line is never wider than its width; the summary is cut with `…` before the label is
- [ ] #2 The right pane's `Related` rule stays on one row at 40 columns, checked with `conspectus tui --snapshot`
- [ ] #3 Framework tests cover the cut summary, the cut label, and widths below the closing rule
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
