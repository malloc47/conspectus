# 0049: Forms move on one ring, submit from any field, and keep their keys in view

| | |
| --- | --- |
| Status | Accepted |
| Page | [controls/form.md](../controls/form.md) |

## Decision

- **Fields are data.** A `Form<M>` is an ordered list of fields, each with
  an id, a label, and a kind: text with a check, a path, a choice of named
  options shown as chips (which may take a typed value of its own, marked
  `[custom]`), a toggle, a check group, a one-of-N row, or a read-only line
  in a tone (a note, derived output, a warning). A field may be shown only
  while a condition over the form's values holds; a hidden field has no
  row, no stop, and no check.
- **One ring.** `Tab`, `↓`, and `Ctrl-N` go to the next stop and `S-Tab`,
  `↑`, and `Ctrl-P` to the previous, wrapping. A read-only line is never a
  stop. `Tab` on a path field completes first.
- **`Enter` submits from any field** (house rule 6). Each shown field's
  check runs top to bottom; the first that fails puts its reason on the
  error line and takes the focus. Then the host's `on_submit` (shared
  behind an `Arc`) gets the values and answers `Commit(M)`,
  `Reject(String)`, or `Close`.
- **One error line in the footer slot** (house rule 7), directly above the
  keys, cleared by the next change to a value, not by moving.
- **A fixed footer** (house rule 8). Only the rows scroll, keeping the
  focused one in view; on a short frame the focused row, the error line,
  the footer, the notes, and the top band are kept, in that order.
- **Option keys by kind.** `←` and `→` step a choice or a one-of-N row,
  flip a toggle, or move a check group's cursor; `Space` flips; on a text
  field the same keys edit.
- **Shapes that differ.** A one-of-N option is `(•)` or `( )`, a checkbox
  `[x]` or `[ ]`, both from vetted glyphs.
- **One grid.** The cursor gutter, a label column as wide as the longest
  shown label but between 11 and 16 cells, a two-cell gap, and the value.
- A host keeps fields that follow others in step by reading `Form::edited`
  after each key; a row kit lets a host draw rows of its own on the grid.

## Rationale

The host's forms had three behaviors for `Enter` and `Tab`, error displays
that scrolled away, hard-coded colors, and label columns of their own.

## Consequences

- Override rows build on this (record [0050](0050-override-rows.md)).

## Alternatives considered

- **"Enter advances" as a per-form policy.** Two behaviors to learn.
- **The error line under the rows, before the footer.** First to scroll
  away on a short terminal.
- **The error in place of the footer.** A form has rows to spare more often
  than a prompt.
- **A one-cell radio glyph.** Narrower, but the likely glyphs carry other
  meanings and a new one needs measuring.
- **`Space` as "next option".** A check group could not tell moving from
  flipping.
- **Field ids as a type parameter.** Every host enum of overlays would carry
  it; string ids keep one form type per message.
- **A fixed 11-cell label column.** Some forms need 16; the clamp gives
  both.

## Origin

- host ADR 0168, "Forms move on one ring, submit from any field, and keep
  their keys in view": decisions 1 to 8.
