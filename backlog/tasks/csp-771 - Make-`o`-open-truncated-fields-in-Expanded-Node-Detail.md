---
id: CSP-771
title: Make `o` open truncated fields in Expanded Node Detail
status: To Do
assignee: []
created_date: '2026-10-28 05:52'
labels:
  - tui
dependencies: []
priority: low
type: bug
ordinal: 857000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found while verifying CSP-712 with `conspectus tui --snapshot`. With Expanded Node Detail on (`F`), a field past the five core rows that is cut short shows the `(truncated · o)` hint, but pressing `o` reports "explorer: row has no truncated value to open". `App::open_value_modal_for_cursor` looks the cursor's `ExplorerRow::NodeField { index, .. }` up in `view.core_fields`, while `NodeView::flat_rows` numbers the rows over `fields(expanded_detail)`, which in expanded mode is the longer per-kind list. The row already carries its own `long_value`. Reproduce on the showcase fixture: keys `jjjjjljjjj<Tab>Fjjjjjjjjjo` (the `last_message_preview` row of `showcase-claude`).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 In Expanded Node Detail, `o` on any row showing `(truncated · o)` opens the full-value modal with that field's label and full value
- [ ] #2 A test covers a truncated field beyond the core rows in expanded mode
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
