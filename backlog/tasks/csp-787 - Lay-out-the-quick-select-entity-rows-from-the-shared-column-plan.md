---
id: CSP-787
title: Lay out the quick-select entity rows from the shared column plan
status: Done
assignee:
  - '@claude'
created_date: '2026-10-30 05:25'
updated_date: '2026-10-30 21:25'
labels:
  - tui-framework
  - tuifw-data
milestone: m-25
dependencies:
  - CSP-753
priority: low
type: enhancement
ordinal: 873000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
CSP-753 unified the hit columns of the search modal's content rows and the Search view's hit, pane, and pane-hit rows into one declaration. The quick-select entity rows (All scope) still use their own column split, because they need two width rules the shared plan does not express yet. Add those rules to the column schema generically and move the entity rows onto the shared declaration, so a search result has one column definition everywhere.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 The quick-select entity rows are laid out from the shared column plan, and the two missing width rules are added to tuikit::columns with tests
- [x] #2 Quick-select entity rows look the same at wide widths and drop cells by the one narrow-width policy, checked with conspectus tui --snapshot at a normal and a narrow width
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Name the two rules the entity split needs (CSP-743/753 seams): a share of what the other columns leave, and a fill that hands its cells to a neighbor when it leaves. Define the first so it reproduces the split's "what the glyph and the word leave": the room less every column that is neither such a share nor a fill, and the gaps beside those columns.
2. `tuikit::columns`: `Width::ShareOfRest` and `Column::share_of_rest` (bounded by `within`), worked out after the other rules (lined up and after a hugging column); `Column::gives_to(key)`, by which a fill that does not show (dropped, or no cell in the list) hands what is left to that column. Unit tests, doc examples, and the random property schemas.
3. Host: `entity_columns()` schema (glyph, label 45% of the rest within 16..48 narrowing last, context fill at least 10 dropping and giving to the label, outcome a third of the row within 0..16 at the right edge) laid out by `EntityGrid` (measured once a frame over the entity results); remove the hand split; a test holds the schema to the hand split at every width 8..260.
4. Docs: columns and palette pages (rules, seams removed), ADR 0196, ADR 0175/0183 status, design.md, operator review; `just tui-columns` to confirm no documented frame moves.
5. Verify with `conspectus tui --snapshot` of quick select's wider reach over ten queries at 200, 160, 130, 100, 80, 60, 50, and 40 columns, before and after.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Added share_of_rest and gives_to to tuikit::columns; entity rows on entity_columns()/EntityGrid; schema equals the hand split at every width 8..260; 160 frames read the same (56 differ in blank-pad style only). Full suite 4765 passed, 0 failed, 1 ignored.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Quick select's entity rows are laid out from a column schema, and the two width rules they needed are in `tuikit::columns` (commits 6ffcaeb4, 0372f9d3, 4e828253). `Column::share_of_rest(key, per, of)` (`Width::ShareOfRest`, bounded by `within`) takes a part of the room less every column that is neither such a share nor a fill and the gaps beside them; `Column::gives_to(key)` makes a fill that does not show (dropped below its minimum, or no cell in the list) hand what is left to the named column. The host's `entity_columns()` declares the glyph, the label (`share_of_rest(45, 100).within(16, 48)`, narrowing last), the context (`fill().at_least(10).drops(0).gives_to(Label)`), and the outcome word (`share(1, 3).within(0, 16).right()`); `EntityGrid` measures the entity results once a frame and lays each out by the one narrow-width policy, so every search result (content hit and entity) is a column declaration. The hand split (`entity_columns(inner)`, `entity_cells`, `ROW_LEAD`, `COLUMN_GAP`) is gone. Docs: the columns page (the rules, the policy's steps, the seam removed), the palette page's seam removed, ADR 0196 (ADRs 0175 and 0183 amended), design.md, operator review.

Verification: framework tests for a share of what is left (the gaps it counts, a column left below its room not counted, after a hugging column), for a fill handing its cells on (dropped, absent from the list, and none named), doc examples for both, and the random property schemas drawing both (never wider than the room, lined-up columns in one place, a bar's cell given by the fill). A host test holds the schema to the old hand split at every width from 8 to 260 cells (identical), and the existing entity-fit test passes on the schema. `conspectus tui --snapshot` before/after of quick select's wider reach (`/<Tab>` with ten queries) at 200x40, 160x40, 130x30, 100x30, 80x24, 60x20, 50x16, and 40x10, and the framed modals at 100x30 and 40x10: 160 frames, every one with the same text; 56 differ only in the style of blank pad cells (a context's and the word's pad take their text's receding look), which shows nowhere. `just tui-columns` changes no documented frame. fmt, clippy `-D warnings`, the independence guard, the isolated suite (4765 passed, 0 failed, 1 ignored), rustdoc `-D warnings`, `cargo build --examples`, and `git diff --check` pass. Frames in /tmp/tuifw-baseline/frames-787-{before,after}.

**Regressions**
- None seen: no captured frame reads differently.

**Consistency wins**
- The entity rows give way by the one narrow-width policy every other row uses (the context leaves first and the label takes its cells, then the label narrows), declared once beside the content hit's columns.
- By the rules rather than by hand, two cases the fixture does not show change: a list in which no entity has a context gives the label the row up to the word (it padded an empty column), and a label cut across its match picks out the visible part of the match.
<!-- SECTION:FINAL_SUMMARY:END -->
