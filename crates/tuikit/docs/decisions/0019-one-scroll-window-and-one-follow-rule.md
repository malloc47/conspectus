# 0019: One scroll window, one follow rule, and windows returned by value

| | |
| --- | --- |
| Status | Accepted |
| Page | [controls/scroll.md](../controls/scroll.md) |

## Decision

- **Stepping by kind.** Short choice lists step with `wrap_step` and
  scrollable content with `clamp_step` (house rule 2).
- **One follow rule, minimal motion.** `follow_offset` first brings the
  offset back inside the content, then moves the window down until the
  cursor's last row is its last line, or up until the cursor's first row is
  its first line, and otherwise leaves it. No centering, no margin. A span
  taller than the window shows its start, the row that names the item.
- **One clamping window.** `ScrollWindow` is a `Copy` offset, viewport, and
  content; every constructor and step clamps, so after any sequence of
  steps, pages, jumps, and resizes the offset is inside the content. Its
  page is the rows it shows (house rule 12).
- **Windows are returned, not sent.** A control works its window out from
  what it kept and the area it was given, draws it, and returns it; its
  owner keeps it after the frame. Nothing is sent from inside a draw.
- **Rows counted from the rows drawn.** `WrappedRows` wraps with the text
  module and keeps each line's rows, so a scroll range, a scrollbar's
  length, and a wrapped cursor's span are counted from what is drawn.

## Rationale

Scroll state reconciled during a draw (offsets written through interior
mutability, viewport sizes sent back as messages) made drawing impure and
the next frame late, and each scrolled surface followed its cursor its own
way. Keeping a window is no state transition, so returning it by value keeps
the draw a function of state.

## Consequences

- Property tests over generated key sequences check that the cursor stays in
  the window and the window in the content.
- A host's draw returns a report of the windows it drew and keeps them after
  the frame.

## Alternatives considered

- **A message after each frame carrying the windows.** Every state change
  stays a message, but a frame is no user action: the host's reducer would
  have to exempt it from what actions do, and every message-driven test
  would have to send it.
- **Interior mutability**, offsets written during a shared-borrow draw.
  It hides the write and makes the draw order-sensitive.
- **A measure pass before the draw.** The screen would be laid out twice,
  duplicating the layout code where estimates used to disagree with the
  draw.
- **The end of a tall span winning.** It hides the row that names the item.
- **Scrolling from a third-party crate.** One inverts who owns the offset;
  another is all or nothing.

## Origin

- host ADR 0156, "One scroll window, one follow rule, and viewports returned
  by value": decisions 1 to 4, 6, and 7.
- host ADR 0085, "The interface's Elm/MVU architecture": contract 5 as
  amended (the draw returns the windows it drew, and no viewport is sent as
  a message).
