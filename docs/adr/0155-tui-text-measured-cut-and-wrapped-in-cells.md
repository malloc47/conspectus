# ADR 0155: Text Is Measured, Cut, And Wrapped In Terminal Cells By One Module

## Status

Accepted. Applies house rule 14 of ADR 0151 and extends ADR 0150 with the
framework's `text` module. Amends ADR 0106 (preview wrap modes): the Plain
and Smart wraps break rows by the shared rule below, so a word that ends
exactly at the preview's edge stays on its row. Amends ADR 0076 (scrollbar
choice): a scrolled pane's content length is the row count it draws, not an
estimate. ADR 0147's ASCII-only matcher moves into the framework unchanged.

## Context

The inventory for the framework extraction
(`docs/plans/tui-framework/inventory/`) found the TUI cutting, padding,
wrapping, and highlighting text in many places that disagreed:

- **19 cut and pad implementations**, 9 of them counting `char`s: the
  panels' helpers, a popup `ellipsize`, the forms' `truncate_chars` and
  `fit`, the badge, the explorer's `truncate`, and inline cuts in three
  find chips, the search header, and the process preview. A CJK title of
  30 characters (60 cells) passed a 48-character cap and wrapped; an
  emoji written with a variation selector counted one cell and pushed its
  row a cell past its column. Even the cell-counting helpers measured one
  `char` at a time, which undercounts such emoji.
- **7 word wrappers.** The transcript viewer's (which the widgets
  imported, a layering inversion), the forms' two, the preview's styled
  one, the help page's hanging indents, ratatui's paragraph wrap at 7
  sites, and three `div_ceil` estimators that sized scrollbars and zones
  without wrapping. The custom wrappers counted the space after a word, so
  a word that ended exactly at the edge moved to the next row (ratatui's
  did not); the viewer's left a blank row behind and dropped the blank
  lines between Markdown blocks; the estimators disagreed with what the
  paragraph drew, which the explorer covered with a spare row.
- **2 snippet croppers** (a cell-based one that favored the text after a
  match and cut on word edges, and a character-based one that centered
  it) and **7 match painters**, each restyling a match its own way.
- Key-hint footers wrapped two ways: the forms between hints, everything
  else between words, so `Esc` could end one row and `close` start the
  next.

## Decision

1. **`tuikit::text` owns measuring, cutting, padding, wrapping, snippets,
   finding, and painting.** Its functions: `width`, `spans_width`;
   `truncate_end`, `truncate_middle`, `clip`, `clip_start`, `pad`,
   `pad_start`, `fit`, `fit_spans`; `wrap`, `wrap_chars`, `wrap_at`,
   `wrap_line`, `wrap_line_chars`; `fit_snippet`; `find_ci`,
   `find_all_ci`; and `paint`, the one match painter. The host's
   adapters keep what text means: `compact_path_*` and `compact_mux_*`,
   which column holds what, and which style a match takes.

2. **Graphemes, measured as ratatui draws them.** Every operation moves in
   whole graphemes, segmented by ratatui's own `Span::styled_graphemes`
   (so no dependency is added; `unicode-segmentation` stays a transitive
   one) and measured with `unicode-width`, so a cut never strands an
   accent or half an emoji and a measured width is the width drawn.
   Control characters, which ratatui skips, are kept one at a time so a
   cut text stays a prefix of its source.

3. **Cutting rules.** A cut that loses text ends in `marks::ELLIPSIS`
   inside the budget; budget 1 is only the mark and budget 0 is empty. A
   two-cell character that would straddle the budget goes whole (the cut
   can be a cell short; `fit` pads it). Spaces alone past the budget are
   dropped with no mark. Cuts are idempotent. `truncate_middle` is the cut
   for paths and identifiers; a cell the start cannot use goes to the end.
   `fit_spans` marks a cut that falls at a span boundary too (the old
   helper dropped the rest silently), in the style of the span the mark
   stands for.

4. **Wrapping rules, one row breaker for plain and styled text.** A row
   breaks at the last space that fits; a word that ends at the edge stays;
   spaces before a break stay when they fit and a continuation row never
   starts with one; a word wider than a row breaks between graphemes and
   carries on; a grapheme wider than the whole row is drawn as the
   ellipsis mark; width 0 is 1. These are ratatui's paragraph rules with
   whitespace kept (`Wrap { trim: false }`). `wrap_line` takes a hanging
   indent. Key-hint footers wrap with `wrap_at` on
   `overlay::HINT_SEPARATOR`, breaking only between hints.

5. **A surface that counts its rows draws the rows it counted.** The
   preview pane, the explorer, the left pane's banner and placeholder, the
   worktree menu, and the viewer's empty banner wrap with `tuikit::text`
   and draw the rows unwrapped. The explorer wraps at full width, or one
   column less when it scrolls and a scrollbar takes that column, and no
   longer keeps a spare row. No production code uses `Paragraph::wrap`.

6. **One snippet fit.** `fit_snippet` keeps the match, gives two thirds of
   the spare cells to the text after it, cuts on word edges, marks each cut
   side, and drops the characters it is told to (Markdown code ticks for
   one-line previews). It crops both the stored hit snippets (now capped at
   160 cells, marks included) and every displayed one; the quick select
   fits its snippet to the rest of its row.

7. **One painter.** `paint(spans, ranges, restyle)` restyles byte ranges
   (merged, snapped outward to grapheme edges) with a function of the
   underlying style, so each surface keeps its look: the viewers and the
   pane preview patch the match style over the text, the search rows and
   snippets replace it.

8. **The CLI renderer shares the measure and the cut, not the column
   allocator.** `output::render::display_width` and `truncate_to_width`
   delegate to `text::width` and `text::truncate_end`, and the search
   table pads with `text::pad`. `fit_to_width` stays the renderer's own:
   it allocates column budgets, which is column layout (the framework's
   future `ColumnSchema`), not text. The CLI search's `crop_around`, which
   centers its match, is left for a follow-up (CSP-774) because moving it
   changes what `conspectus search` prints.

9. **Left as they are**, each for a reason: `header::truncate_chip_list`
   caps a list by count, not width; the DOT and HTML exports cut labels by
   characters for formats that are not terminal cells; `format!` padding
   of numbers and fixed ASCII labels counts characters that are cells;
   the forms' text-field window is editing, which `LineEdit` takes over.

## Consequences

- CJK, emoji, and combining text cut, pad, and wrap at the same columns as
  ASCII; property tests over generated text (ASCII, CJK, emoji alone, with
  a presentation selector, and as a joiner sequence, combining accents)
  hold every cut, fit, wrap, and snippet to its budget, and host tests
  render wide text in rows, badges, forms, and the explorer.
- Visible for ASCII text: a cut form value ends in `…` instead of a hard
  cut; a word that ends exactly at a row's edge stays on that row
  (help, viewers, modals, previews); footers break between hints and never
  start a row with `·`; the explorer's spare row is gone (the preview
  gains a row, or the explorer's last row is no longer hidden); the
  worktree menu grows to its wrapped rows; the transcript viewer keeps the
  blank lines between Markdown blocks; the quick select's snippet ends in
  `…` inside its row.
- The widgets import no text helper from `crate::viewer`; the viewer
  imports them from the framework, which CSP-748 builds on.
- A new surface that wraps text it must count uses `wrap_line` and draws
  the result; a reviewer can treat a `Paragraph::wrap` as a smell.

## Alternatives Considered

- **Add `unicode-segmentation` as a direct dependency.** Exact grapheme
  segmentation either way, but ADR 0152 forbids a new dependency without
  its own record, and ratatui already exposes its segmentation.
- **Approximate graphemes (a character plus the zero-width ones after
  it).** No dependency, but it misses emoji modifier sequences, flags, and
  joiner sequences, and could disagree with how ratatui draws.
- **Keep ratatui's paragraph wrap and fix the estimators.** The estimate
  would still be a second implementation of the wrap; drawing the rows the
  framework produced makes the count exact by construction.
- **Keep the old wrappers' trailing-space count.** It matched nothing else
  and wasted a cell at every exact fit; ratatui, terminals, and pagers all
  keep the word.
- **Keep the centered stored snippet.** Two algorithms would remain, and a
  display crop favoring the text after the match would run out of that
  text first.
- **Move `fit_to_width` into `text`.** It decides how wide columns are,
  not how text fits one; it belongs with the column schema.

## Open Questions Answered

- *Does the CLI table renderer share the module?* Its cell measure and cut
  do; its column allocation does not (item 8), and the CLI snippet crop is
  a follow-up.
- *What does a cut of nothing but spaces look like?* It drops the spaces
  and adds no mark.
- *What happens to a two-cell character in a one-cell row?* The ellipsis
  mark stands in for it.
