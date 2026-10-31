---
id: CSP-774
title: 'Crop CLI search snippets with tuikit::text::fit_snippet'
status: Done
assignee:
  - '@claude'
created_date: '2026-10-28 13:57'
updated_date: '2026-10-31 01:55'
labels:
  - tui-framework
milestone: m-25
dependencies:
  - CSP-723
priority: low
type: enhancement
ordinal: 860000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
`conspectus search` crops its table's snippet column with `cli/search.rs` `crop_around`, a third snippet cropper next to the one the TUI now uses (`tuikit::text::fit_snippet`, ADR 0155). `crop_around` grows the window one character at a time on alternating sides, so it centers the match, then moves each cut to a word edge; the framework favors the text after the match (two thirds of the spare cells) and cuts on word edges. The CLI already shares the framework's cell measure and end cut (`output::render::display_width` and `truncate_to_width` delegate to `tuikit::text`).

Scope: replace `crop_around` with `tuikit::text::fit_snippet` (no dropped characters, or decide whether Markdown ticks should go as they do in the TUI), keep `fit_snippet`'s bold highlight in `cli/search.rs`, and port or retire the `crop_around` tests in `cli/search_tests.rs`.

This changes what `conspectus search` prints when a snippet is cut: the window moves toward the text after the match. Decide whether that is wanted for the CLI before landing it; if the CLI should keep a centered crop, record why and close this.

Tests and checks: the `cli/search_tests.rs` width and floor tests pass; a snippet cut at several widths keeps its match and fits; `tests/cli_search.rs` passes.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 The CLI search table crops snippets with tuikit::text::fit_snippet, or the reason it keeps its own crop is recorded
- [x] #2 cli/search_tests.rs covers the crop at several widths, with wide characters
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Decision: land it (consistency-first, ADR 0151): one snippet fit for every surface; the stored snippet the CLI cuts is already cropped by `fit_snippet` (two thirds after the match), so a centered display crop was a second algorithm on top of it. Keep every character (no Markdown tick dropping), so the table reads as the JSON's `snippet.text`.
1. Replace `crop_around` with `tuikit::text::fit_snippet(snippet, highlight, budget, &[])`; the uncut path goes through it too (budget unbounded), which validates the highlight before bolding. Keep the bold highlight and the 20-cell floor.
2. Port the `crop_around` tests in `cli/search_tests.rs` to the table's fit: several widths, word edges, after-favoring, wide characters (CJK snippet and a wide match), a match wider than the budget.
3. Add a `tests/cli_search.rs` case: a long message at several `--width`s keeps the match, cuts on words, reads on after the match; the JSON's snippet stays whole.
4. Capture before/after CLI output; operations guide, changelog, ADR 0155 status note, operator-review row; file the framework fit's CJK weakness found on the way.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Verified: `cli::search` unit tests (21) incl. the ported `a_cropped_snippet_keeps_the_match_and_cuts_on_word_edges` (6 widths), new `a_cut_gives_the_text_after_the_match_more_room` (exact `…seven needle eight nine ten…` at 30), `a_wide_character_snippet_is_cropped_by_cells_without_splitting` (6 widths plus a wide match at 4), `a_match_wider_than_the_budget_is_itself_cut`; `tests/cli_search.rs` (25) incl. new `a_cut_table_snippet_keeps_the_match_and_reads_on_after_it` (80/100/120 columns, JSON snippet whole with highlight [45, 51]); `tests/search_docs.rs` passes. Before/after CLI output: /tmp/tuifw-baseline/frames-774-{before,after}.txt. Found: CJK text without spaces near the match now shows only the match once cut (`…wombat…`), the framework fit's word-edge snap; filed CSP-800. Full isolated suite 4811 passed, 0 failed, 1 ignored; fmt, both clippy runs, doc, examples, diff --check pass.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`conspectus search`'s table crops its snippet column with `tuikit::text::fit_snippet`, the fit every TUI surface and the stored hit snippet already use; `crop_around` is gone. Decided for the CLI by the consistency-first rule (ADR 0151): the snippet the table cuts was already cropped by that fit (favoring the text after the match), so a centered display crop was a second algorithm on top of it. No characters are dropped (the TUI drops Markdown ticks from its rows), so the table reads as the JSON's `snippet.text`. The bold highlight and the 20-cell floor stay; the uncut path validates the highlight through the same fit before bolding it. JSON output is unchanged.

- Tests: the `crop_around` tests in `cli/search_tests.rs` ported to the table's fit at several widths (word edges, after-favoring with an exact string, CJK text and a wide match, a match wider than the budget); `tests/cli_search.rs` gains a long-message case at three `--width`s that also checks the JSON keeps the snippet whole.
- Docs: operations guide (the table's snippet), changelog, ADR 0155 status note, operator-review row. Follow-up CSP-800 (filed): the framework fit drops all context beside a match in text with no space near the cut.

**Regressions:**
- CJK (space-less) text cut to `--width` can show only the match (`…wombat…` where the centered crop showed the text before it), as the TUI already does; CSP-800 fixes the fit for every surface.

**Consistency wins:**
- A cut snippet reads on from the match as in the TUI: at 80 columns `…eight nine wombat ten eleven twelve…` (it read `…seven eight nine wombat ten eleven…`); one fit for the CLI table, the TUI rows, and the stored snippet.

Checked by running the CLI before and after (`/tmp/tuifw-baseline/frames-774-*.txt`, widths 60 to 120 and JSON; the TUI is untouched). Full isolated suite 4811 passed, 0 failed, 1 ignored; fmt, both clippy runs, doc, examples, and `git diff --check` pass.
<!-- SECTION:FINAL_SUMMARY:END -->
