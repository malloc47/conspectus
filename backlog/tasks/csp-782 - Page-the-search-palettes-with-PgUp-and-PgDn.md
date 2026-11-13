---
id: CSP-782
title: Page the search palettes with PgUp and PgDn
status: Done
assignee:
  - '@claude'
created_date: '2026-10-29 14:03'
updated_date: '2026-10-30 21:46'
labels:
  - tui-framework
milestone: m-25
dependencies:
  - CSP-743
priority: low
type: enhancement
ordinal: 868000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Quick select and content search are palettes (ADR 0175), and the palette pages by the rows shown with `PgUp`/`PgDn` (ADR 0174, house rule 12), but `SEARCH_KEYS` and `SEARCH_CONTENT_KEYS` in `src/tui/keybindings/overlay.rs` bind neither, so in the search the two keys do nothing while the picker pages. CSP-743 kept the search's key tables as they were, since that story changed no search behavior.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 In quick select and content search, PgUp and PgDn move the cursor by the rows shown and stop at the ends, as in the picker
- [x] #2 The keys are aliases: the search footers are unchanged
- [x] #3 A test drives both keys in each mode, and the key-table checks pass
- [x] #4 The operator guide's quick select and content search sections name the keys
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Add `SearchKey::PageUp`/`PageDown` (role `Page`), mapped to `PaletteKey::PageUp`/`PageDown` in the search's `PaletteAction` impl, so the palette pages by the rows its window shows and stops at the ends.
2. Bind `PgUp`/`PgDn` as aliases in `SEARCH_KEYS` and `SEARCH_CONTENT_KEYS` (footers unchanged); content search's verb match lists the two as palette keys.
3. Tests: quick select (current reach) and content search each drawn tall enough to overflow, then PgDn/PgUp by the page and stopping at both ends; key-table registry checks (`mislabeled`, `unresolved`, `loose_characters`, `ctrl_c_first`, convention) stay green; footer strings unchanged.
4. Operator guide: name `PgUp`/`PgDn` in the quick select and content search sections; CHANGELOG entry.
5. Snapshot check at 100x30 and 60x20 (footer unchanged, paged cursor visible), full checks, final summary.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented in e985c535: `SearchKey::PageUp`/`PageDown` (role Page) mapped to the palette's page actions, aliases in both search tables. Tests `page_keys_move_by_the_rows_shown_and_stop_at_the_ends` in `search_tests.rs` (quick select through `handle_in`, so the re-rank after each key keeps the paged cursor) and `search_content_tests.rs` (content search, 40 results at 100x24; paging asks for no new scan). Snapshot frames in /tmp/tuifw-baseline/frames-782-after: at 60x20 quick select (17 results, 10 shown) PgDn moves 0 to 10, stops at 16, PgUp from 16 goes to 6; at 100x30 all 17 fit, so PgDn goes to the last; content search (7 results) PgDn goes to the last; footers unchanged in every frame. Checks: fmt, clippy -D warnings, full isolated suite 4767 passed / 0 failed / 1 ignored, cargo doc -D warnings, cargo build --examples, git diff --check, tuikit independence.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Quick select and content search now page with `PgUp`/`PgDn`: `SearchKey::PageUp`/`PageDown` are aliases in `SEARCH_KEYS` and `SEARCH_CONTENT_KEYS`, handed to the palette's page actions, so the cursor moves by the rows the list shows and stops at the first and last result, as in the picker. Footers are unchanged (the keys are aliases). The operator guide's quick select and content search sections name the keys; CHANGELOG and ADR 0175's consequence are updated.

Verification: a test per mode drives PgDn to the page, past the end, PgUp back by a page and past the start (`page_keys_move_by_the_rows_shown_and_stop_at_the_ends` in `search_tests.rs` and `search_content_tests.rs`); the key-table registry checks and the footer and search snapshots pass unchanged; `conspectus tui --snapshot` at 100x30 and 60x20 (frames in /tmp/tuifw-baseline/frames-782-after). fmt, clippy, full isolated suite (4767 passed, 0 failed, 1 ignored), cargo doc, examples, and `git diff --check` pass.

**Regressions**
- None.

**Consistency wins**
- `PgUp`/`PgDn` page quick select and content search by the rows shown, stopping at the ends (they did nothing); at 60x20 quick select's PgDn moves the cursor from the first result to the eleventh, the first row of the next page.
<!-- SECTION:FINAL_SUMMARY:END -->
