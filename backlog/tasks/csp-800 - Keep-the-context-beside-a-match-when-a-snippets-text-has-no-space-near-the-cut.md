---
id: CSP-800
title: >-
  Keep the context beside a match when a snippet's text has no space near the
  cut
status: Done
assignee:
  - '@claude'
created_date: '2026-10-31 01:48'
updated_date: '2026-10-31 02:45'
labels:
  - tui-framework
milestone: m-25
dependencies: []
references:
  - crates/tuikit/src/text/snippet.rs
  - src/cli/search.rs
  - src/viewer/search.rs
priority: medium
type: bug
ordinal: 886000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found while moving `conspectus search`'s table onto `tuikit::text::fit_snippet` (CSP-774). The fit moves a cut that would split a word to the first blank between the cut and the match, even when that blank is the one next to the match, so all the context on that side goes. Its own doc says the move happens "unless that would give up the room next to the match". Text without spaces near the match loses everything: `日本語のテキストと説明 wombat もっと日本語のテキストと説明がここに続きます` fits a 63-cell budget as `…wombat…`, where the CLI's old centered crop showed `日本語のテキストと説明 wombat…`. A long token beside the match (a path, a URL) does the same in English text.

Every snippet goes through this fit: the TUI's content-search rows and Search view, quick select's match line, the stored hit snippet (`snippet_for`, so `conspectus search --format json`'s `snippet.text` for long bodies), and now the CLI table.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 A cut whose only word edge on its side is next to the match keeps context there (cut between graphemes, or at an ideograph boundary) instead of dropping the side
- [x] #2 Tests in the framework's snippet tests cover CJK text without spaces and a long token beside the match at several budgets; the CLI table's wide-character test asserts context shows
- [x] #3 Changed frames and stored snippets are reviewed and listed; the JSON shape is unchanged
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Cause: `crop` moves a cut that splits a word to the first blank between the cut and the match even when that blank is the one beside the match, so the side goes; and a run of CJK text has no blanks, so the whole run is one word.
1. Boundary rule (tuikit::text::snippet): a cut falls on a word's edge at either end of the text, beside a blank, or beside a wide character (`Cluster::is_wide`, two cells: CJK, full-width forms, emoji; scripts set in wide characters put no spaces between words and may break between any two characters). Measured with the unicode-width/segmentation ratatui already brings; no new crate. Text without wide characters has exactly the old edges.
2. Snap rule: a cut that splits a word moves to the nearest edge toward the match, unless that edge leaves no context on that side (the only edge is next to the match); then the cut stays between graphemes. This is what the fit's doc already promised ("unless that would give up the room next to the match"). Spaced text whose snap keeps a word is unchanged; a fit that showed nothing on one side now shows the cut word there (AC #1). Judgment call recorded in operator-review.
3. Tests: snippet_tests.rs: the task's CJK example at 63 cells exactly and at several budgets (context both sides), CJK with no spaces at all, a path and a URL beside the match at several budgets, spaced text with a whole word still snapping (exact strings), a one-cell room. text_tests.rs: property test over generated CJK, mixed, emoji, and combining text: width within the budget, the match whole when it fits, and a non-blank grapheme on each side whenever that side's guaranteed room reaches one. cli/search_tests.rs: the wide-character test asserts context on both sides.
4. Before/after: CLI `search` table at 50..120 columns and JSON, and the TUI content-search modal (sessions and messages lists, 140/100/60 columns) on CJK, mixed, and long-token fixtures (/tmp/tuifw-baseline/frames-800-*); JSON shape unchanged.
5. Docs: fit_snippet rustdoc, crates/tuikit/docs/controls/text.md (Snippets), ADR 0155 status note, operations guide and CHANGELOG, operator-review row.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented in `crates/tuikit/src/text/snippet.rs`: `is_edge` (a cut falls on a word's edge at a text end, beside a blank, or beside a wide grapheme, `Cluster::is_wide` in measure.rs), and the snap keeps the grapheme cut when the only edge on its side is the one beside the match. Old-vs-new comparison over generated English (budgets 8..width, ~141k fits): 15% differ, every one a fit that had an empty side; no other spaced fit changed. The new property test `snippets_keep_context_beside_the_match_when_room_allows` fails on the old fit (`…x京y…` at 10) and passes on the new. Deliberately updated: `cli::search` `a_cropped_snippet_keeps_the_match_and_cuts_on_word_edges` (at 14 cells the old fit dropped both sides, `…needle…`; now `…a needle eps…`), and two tui-columns frames regenerated with `just tui-columns` (Search view hit and pane-hit rows). Before/after: /tmp/tuifw-baseline/frames-800-{before,after} (CLI table 50..120 columns, JSON, content-search modal sessions/messages at 140/100/60 columns). JSON: only the long CJK body's stored `snippet.text` and highlight changed (`…wombat…` to context on both sides); shape unchanged.
Checks: full isolated suite 4818 passed, 0 failed, 1 ignored (no rerun); fmt; clippy workspace all features; `just tuikit-features` (clippy and tests features off and richtext); tuikit clippy all targets; RUSTDOCFLAGS=-D warnings cargo doc; cargo build --examples; git diff --check.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
A cut snippet keeps the text beside its match. The fit (`tuikit::text::fit_snippet`, which every surface uses) moved a cut that split a word to the first blank toward the match, even when that blank was the one beside the match, so Japanese text with spaces only around the match, or a path or URL beside it, showed `…wombat…`.

- Boundary rule: a cut falls on a word's edge at either end of the text, beside a blank, or beside a wide grapheme (`Cluster::is_wide`: two cells, so CJK, full-width forms, emoji), measured with the unicode-width/segmentation ratatui already brings; no new crate. Text without wide characters has exactly the old edges.
- Snap rule: a cut that splits a word moves to the nearest edge toward the match unless the only edge on that side is the one beside the match; then it stays between graphemes and the side shows the part that fits (what the fit's doc already promised).
- Tests: snippet_tests.rs (the story's CJK example exactly at 63 cells and context on both sides from 16 cells up, CJK with no spaces from 13, a path and URL beside the match at several budgets, English still cut at whole words where one fits, a one-blank room); a property test over generated CJK, emoji, combining, narrow-token, and mixed text (within the budget, only a piece of the source, the match whole, a non-blank grapheme on each side whenever that side's guaranteed room reaches one), which fails on the old fit; the CLI wide-character test asserts context on both sides.
- Deliberately updated: the CLI word-edge test (at 14 cells `…a needle eps…`, was `…needle…`) and two tui-columns frames (`just tui-columns`).
- Docs: rustdoc, framework text page, ADR 0155 status, tui-columns prose, operations guide, CHANGELOG, operator-review row (the English trade-off).

**Regressions:**
- English (or any narrow text) whose only word on one side is wider than that side's room shows a cut word where it showed nothing: `…wombat ten…` reads `…ine wombat ten…` (50-column table, 60-column modal); Search view `…xpand the notes into a…`, `…mit, feature notes,…` (were `…the notes into a…`, `…feature notes,…`). The brief asked for identical spaced text; that holds wherever a whole word fits (15% of generated English fits changed, all of them fits with an empty side). Flagged for review.

**Consistency wins:**
- CJK beside a spaced match: `…のテキストと説明 wombat もっと日本語のテキストと説明がここに…` (was `…wombat…`) in the table, the modal's session and message rows, the Search view, and quick select.
- A long token beside the match: `…config.toml wombat https://example.com/a/…` (was `…wombat…`); mixed text `…互に wombat 出てくる…` (was `…wombat 出てくる…`).
- The stored snippet of a long CJK body (`snippet.text` in `--format json`): context on both sides (was `…wombat…`); JSON shape unchanged.

Checked: before/after in /tmp/tuifw-baseline/frames-800-* (CLI 50 to 120 columns and JSON; content-search modal at 140, 100, 60 columns); CJK with no spaces at all was already right and is unchanged. Full isolated suite 4818 passed, 0 failed, 1 ignored; fmt, both clippy runs, `just tuikit-features`, doc, examples, `git diff --check` pass.
<!-- SECTION:FINAL_SUMMARY:END -->
