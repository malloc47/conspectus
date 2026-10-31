# `text`

| | |
| --- | --- |
| Module | `tuikit::text` |
| Layer | 1 (foundation) |
| Type parameters | none |
| Cargo feature | none |
| Outcomes | none: every function is pure and returns the text, rows, or spans it built |

## Purpose

`text` measures, cuts, pads, wraps, and marks strings in terminal cells, so
a column lines up and a row fits whatever the text holds. A terminal gives a
CJK character or an emoji two cells, a combining accent none, and an emoji
written with a variation selector the two cells of the picture it draws.
Code that counts `char`s cuts a row a cell late or early, and the columns
after it drift. Every function here measures cells and moves in whole
graphemes (a letter and its accent, an emoji and its selector), so no cut,
wrap, or highlight separates them.

The module replaces the scattered helpers a program grows: several
truncators (some counting characters), padding helpers, word wrappers that
disagree about where a row breaks, two ways of cropping a snippet around a
match, a match painter per surface, and a fit routine per line whose pieces
give way (a path, a strip of names, a status line). The host keeps what its
text means:
which column holds which value, how wide each column is, domain-specific
compaction (a path shortened to its last segment, an identifier kept to a
fixed head and tail), and which style a match takes.

## Usage example

```rust
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use tuikit::text::{
    find_all_ci, fit, fit_snippet, fit_spans, paint, spans_width, truncate_middle, width,
    wrap, wrap_at, wrap_line,
};

// Columns: a fixed-width label, then a path cut in the middle.
let label = fit("東京オフィス", 8);
assert_eq!(width(&label), 8);
let path = truncate_middle("/srv/data/archive/2026/report.txt", 20);
assert_eq!(width(&path), 20);

// A styled row cut to the pane, ending in the ellipsis mark.
let row = vec![Span::raw(label), Span::raw("  "), Span::raw(path)];
let row = fit_spans(row, 24);
assert!(spans_width(&row) <= 24);

// Prose and key hints wrapped to a modal's width.
let body = wrap("Removes every entry that is already merged.", 20);
assert!(body.iter().all(|row| width(row) <= 20));
let hints = wrap_at("Enter open · Space mark · Esc close", " · ", 24);
assert_eq!(hints, ["Enter open · Space mark", "Esc close"]);

// A styled line with a hanging indent, for a body that needs its row count.
let line = Line::from(vec![Span::raw("  note: "), Span::raw("the rest of a long sentence")]);
let rows = wrap_line(&line, 18, 2);
assert!(rows.iter().all(|row| row.width() <= 18));

// A search result: crop around the match, then paint it.
let text = "the cache is rebuilt from the index whenever the schema changes";
let at = text.find("index").unwrap();
let snippet = fit_snippet(text, at..at + 5, 30, &[]);
let bold = Style::default().add_modifier(Modifier::BOLD);
let spans = paint(&[Span::raw(snippet.text.as_str())], snippet.highlight.clone(), |_| bold);
assert_eq!(spans.iter().filter(|span| span.style == bold).count(), 1);

// Every occurrence, for a pager's find.
let painted = paint(&[Span::raw(text)], find_all_ci(text, "THE"), |base| base.patch(bold));
assert_eq!(painted.iter().filter(|span| span.style == bold).count(), 3);
```

### The functions

| Function | Does | Notes |
| --- | --- | --- |
| `width`, `spans_width` | cells a string or a span list takes | the count ratatui lays out by |
| `truncate_end` | cut to a budget, `…` in the last cell | for labels; spaces alone past the budget go without a mark |
| `truncate_middle` | cut the middle out, `…` between the ends | for paths and identifiers (house rule 14) |
| `clip`, `clip_start` | hard cut, no mark, keeping the start or the end | for a column whose content is a prefix by nature, or an identifier's distinctive tail |
| `pad`, `pad_start` | fill to a width with spaces after or before | never cut |
| `fit` | cut then pad: exactly the width | |
| `fit_spans` | cut a styled row, `…` in the style of the cut span | spaces alone past the budget go without a mark |
| `wrap` | word wrap | rules below |
| `wrap_chars` | wrap anywhere between graphemes, as a terminal does | |
| `wrap_at` | wrap only at a separator, dropping it at a break | for key hints; an item wider than a row is word-wrapped |
| `wrap_line`, `wrap_line_chars` | the styled forms of `wrap` and `wrap_chars` | `wrap_line` takes a hanging indent for continuation rows |
| `fit_snippet` | crop an excerpt around a match to a budget | favors the text after the match; drops the characters you name first |
| `elide` | keep the pieces of a line that fit by rank, mark what gave way | rules below; `Piece`, `MarkAt`, `Elided`, `Item` |
| `elide_filled` | `elide`, then fill the room back with pieces that still fit | for a line whose pieces stand alone, such as a status line |
| `find_ci`, `find_all_ci` | find matches, ASCII letters without case | offsets stay valid for highlighting |
| `paint` | restyle byte ranges of a span list | the one match painter |
| `paint_matches` | patch every occurrence of a query in a row, after a gutter it skips | `find_all_ci` and `paint` together, for a reader's rows |
| `strip_ansi` | drop ANSI escape sequences from text | for comparing captured or written output as plain text; the test kit re-exports it |

### Cutting rules

- Every cut that loses text ends in `glyphs::marks::ELLIPSIS`, inside the
  budget: the result never exceeds the width. A budget of 1 leaves only the
  mark; a budget of 0 leaves nothing.
- A two-cell character that would straddle the budget is dropped whole, so a
  cut can be a cell short; `fit` and `pad` fill that cell.
- When only spaces lie past the budget, they are dropped with no mark: no
  text was lost.
- A cut is idempotent: cutting a cut text to the same budget changes nothing.

### Wrapping rules

Every word wrapper lays out rows the same way, styled or not:

- A row breaks at the last space that fits. A word that ends exactly at the
  edge stays on its row.
- Spaces before a break stay on their row when they fit; spaces that would
  run past the edge are dropped, and a continuation row never starts with
  one. The first row keeps its leading indent.
- A word wider than a row is broken between graphemes, and the rest of it
  carries on as ordinary text.
- No grapheme is split and no row is wider than the width. A character wider
  than the whole row (a two-cell character in a row of one) cannot be drawn,
  so the ellipsis mark stands in for it.
- A line break ends a row, an empty text is one empty row, and a width of 0
  is treated as 1.

These are the rules ratatui's paragraph wrapper follows when it keeps
whitespace (`Wrap { trim: false }`), so pre-wrapped text reads like a wrapped
paragraph. The difference is that the row count is known before drawing: a
surface that needs it (a scroll range, a scrollbar's length, a height
budget) wraps with these functions and draws the rows unwrapped, so the
count and the drawing cannot disagree.

### Snippets

`fit_snippet(text, highlight, budget, drop)` removes the characters in
`drop` (for example Markdown code ticks, `&['`']`; or `&[]`), moves the
highlight with the text, and, when the text is wider than the budget, keeps
the match and spends the rest on context: two thirds after the match, the
rest before it. Each cut side ends in the mark, a cut that would split a
word moves to the word's edge toward the match, and a match wider than the
budget shows as much of its start as fits. The highlight is widened to
whole graphemes.

A word ends at a blank and beside a wide character: Chinese and Japanese
text puts no spaces between its words, and a line may break between any
two of its characters, so a cut may too.

When the only edge on a side is the one beside the match, past the blanks
there, moving to it leaves that side empty. For a word that is the better
result: a cut word (`…ine wombat ten…`) reads worse than none (`…wombat
ten…`), so prose, the common case, is only ever cut at whole words. A
token keeps the part that fits instead, cut between its graphemes
(`…oml wombat https:/…`). A stretch of text between blanks is a token
when it

- holds a wide character: unspaced Chinese or Japanese, an emoji;
- is wider than 24 cells, which no word in common use reaches with its
  punctuation (`"internationalization",` is 23) and a commit hash (40) or
  a dashed id (36) passes; or
- has a `/`, `\`, `_`, `.`, or `:` with a character before it and two
  letters after it: a path, a file name, an address, a code name
  (`docs/notes.md`, `example.com`, `snake_case`, `std::fmt`).
  Abbreviations (`e.g.`, `U.S.`), figures (`3.14`, `10:30`, `24/7`), and
  closing punctuation do not count; `and/or` does.

A side with room for one grapheme past the blanks beside the match
therefore comes back empty only when the stretch there is a word too wide
for its room. A cut with no edge between it and the match (a match inside
a word) stays between graphemes. The fits of a generated English corpus at
every budget are held to a golden (`src/text/goldens/prose_snippets.golden`),
so a change to how prose is cut shows there line by line.

### Giving way

A line of pieces that can go one by one (the hops of a breadcrumb, the
names of a tab strip, the hints of a status line) fits its room by one
rule, `elide(pieces, separator, mark, room, at)`. The caller measures each
piece in cells and gives it a rank (`Piece::ranked`) or none
(`Piece::unranked`); `separator` is the cells between two items and `mark`
the cells of one mark, with any spacing it carries. A piece may have a gap
of its own (`.gap(cells)`), taken in place of the separator before it when
an item precedes it, for a line whose pieces are not all joined alike; a
mark in a gap takes the separator.

- Start from every piece. While the line is wider than the room and a
  ranked piece is kept, the lowest-ranked one gives way; of two with one
  rank, the later one.
- Each run of neighboring pieces that gave way leaves one mark: in its gap,
  as an item between separators (`MarkAt::Gap`, a breadcrumb's
  `a › … › e`), or once at the end of the line with no separator before it
  (`MarkAt::End`, a strip's `one · two …`). A mark's cells are counted
  before the line counts as fitting.
- When only unranked pieces are left and they still do not fit, the marks
  go; if the pieces are still too wide, the last one is cut to the cells
  the others leave (`Elided::cut`), for the caller to cut with
  `truncate_end` or `fit_spans`.
- The line then fits the room, unless more than one piece is unranked and
  those before the last fill it already.

`Elided` holds the kept pieces by index, where the gap marks stand, whether
an end mark follows, and the cut; `items()` lists the line's items in order
(pieces and gap marks, a separator between each two), and `width()` its
cells.

```rust
use tuikit::text::{Item, MarkAt, Piece, elide};

// A path of four steps, the last kept whatever the room, the first
// outlasting the middle, separated by three cells, with a one-cell mark.
let steps = [
    Piece::ranked(5, 4),
    Piece::ranked(5, 1),
    Piece::ranked(5, 2),
    Piece::unranked(5),
];
let line = elide(&steps, 3, 1, 18, MarkAt::Gap);
assert_eq!(line.items(), [Item::Piece(0), Item::Mark, Item::Piece(3)]);
assert_eq!(line.width(&steps, 3, 1), 17);
```

The `columns` module has a narrow-width policy of its own for the cells of
a row, which narrow as well as drop; a line whose pieces only stay or go
uses `elide`.

### Filling the room back

`elide` gives way strictly by rank, so once a wide piece has gone, a
narrower one of lower rank that would fit beside the kept pieces has gone
too. That is right for a line whose pieces read in sequence (a path, a
strip of names), where showing a later piece past a gap would put it
where the mark does not say. A line whose pieces stand alone, each worth
showing whatever else shows (a status line's hints and settings), wants
every cell used. `elide_filled` takes the same arguments, applies `elide`,
and then brings back each piece that gave way, highest rank first, when
the line still fits with it:

- Pieces of one rank come back in order, the earlier first, and once one
  of them does not fit, the later ones of that rank stay out. The pieces
  kept of each rank are therefore always its first ones, as `elide` leaves
  them, so a group given one rank never shows a later member without the
  earlier.
- With every rank different, the result is packing by priority: in order
  of importance, keep each piece that fits beside those kept, then, when
  any was left out, make room for the mark.
- The marks follow what is kept. When `elide` had to drop its marks or
  cut its last piece, there is no room to fill, and nothing comes back.

```rust
use tuikit::text::{MarkAt, Piece, elide, elide_filled};

// A narrow hint of low rank after a wide one of higher rank.
let hints = [Piece::unranked(6), Piece::ranked(20, 5), Piece::ranked(9, 2)];
// 6 + 3 + 9 and the two-cell mark: 20 cells.
assert_eq!(elide(&hints, 3, 2, 24, MarkAt::End).kept, [0]);
assert_eq!(elide_filled(&hints, 3, 2, 24, MarkAt::End).kept, [0, 2]);
```

### Finding and painting

`find_ci` folds ASCII letters only, so a match is exactly as many bytes as
the needle and `offset..offset + needle.len()` is its range. Folding other
letters can change byte lengths, which would put a highlight in the wrong
place. `paint(spans, ranges, restyle)` takes byte ranges into the spans'
text laid end to end (an iterator, an `Option` of one range, or
`find_all_ci`'s output) and gives each painted grapheme `restyle(base)`:
`|base| base.patch(m)` layers the match style `m` on the surrounding look,
`|_| m` replaces it. Range edges move outward to grapheme edges, so a
highlight never strands an accent. `paint_matches(line, skip, query, look)`
is the two together for a row a reader draws: every occurrence of the query
after the row's first `skip` cells (a gutter of line numbers or labels) is
patched with `look`; the pager's `Find::paint` and a host's own highlights
outside a pager draw through it, so they find the same matches.

## Key table

None: the module handles no keys.

## Theme tokens

None: the module reads no theme. Its callers choose the styles: a match is
usually painted with a style built from `match_highlight`, and the mark a
cut adds takes the style of the span it stands in for.

## House rules followed

| # | Rule | How this module follows it |
| --- | --- | --- |
| 14 | Width | Every function measures terminal cells and moves in whole graphemes; every cut ends in `marks::ELLIPSIS`; `truncate_middle` is the cut for paths and identifiers; `elide` takes widths in cells and marks every run of pieces that gave way |

Not applicable: 1 to 13 and 15 to 18 govern how controls look, move, close,
confirm, edit, page, and label. The module only shapes text; the controls
that draw it follow those rules.

## Seams left open

- **East Asian ambiguous widths.** `width` counts ambiguous-width characters
  as one cell, as most terminals outside East Asian locales draw them. A host
  whose users run a CJK terminal would want them as two; a width context
  passed to the functions would carry the choice.
- **Hyphenation and language-aware breaking.** Words break at spaces and,
  when too long, between graphemes. Scripts written without spaces (Chinese,
  Japanese, Thai) wrap between graphemes; a line-breaking table would plug
  in where a row's break is chosen. A snippet's cut has one piece of such
  a table, the edge beside a wide character. Thai, set in narrow
  characters with no spaces between its words, is a word to the cut until
  it runs past 24 cells, so a shorter Thai phrase too wide for its room
  leaves its side empty; a script-aware token rule would keep part of it.
- **Folding beyond ASCII.** `find_ci` could take a folding mode that keeps a
  byte map from folded text back to the source, which a highlight would then
  read.
- **Grapheme cursor motion.** An editor's left and right keys could move by
  the same graphemes the cuts use; today the edit engine has its own rule.
