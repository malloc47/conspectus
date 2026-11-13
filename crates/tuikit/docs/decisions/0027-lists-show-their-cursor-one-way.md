# 0027: Lists show their cursor one way and step by their kind

| | |
| --- | --- |
| Status | Accepted |
| Page | [controls/selection.md](../controls/selection.md) |

## Decision

- **The marker.** Every row of a list starts with a two-cell gutter: the
  pointer mark and a space on the cursor row, focused or not, and two spaces
  on every other row. No text moves when the cursor does, and the cursor row
  can be found with no color and no reverse video. The marker takes the
  row's look, not the accent, which is the focus cue's.
- **The highlight.** The cursor row takes the theme's selection pair on
  every span, gutter included: `selection_active` while the list takes keys,
  `selection_inactive` while it does not (house rule 10). A filled span (one
  with a background or drawn reversed: a chip, a field's fill, a text
  cursor) keeps its look; the row's own colors and muted text stay under the
  highlight. The highlight covers the row's spans and is not padded to the
  list's width.
- **The ends.** `Ends::Wrap` for short choice lists (menus, pickers,
  checklists, palettes, a form's ring); `Ends::Clamp` for scrollable content
  (trees, property rows, pagers, logs). A value that cycles in place has no
  cursor row and keeps wrapping.
- `selection::row` draws a row this way; ghost and pending looks are in the
  same module.

## Rationale

House rules 1, 2, and 10. The host had four selection looks. Reverse video
alone vanishes when colors are stripped, an accent marker reads as the focus
cue, and a highlight that hid a chip's fill made the chip unreadable.
Wrapping suits a short list a user cycles through; clamping suits content a
user reads to its end.

## Consequences

- No control types a cursor glyph or reverses a row by hand; the raw-style
  guard and the conformance suite check it.

## Alternatives considered

- **An accent-colored marker.** An accent cell inside a highlighted row
  reads as a block of color and as the focus cue.
- **A highlight padded to the list's width (a bar).** Conventional, but it
  changes every documented frame of the host's main list; left as a seam
  on the page.
- **Dropping the muted look on the cursor row.** Keeping it keeps the row's
  hierarchy legible.
- **Logs that wrap.** House rule 2 says logs clamp.
- **A form highlighting only its label.** A fifth look; the field's fill
  keeps its look inside the highlight anyway.

## Origin

- host ADR 0159, "Lists show their cursor one way and step by their kind":
  decisions 1 to 5.
