# 0040: Every modal footer is one footer, with one look and one fit

| | |
| --- | --- |
| Status | Accepted |
| Page | [controls/frame.md](../controls/frame.md) |

## Decision

- **One footer.** `frame::ModalFooter` draws every modal's footer: a
  `Footer` of key segments, the line a modal keeps above its keys (a form's
  error line, a reader's find prompt or chip), and the blank row that
  separates them from the body. Its keys break only between segments, and
  the same footer can sit on a frame's bottom border.
- **One look.** Every segment takes the theme's `muted` and no color of its
  own, so it shows the color of where it sits: the body's foreground, or the
  accent on a bottom border. A segment that cannot act now keeps `inert`
  with `muted`. The keys sit on the frame's last rows under a blank row.
- **The blank row gives way first**, and only when the footer does not fit.
- **One fit.** The footer leaves the body at least one row. When it wants
  more rows than that leaves, the blank row goes, the status line keeps a
  row while there is one, the keys keep their last rows, and their first
  rows spill after the body's last row, where they scroll with it. No key is
  lost, and the keys that leave are the last to go out of view. Beside a
  scrollbar the footer is laid out a cell narrower. A frame of eight cells
  or fewer keeps the footer on one row, cut at its edge.

## Rationale

House rule 8 covers what a footer says; this is how it looks and fits.
Footers looked different from control to control (plain under a blank row,
receding without one), and short frames dropped the keys that leave first.

## Consequences

- The action menu, dialog, checklist, settings panel, form, palette, framed
  pager, log list, and prompt all draw through it, and the conformance
  suite's footer check holds it.

## Alternatives considered

- **The plain look under a blank row.** A plain footer reads like one more
  entry.
- **Receding with no blank row.** A row more for the body, but a cursor
  row's highlight runs into the keys, and terminals that draw `dim` as
  nothing lose the separation.
- **The blank row giving way whenever the body does not fit.** The look
  would depend on how long the content is.
- **Keeping the footer's first rows.** It drops the keys that leave first.
- **Dropping keys by priority.** It keeps the keys that leave, but the
  dropped keys are gone; spilled ones are one scroll away.
- **A cap on the footer's rows.** A second limit beside the body's one row,
  and it cut the keys that leave.

## Origin

- host ADR 0193, "Every modal footer is one footer: one look under a blank
  row, and one fit that keeps the keys that leave": decisions 1 to 4.
- host ADR 0178, "Help pages and long values are pager contents, and a
  framed footer scrolls what does not fit": decision 3 (the fit that spills
  a footer's first rows after the body).
