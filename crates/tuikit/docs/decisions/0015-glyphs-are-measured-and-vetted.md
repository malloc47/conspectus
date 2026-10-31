# 0015: Every glyph is one measured cell, in a table the host extends

| | |
| --- | --- |
| Status | Accepted |
| Page | [controls/glyphs.md](../controls/glyphs.md) |

## Decision

- Every non-ASCII character the framework draws has been measured to take
  one terminal cell in common monospace fonts, and is recorded with a note
  in the vetted table. Printable ASCII is always vetted.
- The defaults are one-cell geometric and technical symbols: no emoji, no
  glyph from a patched icon font, nothing that needs a variation selector. A
  host may let its users override its own glyphs.
- `glyph_risk` refuses a glyph whose width is not one cell, that is more
  than one character, that holds a joiner or a variation selector, that sits
  in an emoji block, or that is on a deny-list of text characters terminals
  draw as color emoji. The collapsed disclosure arrow `▶` is the one
  exemption, kept on purpose.
- The table is an owned value, `VettedGlyphs`: it starts from the
  framework's entries, and a host registers its own with a note. A
  duplicate, an empty note, or a risky glyph is refused and changes nothing.
  There is no global registry.
- `glyphs::drawn_glyphs` is a pure scanner (the non-ASCII characters in
  code, comments and test items skipped); a host walks its trees and the
  framework's with it against the union table. `scripts/glyph-widths.py`
  measures glyphs in common fonts.
- Meaning never rests on color alone: things that differ have different
  shapes (a checkbox `[x]` and a one-of-N `(•)`, a filled and a hollow
  mark).

## Rationale

A glyph the font lacks is drawn from a fallback font and spills over its
neighbor, breaking every width budget. Emoji widths differ by terminal and
font, and patched icon fonts are not on every machine; degrading silently
would confuse users who do not know their font is the cause. Distinct shapes
keep the interface readable without color and for color-blind users.

## Consequences

- Adding a glyph is a measurement and a table entry with a note
  ([contributing.md](../contributing.md#the-glyph-guard)).
- The Braille spinner frames are vetted with a note that none of the
  measured fonts carries them, so terminals draw them from a fallback font,
  slightly wide: an accepted preference.

## Alternatives considered

- **A process-wide registry** that hosts add to at startup. Smaller call
  sites, but it reads hidden state, makes two tests in one process share a
  table, and breaks the rule that a control gets what it needs through its
  parameters.
- **The scanner as a test helper only.** Every host would write its own
  lexer.
- **Icon-font glyphs or emoji by default.** Widths and availability vary;
  frames and tests would break on other machines.
- **ASCII only.** A posture of its own that the rest of the interface does
  not take.
- **Detecting the font at run time.** Unreliable.
- **Replacing `▶` with a look-alike that cannot take emoji presentation.**
  Removes the exemption, but changes what users see, which is their call.

## Origin

- host ADR 0073, "A glyph and a color for each kind of thing shown":
  decisions 1 and 7 (one-cell geometric defaults with opt-in overrides, and
  legibility without color), its rejected icon-font, emoji, ASCII-only, and
  detection alternatives, and its amendment that every drawn glyph is
  measured and recorded.
- host ADR 0154, "Shared glyph marks, the vetted table, and the source
  scan": decisions 1, 2, 4, 6, and 7.
