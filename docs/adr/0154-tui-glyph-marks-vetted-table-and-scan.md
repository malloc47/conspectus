# ADR 0154: Shared Glyph Marks, The Vetted Table, And The Source Scan

## Status

Accepted. Amends ADR 0073 (node-kind visual identity): the table of
measured glyphs that ADR names, `icons::VETTED`, moves into the framework
crate's `glyphs` module (ADR 0150) and the host adds its own glyphs to it.
Applies house rule 14 of ADR 0151 to glyph widths. Amended by
[ADR 0168](0168-tui-forms-one-ring-submit-from-any-field-fixed-footer.md):
the marks gain the derived-line corner `└`, and the module names the
three-cell option boxes built from vetted glyphs (`[x]`, `[ ]`, `(•)`,
`( )`). Amended by
[ADR 0179](0179-tui-log-lists-read-the-hosts-log-over-an-embedded-pager.md):
the module names `TIMES`, the failure mark's character read as the
multiplication sign in a repeat count (`(×3)`), not a mark of its own.
Amended by [ADR 0181](0181-tui-rich-text-is-a-framework-feature-and-the-preview-scrolls-in-a-region.md): a rendered table's border glyphs are in the
framework's vetted table, and the host no longer registers them.

## Context

`src/tui/icons.rs` mixed two things. The generic half was a table of
glyphs measured to fit one terminal cell, the width and emoji checks on it,
and a test that failed when a source file drew a glyph nobody had measured.
The Conspectus half was the node-kind glyphs, the row-state glyphs, and the
host's overrides. Meanwhile the table was not the single source for the
glyphs it listed:

- `marks::FAIL`, `POINTER`, and `BACK` had no users, while `▲` was typed at
  10 sites, `►` at 4, `√` at 2, `×` at 2, and `◄`, `▼`, and `▶` were typed
  where constants existed. `marks::NO_VALUE` had 4 users against 33 typed
  `—` placeholders.
- `►` (U+25BA, the list cursor and pane focus marker) and `▶` (U+25B6, the
  collapsed-tree arrow) are look-alike codepoints, and only the second can
  take emoji presentation.
- The scan that fails on an unmeasured glyph covered `src/tui` only, so the
  transcript viewer's table borders and result arrow, and the graph
  export's star, were never measured. It also read lines that start with
  `//` as comments and every other line as code, so a trailing comment or
  an inline test module counted.
- The chrome drew nine glyphs the reference document and its test did not
  cover.

A framework that other programs reuse needs the marks, the checks, and the
scanner without the host's vocabulary, and needs to let a host add its own
glyphs to the same table.

## Decision

1. **`tuikit::glyphs` owns the generic half.** It defines the marks (list
   cursor, back, the two disclosure arrows, ok, fail, warning, bullet,
   ellipsis, no-value, one rule cell, and the eight spinner frames) as
   constants in `glyphs::marks`, with `marks::ALL` listing each once. It
   also owns `cell_width`, `glyph_risk` (width other than one cell, more
   than one character, joiner or variation selector, emoji block, and a
   deny-list of text characters terminals draw as color emoji), and the
   framework's own table entries (the marks plus `·`, `›`, the four key
   arrows, and the box-drawing family).

2. **The table is an owned value with registration.** `VettedGlyphs`
   starts from the framework's entries (`Default`), and a host calls
   `register(glyph, note)` for its own. A registration is refused when the
   glyph is a duplicate, the note is empty, or `glyph_risk` rejects it, and
   a refusal changes nothing. Printable ASCII is always vetted. There is no
   global registry: a control reads no hidden state, and two tables never
   share any.

3. **Conspectus registers its glyphs from an adapter.**
   `tui::icons::register_glyphs` adds the node-kind glyphs (an exhaustive
   match gives each kind a note, so a new kind cannot ship without one), the
   state glyphs, the untracked-PR mark (moved from `marks` to `state`
   because it is a PR state), and the glyphs the sources draw that belong to
   no row vocabulary: `↳` and the table borders in the transcript viewer,
   and `★` in the graph export. The node-kind glyphs, state glyphs,
   `IconOverrides`, and the `[tui.theme.icons]` loader stay in Conspectus;
   the loader measures overrides with `cell_width`.

4. **The scanner is a pure function; the host walks the trees.**
   `glyphs::drawn_glyphs(source)` returns each non-ASCII character that
   Rust code draws (string and character literals) with its line, skipping
   comments of every kind and every `#[cfg(test)]` item.
   `VettedGlyphs::unvetted(source)` keeps the ones the table does not hold.
   The host's consistency test (`consistency_tests/glyphs.rs`) walks the
   whole host `src` and the framework's `src` through the shared
   source-tree helper (`tests/support/source_tree.rs`) and fails on an
   unvetted character. It now covers all of `src` rather than `src/tui`, so
   the viewer's and the export's glyphs are measured too; `é` and `§` left
   the table, since they were only ever drawn in test code, which the scan
   no longer reads.

5. **Two guards keep the marks single-sourced.** The framework's own guard
   (`crates/tuikit/tests/glyph_literals.rs`) fails on any non-ASCII
   character in framework code outside the `glyphs` module, and on a
   character inside it that the framework's table does not hold; it has no
   exception list. The host's guard (`consistency_tests/marks.rs`) fails
   when drawing code under `src/tui` or `src/viewer` types the character of
   `►`, `◄`, `▼`, `▶`, `√`, `×`, `▲`, `•`, `─`, or a spinner frame, or a
   string that is exactly `—` or `…`. Prose such as `Loading…` and
   `not found — try again` is copy and is allowed. Two reviewed exceptions
   exist: the multiplication sign in a repeat count (`(×3)`) and the table
   reader's border set.

6. **The look-alike codepoints stay two, and the two operator-preference
   exceptions stay.** The pointer is `►` and the collapsed arrow is `▶`;
   which glyph each site shows is unchanged by this decision (house rule 1
   will unify the row cursor marker separately). `▶` is the one glyph
   `glyph_risk` exempts from the emoji deny-list, and the Braille spinner
   frames are vetted with a note that none of the measured monospace fonts
   carries them, so the terminal falls back to a proportional font that
   draws them 1.22 cells wide. Both are documented in the module, its
   control page, and the table notes.

7. **The measuring script moves with the module.**
   `crates/tuikit/scripts/glyph-widths.py` takes glyphs or `--source` with
   directories, and works on any tree.

## Consequences

- The default frames are byte-identical: every site keeps the glyph it
  showed. A glyph's meaning can no longer drift between uses, and the
  chrome's glyphs (focus marker, breadcrumb, the Other header, the
  ambiguity chips) are covered by the same width and vetting checks as the
  left panel's.
- `docs/tui-columns.md` still lists the left panel's glyphs only; the
  ellipsis and the rule cell are marks but are described in prose there.
- A new host glyph needs a measurement and a registration note, and a new
  node kind needs a note (the match is exhaustive).
- The scanner is a lexer, not a parser: it assumes `rustfmt`-formatted
  source and treats `#[cfg(test)]` as the only test gate. It is a tripwire
  for a forgotten glyph.
- `·` (about 139 sites) and the key arrows are vetted but have no
  constants yet.

## Alternatives Considered

- **A process-wide registry** that hosts add to at startup. Smaller call
  sites, but it reads hidden state, makes two tests in one process share a
  table, and breaks the rule that a control gets what it needs through its
  parameters.
- **The scanner stays a test helper.** Nothing in the crate would then
  offer the check to a host, and every host would re-implement the lexing.
  The framework's test kit is meant to hold such helpers (ADR 0150); the
  library function is its first form.
- **Keep scanning `src/tui` only.** Cheaper, but leaves the viewer's and
  the export's glyphs unmeasured, and the scan would pass vacuously as code
  moves between crates. Scanning both crates' `src` costs 18 registered
  entries (the viewer's table borders and arrow, and the export's star).
- **Fail the scan on a trailing comment as before.** A line is not a
  lexical unit; a comment after code or a test module in the middle of a
  file made the old scan flag characters that are never drawn.
- **Replace every `…` and `·`.** `…` is both a mark and ordinary punctuation
  in copy, and `·` is a separator with no semantics beyond layout. The
  guard refuses a bare `—` or `…` string and leaves prose alone; the text
  helpers use the constant, and the rest is copy.
- **Move `▶` to a non-emoji-capable look-alike.** Removes the exception, but
  it changes what the operator sees and is the operator's call.

## Open Questions Answered

- *Is the vetting table mandatory for adopters?* The scan is: a host that
  runs it against its trees gets the check, and the framework runs it
  against its own. A host that does not call it loses nothing at runtime.
- *Does the framework offer an ASCII fallback?* No. Marks are constants, so a
  host that wants one maps them itself; the control page lists it as a seam.
- *Which exceptions are the operator's?* `▶` as the collapsed arrow and the
  Braille spinner. Both are kept and recorded in the table notes.
