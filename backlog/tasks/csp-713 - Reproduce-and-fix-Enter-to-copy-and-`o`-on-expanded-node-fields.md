---
id: CSP-713
title: Reproduce and fix Enter-to-copy and `o` on expanded node fields
status: Done
assignee:
  - '@claude'
created_date: '2026-10-27 20:26'
updated_date: '2026-10-28 06:35'
labels:
  - tui-framework
  - tuifw-cleanup
milestone: m-25
dependencies: []
documentation:
  - docs/plans/tui-framework/inventory/07-chrome-and-explorer.md
priority: medium
type: bug
ordinal: 799000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Enter-to-copy and `o` (inspect the full value) index `core_fields` while the rendered rows index the expanded field list, so with `F` on, the extra fields do nothing. No test covers it. The inventory found this by reading (slice 07, section 3.17). The future `PropertyGrid` copy and inspect outcomes depend on the right index, so the defect is settled before that extraction.

Scope: Write a failing test that expands the field list and copies or inspects an extra field. Fix the indexing in the explorer. If it does not reproduce, record that in the inventory file and close this task.

TUI changes:
- Consistency wins: copy and inspect work on every field the pane shows
- Regressions or behavior changes: none expected

Tests and checks: A regression test covers `F` on and off for copy and inspect. Manual check with `conspectus tui --snapshot --snapshot-keys` on the showcase fixture.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 The defect is reproduced by a failing test, or recorded as not reproducible in the inventory file
- [x] #2 With `F` on, Enter-to-copy and `o` act on the field under the cursor for every shown field
- [x] #3 A test covers copy and inspect with the field list expanded and collapsed
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Reproduce with `conspectus tui --snapshot` on the showcase fixture (keys `jjjjjljjjj<Tab>Fjjjjjjjjjo`): `o` on `last_message_preview` reports no truncated value.
2. Write failing tests first: Enter-to-copy and `o` on every shown field with Expanded Node Detail off and on, a field past the five core rows, and one key-driven test through the snapshot driver.
3. Fix at the root: `ExplorerRow::NodeField` carries its own `value`; `explorer_copy_target` and `open_value_modal_for_cursor` read the row instead of looking the index up in `core_fields`.
4. Confirm with `--snapshot`; record the verification in inventory slice 07 and the changelog; run fmt, clippy, the isolated suite, docs build, `git diff --check`.
5. Close CSP-771 (the same defect from the `o` side) with a final summary that points here.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Reproduced with `conspectus tui --snapshot` on the showcase fixture (keys `jjjjjljjjj<Tab>Fjjjjjjjjjo`): the status line read "explorer: row has no truncated value to open". Five tests failed before the fix and pass after it. `ExplorerRow::NodeField` now carries its own `value`; `explorer_copy_target` and `open_value_modal_for_cursor` read the row. Checks: fmt, clippy (workspace, all targets, all features, -D warnings), the isolated suite (3485 passed, 0 failed, 1 ignored; the first run hit two timing flakes in `discovery::forge` tests that pass alone and on rerun), cargo doc with -D warnings, git diff --check.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Enter-to-copy and `o` now act on the field under the cursor for every field the right pane shows, with Expanded Node Detail (`F`) on or off.

Cause: `ExplorerRow::NodeField { index, .. }` numbers rows over `NodeView::fields(expanded_detail)`, but `App::explorer_copy_target` and `App::open_value_modal_for_cursor` looked the index up in `core_fields`. Past the five core rows the lookup missed, so Enter copied nothing and `o` reported "row has no truncated value to open".

Fix: the row carries its own `value` beside `label` and `long_value` (`src/tui/explorer.rs`), and both gestures read the row (`src/tui/app.rs`) instead of a second lookup by index. The same row data is what the future `PropertyGrid` copy and inspect outcomes need.

Verification:
- Reproduced before the fix with `conspectus tui --snapshot` on the showcase fixture (`jjjjjljjjj<Tab>Fjjjjjjjjjo`: status line "explorer: row has no truncated value to open"). After the fix the same keys open the modal titled `last_message_preview` with the full text; `jjjjjljjjj<Tab>Fjjjjjjjjj<Enter>` shows the toast "copied: last_message_preview"; checked at 150x34 and 70x24.
- Tests, each failing before the fix: `enter_copies_the_field_under_the_cursor_with_the_node_detail_collapsed_and_expanded` and `o_inspects_the_field_under_the_cursor_with_the_node_detail_collapsed_and_expanded` (every shown field, F off and on), `o_opens_a_truncated_field_past_the_core_rows_when_the_node_detail_is_expanded`, `enter_copies_a_field_past_the_core_rows_when_the_node_detail_is_expanded` (`src/tui/app_tests.rs`), and `key_prelude_o_and_enter_reach_a_truncated_field_past_the_core_rows` (`src/tui/snapshot_tests.rs`, real key path). They replace `open_value_modal_when_cursor_has_a_long_value`, which named the open case but only exercised the no-value branch.
- fmt, clippy (workspace, all targets, all features, -D warnings), the isolated suite (3485 passed, 0 failed, 1 ignored), cargo doc with -D warnings, and git diff --check all pass.

Docs: inventory slice 07 gains a "Verification (CSP-713)" section; the changelog has a Fixed entry.

**Regressions**
- None.

**Consistency wins**
- Enter copies and `o` inspects every field the pane shows, including the extra fields under `F` (full id, state scope, last message preview, long commands), matching the `(truncated · o)` hint.

Also closes CSP-771, the same defect seen from `o`.
<!-- SECTION:FINAL_SUMMARY:END -->
