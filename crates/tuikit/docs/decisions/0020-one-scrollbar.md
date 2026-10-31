# 0020: One scrollbar, ratatui's, in a gutter carved only on overflow

| | |
| --- | --- |
| Status | Accepted |
| Page | [controls/scroll.md](../controls/scroll.md) |

## Decision

- `ScrollGutter::layout(area, rows)` carves a one-column gutter from the
  body's right edge only when the rows overflow and the body is at least two
  columns wide: the bar fades when the content fits.
- `render` draws ratatui's vertical `Scrollbar` in that gutter with a one-row
  margin at the top and bottom. The thumb is placed by the window it is
  given, at the bottom exactly at the last offset, its length the window's
  share of the content, and the reversed modifier is removed so a selection
  highlight does not show through.
- Rows that overflow are laid out one cell narrower, the width the gutter
  leaves, so no text runs under the bar.
- The bar sits inside a pane's border, never on it: the border carries the
  focus cue.

## Rationale

Without a bar nothing said that more content lay beyond the view. Ratatui's
widget is already in the build, and its state (content length and position)
is what the scroll window already holds, so it composes without changing
who owns scrolling. One recipe gives every scrolled control one look.

## Consequences

- Every scrolled control draws its bar through the gutter, and a short list
  pays no column for it.

## Alternatives considered

- **A scroll-view crate** (`tui-scrollview`) that renders into an off-screen
  buffer and owns the offset. It inverts ownership, adds a dependency, and
  suits nested widgets rather than a tall list.
- **Drawing the bar over the content.** It hides the last column.
- **Drawing the bar on the border.** It fights the focus cue.

## Origin

- host ADR 0076, "Scrollbar widget choice for scrolled panes": the widget,
  fade on fit, the length semantics, and the placement inside the border.
- host ADR 0156, "One scroll window, one follow rule, and viewports returned
  by value": decisions 5 and 6 (the carved gutter, the one recipe, and
  laying out one cell narrower).
