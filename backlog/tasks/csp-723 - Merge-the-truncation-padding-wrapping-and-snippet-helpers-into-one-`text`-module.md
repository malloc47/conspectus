---
id: CSP-723
title: >-
  Merge the truncation, padding, wrapping, and snippet helpers into one `text`
  module
status: Done
assignee:
  - '@claude'
created_date: '2026-10-27 20:26'
updated_date: '2026-10-28 14:07'
labels:
  - tui-framework
  - tuifw-foundation
milestone: m-25
dependencies:
  - CSP-718
  - CSP-720
documentation:
  - docs/plans/tui-framework/inventory/01-design-tokens.md
  - docs/plans/tui-framework/inventory/05-search-help-viewers.md
priority: high
type: enhancement
ordinal: 809000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The TUI has 13 truncate/ellipsize/pad implementations (6 count `char`s instead of terminal cells), 6 word-wrap implementations, 2 snippet croppers, and 7 match painters. `tui::widgets` imports `crate::viewer::render::word_wrap`, a layering inversion. Wide characters misalign wherever a char-counting helper runs, and each fix has to be made in several places (inventory slices 01 and 05).

Scope: Create `tuikit::text` with cell-width measure, end and middle truncation, pad, fit-spans, word and character wrap, snippet fit (with the backtick stripping as a parameter), `find_ci`, and one match painter. Migrate all call sites. Keep the domain `compact_path_*` and `compact_mux_*` helpers in Conspectus. Record whether the CLI table renderer's `fit_to_width` shares the module.

TUI changes:
- Consistency wins: CJK and emoji no longer misalign where a helper counted chars; one ellipsis form (`…`) and one middle-ellipsis rule
- Regressions or behavior changes: truncation points move by a cell in places that counted chars; a few rows that wrapped differently from their siblings now wrap alike

Tests and checks: Property tests: output width never exceeds the budget for ASCII, CJK, emoji, and combining marks; truncation is idempotent. Snapshots at 80 and 40 columns for the left pane, forms, and search.

Blockers: The independence guard and the documentation skeleton.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 `tuikit::text` provides measure, truncate, middle-truncate, pad, fit, wrap, snippet fit, `find_ci`, and one match painter, all measuring terminal cells
- [x] #2 All 13 truncation, 6 wrap, 2 snippet, and 7 match-paint sites use the module; the old helpers are removed
- [x] #3 `tui::widgets` no longer imports anything from `crate::viewer` for text helpers
- [x] #4 Property tests cover ASCII, CJK, emoji, and combining characters
- [x] #5 The decision on sharing the module with the CLI table renderer is recorded
- [x] #6 Every visible change is listed in the final summary under **Regressions** and **Consistency wins**, and was checked with `conspectus tui --snapshot` at a normal and a narrow width
- [x] #7 The framework crate names no Conspectus type, string, theme field, or domain word, and the independence guard passes
- [x] #8 The control has a reference page in the framework docs: purpose, usage, key table, theme tokens used, and the house rules it follows
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Inventory (re-counted by grep at af977e9; the inventory's counts drifted):

Truncate / ellipsize / pad (19 implementations, 9 of them counting chars):
1-6. ui/text.rs truncate_to_width (alias), truncate_to_width_strict, truncate_to_width_middle, truncate_to_width_no_marker, pad_to_width, fit_spans_to_width (cells)
7. widgets/popup_frame.rs ellipsize (cells; width 0 returned the mark)
8-9. widgets/form.rs truncate_chars (chars, no mark) and fit (chars, pads)
10. widgets/badge.rs badge_span cut and pad (chars)
11. explorer.rs truncate (chars, 12 callers)
12. preview/process.rs command cap (chars, result one over the cap)
13. ui/header.rs render_find_chip query cut (chars)
14. ui/left_panel.rs search_header query cut (chars)
15. widgets/pane_viewer.rs find chip query cut (chars)
16. viewer/widget.rs find_chip_text query cut (chars)
17. widgets/snippet.rs head (cells)
18. preview_wrap.rs truncate (styled cells)
19. widgets/search.rs content_row_line local pad closure (ellipsize + pad)
Width sums: left_panel spans_width, header spans_cells, search cells; char-count width math feeding budgets in form.rs labels, right_panel breadcrumb and chips, pin_edit, mux_launch, viewer chip_label_width.
Kept in the host: compact_path_label, compact_path_secondary, compact_mux_label, compact_mux_native_id (domain); header truncate_chip_list (a list cap, not a width cut); output/dot.rs and output/html truncate (export labels, not terminal cells); form.rs text-field window (editing, CSP-733).

Word wrap (7): viewer/render.rs word_wrap, word_wrap_with_budgets, hard_break, wrap_styled_line, wrap_plain; widgets/form.rs wrap_words; widgets/form.rs wrap_footer (separator wrap); preview_wrap.rs word_wrap and hard_wrap; popup_frame::wrapped_lines (wrapper with indent); help.rs legend_entry and binding_lines (hanging indent through the viewer); ratatui Paragraph::wrap at 7 sites (left_panel banner and placeholder, preview_pane x2, right_panel explorer, worktree_menu, viewer banner) with the div_ceil estimators ui.rs wrapped_line_count (3 callers), the left_panel banner height, and the explorer per-line rows.

Snippet croppers (2 TUI + 1 CLI): widgets/snippet.rs fit (cells, ticks stripped); viewer/search.rs snippet_around (chars, centered; stored hit snippets, pane scan, quick select match line); cli/search.rs crop_around (CLI table).

Match painters (7): viewer/highlight.rs paint (reused by preview/search.rs mark_rows); pane_viewer highlighted; left_panel append_snippet; search content_row_line, build_match_line, entity_line; right_panel selected_hit_lines. Matcher: viewer/search.rs find_ci (re-exported by tui/search.rs).

Plan:
1. Add tuikit::text (measure, truncate_end, truncate_middle, clip, pad, pad_start, fit, fit_spans, wrap, wrap_chars, wrap_at, wrap_line, wrap_line_chars, fit_snippet, find_ci, find_all_ci, paint) on ratatui's grapheme segmentation; unit and property tests (hand-rolled generator: ASCII, CJK, emoji, VS16, ZWJ, combining); control page; module map; guard minimums.
2. Migrate truncation and padding sites; delete the generic half of ui/text.rs, ellipsize, form truncate_chars/fit, explorer truncate; domain compact helpers stay.
3. Migrate wrapping: viewer render, popup wrapped_lines, help, form wrap_words/wrap_footer, key-hint footers on wrap_at, preview_wrap on wrap_line/wrap_line_chars, Paragraph::wrap sites that count rows pre-wrapped (exact counts replace the div_ceil estimators).
4. Migrate snippets, find_ci, and painters; widgets no longer import crate::viewer for text helpers.
5. CLI: render.rs display_width/truncate_to_width delegate to tuikit::text; fit_to_width (column allocation) stays; record the decision.
6. Capture before/after --snapshot frames (/tmp/tuifw-baseline/frames-723-*), explain every difference; wide-character frames and tests.
7. ADR, design.md, operator-review, final checks.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Landed in seven commits: the module with unit and property tests and its page (13f1e6d), `clip_start` (7f01129), the cut and pad migration (c633094), the wrap migration with pre-wrapped row-counting surfaces (1d9285a), snippets, `find_ci`, and the painters (8b5158e), the CLI renderer delegation (f7b2df1), the explorer cut-detection fix with wide-text host tests (0eb737a), and the docs with ADR 0155 (5b96c91). Two fixups were squashed into their commits (a vocabulary word in a test, the tripwire expectation, a needless borrow); every commit builds and passes `cargo test -p tuikit`.

Frames: `/tmp/tuifw-baseline/capture-723.sh` renders 66 states at 100x30, 80x24, 60x24, and 40x20 for the showcase fixture and a wide-character copy (CJK, emoji, VS16, combining): 528 frames each into `frames-723-before` and `frames-723-after`, with intermediate captures after the cut slice (`frames-723-trunc`, 44 differ, all form-row `…` cuts and wide-text cuts) and the wrap slice (`frames-723-wrap`, 218 more, all wrap, footer, and explorer row-count changes); the snippet, CLI, and fix slices changed no captured frame. 264 of 528 differ overall; each was reviewed with `diff-723.sh` and `celldiff723.py` and falls in a category of the final summary (style-only diffs are a span's style on a trailing blank kept at a wrap break, invisible).

Checks: full isolated suite 3679 passed, 0 failed, 1 ignored (forge timing flakes seen in two intermediate lib runs, gone on rerun); fmt, clippy (workspace, all targets, all features, -D warnings), rustdoc with -D warnings, independence guard, and `git diff --check` pass. Filed CSP-774 for the CLI search's own snippet crop.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Landed `tuikit::text` (ADR 0155): `width`, `spans_width`, `truncate_end`, `truncate_middle`, `clip`, `clip_start`, `pad`, `pad_start`, `fit`, `fit_spans`, `wrap`, `wrap_chars`, `wrap_at`, `wrap_line`, `wrap_line_chars`, `fit_snippet` (dropped characters a parameter), `find_ci`, `find_all_ci`, and `paint`, the one match painter. Everything measures terminal cells and moves in whole graphemes (ratatui's own segmentation, no new dependency). Every TUI cut, pad, wrap, snippet, and painter site uses it; the old helpers are gone (`ui/text.rs` generic half, now `ui/compact.rs` with the domain `compact_*`; `ellipsize`, `form::truncate_chars/fit/wrap_words/wrap_footer`, `explorer::truncate`, `popup_frame::wrapped_lines`, the viewer's `word_wrap` family, `snippet.rs`, `snippet_around`, `MatchSnippet`, `highlight::paint`, `wrapped_line_count`, the inline char cuts and width sums). No production `Paragraph::wrap` remains: surfaces that count rows draw what they wrapped. Widgets import no text helper from `crate::viewer`. Control page `crates/tuikit/docs/controls/text.md`.

CLI decision: `output::render::display_width` and `truncate_to_width` delegate to the framework and the search table pads with it; `fit_to_width` is column allocation and stays (it belongs with `ColumnSchema`); the CLI search's centered `crop_around` is follow-up CSP-774.

Verified: property tests (fixed-seed generator over ASCII, CJK, emoji, VS16, ZWJ, combining) for budgets, idempotence, grapheme-safe wraps and snippets; host wide-text tests; `--snapshot` frames at 100x30, 80x24, 60x24, 40x20 (528 before and after, 264 differ, all explained below); full isolated suite 3679 passed, 0 failed, 1 ignored; fmt, clippy -D warnings, rustdoc -D warnings, independence guard, `git diff --check`.

**Regressions**
- A word that ends exactly at a row's edge stays on it, so some rows rewrap (help, viewers, modals, previews).
- The transcript viewer keeps blank lines between Markdown blocks (an old bug dropped them); transcripts are taller.
- The explorer drops its spare row: the preview starts a row higher, or the explorer's last row shows.
- Form value, note, and completion-hint cuts end in `…`, losing their last cell; `fit_spans` also marks a cut at a span boundary.
- Stored snippets (also `search --json`) cap at 160 cells with marks and favor the text after the match; the quick select's snippet fits its row with a blank cell before the frame.
- Spaces at a wrap break keep their span style (invisible; style-only frame diffs).

**Consistency wins**
- CJK, emoji, and accented text cut and pad by cells: titles at 48 cells, VS16 emoji no longer overflow, badges keep their width, forms fit, file listings and breadcrumbs align.
- One `…` rule; a value 1-2 cells over the explorer budget now offers `o`.
- Key hints wrap between hints everywhere; no row starts with `·`.
- Scrollbars, the worktree menu, and the left-pane notice match the rows drawn; wrapped numbered tool lines no longer run past the edge.
- One snippet fit and one painter (looks unchanged); an exactly 17-character find query shows whole.
<!-- SECTION:FINAL_SUMMARY:END -->
