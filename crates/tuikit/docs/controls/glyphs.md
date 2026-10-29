# `glyphs`

| | |
| --- | --- |
| Module | `tuikit::glyphs` |
| Layer | 1 (foundation) |
| Type parameters | none |
| Cargo feature | none |
| Outcomes | none: marks are constants; [`VettedGlyphs::register`](#vetting-a-glyph) reports a `RegisterError`, and the scanner returns the characters it found |

## Purpose

`glyphs` makes sure a character a program draws occupies the one terminal
cell the layout gave it. A terminal draws a glyph its monospace font lacks
from a fallback font, and that glyph can spill over its neighbor, so a
selection highlight cuts it in half. The module has four parts:

- **Marks** (`glyphs::marks`): the shared glyphs, each defined once: the
  list cursor, a "go back" arrow, the two disclosure arrows, the check and
  failure marks, a warning mark, a bullet, the ellipsis, the no-value dash,
  one cell of a rule, the separator dot, and the eight frames of a spinner. A control draws a
  mark by constant, so two surfaces cannot drift to look-alike codepoints.
- **The vetted table** (`VettedGlyphs`): the glyphs that have been measured,
  each with a note on how. The framework's own glyphs are in it (the
  marks, a few punctuation and arrow glyphs, the box drawing of frames
  and of a rendered table's borders, and the half and quadrant blocks of
  a toast's border); a host registers
  the glyphs of its own vocabulary on top.
- **The checks** (`cell_width`, `glyph_risk`): a glyph is registered only if
  it is exactly one cell and is not one terminals commonly draw as a wide
  emoji or need a joiner or selector for.
- **The scanner** (`drawn_glyphs`, `VettedGlyphs::unvetted`): reads Rust
  source and reports the non-ASCII characters it draws, outside comments and
  test items, so a host test can fail on a glyph nobody measured.

The host keeps what its screens mean: which glyph stands for which kind of
record or state, the configuration that lets an operator override one, and
the adapter that registers them. The framework knows no such glyph.

## Usage example

```rust
use tuikit::glyphs::{VettedGlyphs, cell_width, marks};

// A control draws a mark by constant, and a layout can count on one cell.
let row = format!("{} item", marks::POINTER);
let arrow = if true { marks::EXPANDED } else { marks::COLLAPSED };
assert_eq!(cell_width(arrow), 1);
assert!(row.starts_with(marks::POINTER));

// The host's adapter registers its own glyphs on top of the framework's.
let mut vetted = VettedGlyphs::default();
vetted
    .register("◆", "Geometric Shapes; DejaVu Sans Mono 1.00; ambiguous")
    .expect("a one-cell glyph with a note");

// The host's source test walks its trees and fails on a glyph nobody
// measured. `source` is one file's text; the test decides which files count.
let source = "const A: &str = \"◆\";\nconst B: &str = \"☃\";\n";
for drawn in vetted.unvetted(source) {
    eprintln!("line {}: `{}` is not vetted", drawn.line, drawn.glyph);
}
```

### The marks

| Constant | Glyph | Means |
| --- | --- | --- |
| `POINTER` | `►` | the list cursor, and the focus marker in a pane title |
| `BACK` | `◄` | "go back": the mark between a title and the path that led to it |
| `EXPANDED` | `▼` | an expanded row or group |
| `COLLAPSED` | `▶` | a collapsed row or group (an operator-preference exception, below) |
| `OK` | `√` | a check that passed or a value that checked out |
| `FAIL` | `×` | a check that failed |
| `WARNING` | `▲` | something that needs attention but is not an error |
| `BULLET` | `•` | an unchecked item in a list of choices; between parentheses, the chosen option of a one-of-N row |
| `ELLIPSIS` | `…` | text cut to fit |
| `NO_VALUE` | `—` | a value that is not known |
| `RULE` | `─` | one cell of a horizontal rule |
| `DERIVED` | `└` | the corner that leads in a line derived from the rows above it (a form's command line), so it reads as output |
| `SEPARATOR` | `·` | the separator between the parts of one item (a name and its state in a chip) and between short items in a line |
| `ARROW_UP`, `ARROW_DOWN`, `ARROW_LEFT`, `ARROW_RIGHT` | `↑ ↓ ← →` | the arrow keys, as key labels name them (see [keys.md](keys.md)) |
| `SPINNER` | `⠋ ⠙ ⠹ ⠸ ⠼ ⠴ ⠦ ⠧` | the frames of a busy spinner (an operator-preference exception, below) |

`marks::ALL` lists every mark once, spinner frames included, and a test keeps
the list and the module's source in step. `marks::SPACED_SEPARATOR` is
`SEPARATOR` with a space on each side (` · `), what joins the hints of a
footer; it is three cells, so it is not one of the marks. `marks::TIMES`
is `FAIL`'s character read as the multiplication sign in a count of
repeats (`(×3)`), so it is not a mark of its own either. The boxes a form
draws before an option are three cells too, built from vetted glyphs, and
not marks of their own: `CHECKED` (`[x]`) and `UNCHECKED` (`[ ]`) for a
checkbox, any number of which can be on, and `CHOSEN` (`(•)`, `BULLET`
between parentheses) and `NOT_CHOSEN` (`( )`) for a one-of-N option. The
shapes differ (brackets and parentheses), so a one-of-N row never looks like
a row of checkboxes, with or without color. The pointer (`►`, U+25BA) and the
collapsed arrow (`▶`, U+25B6) are look-alike codepoints on purpose: the
pointer is not emoji-capable, and the collapsed arrow is the preference
below. Do not substitute one for the other.

### Vetting a glyph

Measure the glyph, then register it with a note that says how it was
checked:

1. Run `scripts/glyph-widths.py GLYPH...` from this crate. It reads the
   fonts' tables directly and prints each glyph's advance as a fraction of
   one cell in DejaVu Sans Mono, Liberation Mono, Andale Mono, and Courier
   New. Prefer a glyph DejaVu Sans Mono carries at exactly 1.00. A
   `MISSING` column means that font falls back to another one.
2. `VettedGlyphs::register(glyph, note)` refuses a glyph that is not exactly
   one cell or one character, needs a joiner or variation selector, is in
   the emoji block, or is on the deny-list of text characters terminals
   commonly draw as color emoji. It also refuses a duplicate and an empty
   note. A failed call changes nothing.
3. Printable ASCII is always vetted; do not register it.

`glyph_risk` and `cell_width` are public so a host can reuse the checks, for
example to validate a glyph an operator names in a configuration file.

### Scanning source

`drawn_glyphs(source)` lexes Rust text just far enough to skip what is
prose: line and doc comments, nested block comments, and every item behind
`#[cfg(test)]`. It understands string literals with escapes, raw and byte
strings, and character literals versus lifetimes. It returns each non-ASCII
character left, with its line. `VettedGlyphs::unvetted` keeps the ones the
table does not hold.

The scanner is pure: it reads no file. The host's test walks its trees (the
workspace's shared source-tree helper does that for the host) and decides
which files are test code. The framework's own guard
(`tests/glyph_literals.rs`) scans its `src/` the same way and fails on:

- any non-ASCII character in code outside the `glyphs` module, vetted or
  not: use a mark, or add the glyph to the module;
- any character in the `glyphs` module that the framework's table does not
  hold.

### The two operator-preference exceptions

Two glyphs are kept although they carry a known risk. Both say so in their
table notes.

- **`▶` (`marks::COLLAPSED`, U+25B6).** The collapsed-disclosure arrow. A
  terminal with an emoji font may draw this codepoint wide. It is the one
  glyph `glyph_risk` exempts from the emoji deny-list. Moving to a safe
  look-alike is a one-constant change, but it is an operator decision.
- **The Braille spinner (`marks::SPINNER`).** None of the four measured
  monospace fonts carries these codepoints, so the terminal falls back to a
  proportional font that draws them 1.22 cells wide. The checks cannot see
  that risk, so the table note records it. The frames are kept because the
  operator likes how they look.

## Key table

None: the module handles no keys.

## Theme tokens

None: the module draws nothing and reads no theme. Callers style a mark from
the theme where they place it, for example a disclosure arrow with
`disclosure` and a warning mark with `warning`.

## House rules followed

| # | Rule | How this module follows it |
| --- | --- | --- |
| 1 | List cursor and highlight | Defines `marks::POINTER`, the marker every list draws in its 2-cell gutter |
| 14 | Width | Every glyph in the table is exactly one cell, checked by `cell_width` when it is registered; `marks::ELLIPSIS` is the cut-end mark |

Not applicable: 2 to 13 and 15 to 18 govern how controls move, confirm,
close, page, style, edit, and label; the module only supplies characters and
the checks on them.

## Seams left open

- **An ASCII fallback.** Marks are constants, so a host that wants plain
  ASCII on a limited terminal maps each constant itself, or a later mark
  lookup can take a table. Nothing detects the terminal today, and a host
  that overrides one glyph (an icon in a configuration file) validates it
  with `cell_width` and `glyph_risk`.
- **Another source language.** `drawn_glyphs` lexes Rust. A host whose
  glyphs live in data files can call `glyph_risk` on each glyph directly.
- **More marks.** A mark is a constant plus a line in `marks::ALL` and a
  table entry in `src/glyphs/vetted.rs`; the key arrows are vetted already
  and have no constant yet.
