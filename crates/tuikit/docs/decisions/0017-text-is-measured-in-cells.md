# 0017: Text is measured, cut, wrapped, and painted in terminal cells by one module

| | |
| --- | --- |
| Status | Accepted |
| Page | [controls/text.md](../controls/text.md) |

## Decision

`tuikit::text` owns measuring, cutting, padding, wrapping, snippets,
finding, and painting.

- **Graphemes.** Every operation moves in whole graphemes, segmented as
  ratatui segments them and measured with `unicode-width`, so a cut never
  strands an accent or half an emoji and a measured width is the width
  drawn. Control characters are kept one at a time, so a cut text stays a
  prefix of its source.
- **Cutting.** A cut that loses text ends in the ellipsis mark inside the
  budget. A two-cell character that would straddle the budget goes whole;
  spaces alone past the budget go without a mark; a cut is idempotent.
  Paths and identifiers are cut in the middle. A cut at a span boundary is
  marked too, in the style of the span the mark stands for.
- **Wrapping.** One row breaker for plain and styled text: break at the last
  space that fits; a word that ends at the edge stays; a continuation row
  never starts with a space; a word wider than a row breaks between
  graphemes; a grapheme wider than the row draws as the mark; hanging
  indents; key hints break only between hints.
- **A surface that counts its rows draws the rows it counted.** Nothing lets
  the paragraph widget wrap and then estimates.
- **One snippet fit.** `fit_snippet` keeps the match, gives the text after
  it the larger share of the spare cells, cuts at word edges (and beside
  wide characters), keeps part of a token such as a path or a long
  identifier rather than none, and marks each cut side.
- **One painter.** `paint` restyles byte ranges, snapped to grapheme edges,
  with a function of the style under them, so each surface keeps its look;
  `paint_matches` paints a query's occurrences after a gutter.
- `strip_ansi` lives here, because production code strips captured text.

## Rationale

The host had thirteen truncation helpers and six word-wrap helpers that
disagreed at the edges, and scrollbars placed by estimated row counts. One
module measured in cells is house rule 14.

## Consequences

- A host's other output (a command-line table) can share the measure and
  the cut; allocating column widths is not text but layout (record
  [0060](0060-column-schemas.md)).
- Fitting the pieces of one line is record
  [0018](0018-pieces-of-a-line-give-way-by-one-rule.md).

## Alternatives considered

- **`unicode-segmentation` as a direct dependency.** Exact either way, but a
  new dependency needs its own record, and ratatui already exposes its
  segmentation.
- **Approximating graphemes** (a character and the zero-width ones after
  it). Misses emoji modifier sequences, flags, and joiner sequences, and
  could disagree with how ratatui draws.
- **Keeping the paragraph widget's wrap and fixing the estimators.** The
  estimate would still be a second implementation of the wrap; drawing the
  rows the module produced makes the count exact by construction.
- **Keeping a trailing-space count at an exact fit.** It wasted a cell and
  matched no terminal or pager.
- **A centered snippet.** Two rules would remain, and a crop that favors the
  text after the match would run out of that text first.

## Origin

- host ADR 0155, "Text is measured, cut, and wrapped in terminal cells by
  one module": decisions 1 to 7, and its later refinement of the snippet's
  word edges and tokens.
- host ADR 0180, "The document viewer is a pager content": decision 6 (one
  painter for a query's matches after a gutter).
- host ADR 0164, "The test kit drives frames, keeps colors in goldens, and
  holds every modal to the conventions": decision 5 (one `strip_ansi`, in
  `text`).
