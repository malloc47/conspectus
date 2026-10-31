# 0026: A screen draws in one order through an app frame

| | |
| --- | --- |
| Status | Accepted |
| Page | [controls/app_frame.md](../controls/app_frame.md) |

## Decision

- `AppFrame` lays a screen out as a header (one row unless set), the body
  (the rest, at least `MIN_BODY` rows, to which the header and footer give
  way), and a footer (one row unless set); `areas` returns that layout.
- `render` calls the parts its host supplies through the `Screen` trait in
  one order: the header, the body, and the footer unless an overlay fills
  the screen; then the overlays over the whole area, where the host draws
  its modal stack bottom to top; then the toasts.
- The body returns the host's report (the windows it measured), which
  `render` hands back, or the report's default when an overlay hid the
  body. Only the body is required. The frame sends nothing and keeps
  nothing.

## Rationale

One draw order for every screen: chrome, then every overlay from the bottom
of the stack to its top, then transient feedback. Handing the layout back as
areas lets anything that needs a part's cells again (a snapshot tool, a
test) find it from the same computation.

## Consequences

- A minimum-size screen (one centered empty state below a size the host
  names) is a seam on the page, not built.

## Alternatives considered

- **Closures instead of a trait.** Five closures would each need the host's
  state mutably, so each would take it as an argument and the frame would
  carry five type parameters; a trait on a small adapter reads plainly, and
  its defaults leave every part but the body optional.
- **The frame owning the modal stack and the toasts.** The host lends its
  stack out with itself as the context (record
  [0033](0033-overlays-on-a-host-owned-stack.md)), so the frame cannot hold
  both at once.

## Origin

- host ADR 0189, "The screen is an app frame, the panes split by one rule,
  and the focused pane is framed in the focus accent": decision 3.
- host ADR 0161, "Overlays draw themselves on one host-owned modal stack":
  decision 6 (the one draw order).
- host ADR 0190, "The tab strip, the header, and the status bar give way by
  one rule, and an empty slot says so one way": decision 8 (no minimum-size
  screen yet, and its seam).
