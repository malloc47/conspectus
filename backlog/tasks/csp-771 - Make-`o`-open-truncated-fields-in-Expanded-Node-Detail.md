---
id: CSP-771
title: Make `o` open truncated fields in Expanded Node Detail
status: Done
assignee:
  - '@claude'
created_date: '2026-10-28 05:52'
updated_date: '2026-10-28 06:36'
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
- [x] #1 In Expanded Node Detail, `o` on any row showing `(truncated · o)` opens the full-value modal with that field's label and full value
- [x] #2 A test covers a truncated field beyond the core rows in expanded mode
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Same defect as CSP-713 seen from `o`: fix at the root there (the row carries its own value and long value) and cover it with the shared tests.
2. Close with a final summary pointing at CSP-713.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Same defect as CSP-713, found from the `o` side; the fix is there (the NodeField row carries its own value and long value). `o` on the `last_message_preview` row of `showcase-claude` with `F` on now opens the modal instead of reporting no truncated value.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Fixed together with CSP-713, which is the same defect: with Expanded Node Detail on (`F`), `o` and Enter-to-copy looked the cursor's `NodeField` index up in `core_fields` while the rows number `fields(expanded_detail)`. The row now carries its own `value` and `long_value`, and `open_value_modal_for_cursor` reads the row.

Verification:
- `jjjjjljjjj<Tab>Fjjjjjjjjjo` on the showcase fixture under `conspectus tui --snapshot` now opens the full-value modal titled `last_message_preview` instead of "explorer: row has no truncated value to open".
- AC 1: `o_inspects_the_field_under_the_cursor_with_the_node_detail_collapsed_and_expanded` presses `o` on every shown field; each field with a full value opens a modal with its label and full text, and the rest open nothing.
- AC 2: `o_opens_a_truncated_field_past_the_core_rows_when_the_node_detail_is_expanded` (`src/tui/app_tests.rs`) and `key_prelude_o_and_enter_reach_a_truncated_field_past_the_core_rows` (`src/tui/snapshot_tests.rs`) cover a truncated field past the core rows in expanded mode. Both failed before the fix.
- See the CSP-713 final summary for the full check list.

**Regressions**
- None.

**Consistency wins**
- `o` opens the full value for every row that shows `(truncated · o)`, expanded or not.
<!-- SECTION:FINAL_SUMMARY:END -->
