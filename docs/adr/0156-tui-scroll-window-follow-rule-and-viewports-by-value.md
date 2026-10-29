# ADR 0156: One Scroll Window, One Follow Rule, And Viewports Returned By Value

## Status

Accepted. Applies house rules 2 and 12 of ADR 0151 and extends ADR 0150
with the framework's `scroll` module. Amends ADR 0076 (scrollbar
choice): the gutter is carved out of the body instead of the bar being
drawn over it, and a window's thumb is placed by the rows that window
scrolls. Amends ADR 0085 contract 5 (one draw contract): scroll is
reconciled in the draw from the App and the measured area, and the
drawn windows come back to the App as a value, not as a `Msg`. Amended
by [ADR 0161](0161-tui-overlays-draw-themselves-on-one-host-owned-stack.md):
a scrolled modal (the help page, the value modal, Messages) keeps the
window it drew as it draws, so the viewport report carries the panes'
windows only. Amended by
[ADR 0177](0177-tui-readers-are-one-pager-over-content-the-host-lays-out.md):
the framework's pager keeps a window with a sticky end, and the pane
viewer is one.

## Context

The inventory for the framework extraction
(`docs/plans/tui-framework/inventory/`) found the TUI scrolling the same
way in many places that disagreed:

- **Two steppers and hand-rolled clamps.** `wrap_step` and `clamp_step`
  lived in `src/tui/cursor.rs`; three menus hand-rolled their own clamp.
- **Seven scroll-to-cursor copies.** Two `scroll_offset_for_cursor`
  functions (the forms', the Controls overlay's), two `compute_scroll`
  copies (multi-select, search), an inline formula each in the picker,
  the Messages list, and the row menu, and two follow functions on `App`
  (`adjust_left_scroll`, `adjust_explorer_scroll`) that differed on
  whether the offset was clamped to the content and on which end of a
  tall cursor line won.
- **Five scrollbar sites, one a copy.** Four called the left pane's
  `scrollbar_layout` and `render_vertical_scrollbar`; the help page
  rebuilt the same recipe inline.
- **Wrapped-row counting in several places**: `ui::wrap_lines` and
  `wrap_into`, the explorer's own per-line wrap and cursor-span sums,
  and the banners' counts.
- **Offsets reported by messages from inside the draw.** The left pane
  and the explorer sent `Msg::LeftViewportChanged` and
  `Msg::ExplorerViewportChanged` mid-frame so the reducer could
  reconcile their offsets, and the reducer had to exempt both from the
  rule that any message disarms the explorer's Backspace confirmation.
  The overlays already used a better shape: each draw returned a
  `ScrollFit` that the draw path wrote back.
- **Unclamped offsets.** The explorer kept an offset past its content
  after the content shrank, and the Preview drew a blank tail when
  scrolled past its end.

## Decision

1. **`tuikit::scroll` owns stepping, following, the window, the gutter,
   and the row count.** Its items: `wrap_step` and `clamp_step`;
   `follow_offset`; `ScrollWindow`; `ScrollGutter`; `WrappedRows`.

2. **Stepping.** Short choice lists step with `wrap_step`, scrollable
   content with `clamp_step` (house rule 2). The menus' hand-rolled clamps
   are left for the `selection` extraction, which makes them wrap.

3. **One follow rule, minimal motion.** `follow_offset(offset, rows,
   viewport, content)` first brings the offset back inside the content
   (never past `content - viewport`), then moves the window down until
   the cursor's last row is its last line, or up until its first row is
   its first line, and otherwise leaves it alone. No centering, no
   margin. A span taller than the window shows its start, the row that
   names the item (the row menu already did this; the explorer showed
   the span's end). A list that keeps no window follows from 0, which is
   exactly the "cursor on the last line once it passes the first screen"
   formula the list overlays used, so their frames do not change.

4. **One clamping window.** `ScrollWindow` is a `Copy` offset, viewport,
   and content. Every constructor and step clamps, so after any sequence
   of steps, pages, jumps to the ends, and resizes the offset is inside
   the content; property tests over generated key sequences check that
   the cursor stays in the window and the window in the content. Its
   `page()` is the rows it shows, which page keys move by (house rule
   12). It replaces `popup_frame::ScrollFit`.

5. **One gutter and one bar.** `ScrollGutter::layout(area, rows)` carves
   a 1-column gutter from the body's right edge only when the rows
   overflow and the body is at least two columns wide (fade on fit).
   `render` draws ratatui's vertical scrollbar in it with a 1-row margin
   at the top and bottom, the thumb at the bottom exactly at the
   window's last offset, its length the window's share of the content,
   and the reversed modifier removed so a selection highlight does not
   show through. The thumb is placed by the window it is given, so a
   transcript preview under a banner places it by the transcript's own
   rows, not by the zone's height.

6. **One row counter.** `WrappedRows` wraps lines with
   `text::wrap_line` and keeps each line's row range, so a scroll range,
   a scrollbar's length, a zone's height, and a wrapped cursor's span
   are counted from the rows drawn (ADR 0155). `fit` wraps at the full
   width when the rows fit and one cell narrower when they overflow,
   which is the width the gutter leaves.

7. **Viewports are returned, not sent.** A renderer works its window out
   from the App (the window kept from the last frame and the cursor) and
   the area the layout gave it, draws it, and returns it. `ui::draw`
   returns a `ViewportReport` with the windows of the tree, the explorer,
   the Preview, the help page, the value modal, and the Messages detail;
   the caller hands it to `App::keep_viewports` after the frame.
   Keeping a window changes no selection, focus, or arm, so it is not a
   reducer transition and is not a `Msg`; the two viewport messages and
   the reducer's exemption for them are gone. The App keeps a
   `ScrollWindow` for the tree and the explorer (the per-view slot saves
   the tree's), and the Preview's offset is taken back inside the window
   its loaded content drew. A Preview still showing its placeholder
   keeps its offset, so a key pressed while it loads is not lost. `draw`
   still borrows the App mutably for one thing, the right pane's warm-up
   of the transcript preview's wrapped lines; the framework controls draw
   from shared borrows.

## Consequences

- Default frames are unchanged: the before-and-after snapshot frames of
  every view, pane, overlay, and form at five sizes differ only where an
  offset used to run past its content.
- Visible: a plain or pane Preview scrolled past its end shows its last
  page instead of a blank tail, and the first `J` or `K` back moves it at
  once; the explorer fills its zone after its rows shrink instead of
  showing the last row over blank rows; a cursor line taller than the
  explorer zone shows its first rows.
- No renderer of the main screen sends a message; a source check over
  `src/tui/ui.rs` and `src/tui/ui/` keeps it so.
- Later controls (`TreeView`, `Pager`, `Explorer`, the scroll region of
  the Preview) build on the window, the follow rule, and the gutter
  instead of growing their own. The pagers' own offsets (help, the value
  modal, Messages, the viewers) still step their stored `u16` and clamp
  through `fit`; the `Pager` extraction moves them onto the window, with
  its sticky end.

## Alternatives Considered

- **A post-frame `Msg::FramesDrawn(report)`.** Keeps every state change
  in `update`, but a frame is not an operator action: the reducer would
  again have to exempt it from disarming the Backspace confirmation, and
  every message-driven test would have to send it. The report is applied
  by one method instead.
- **Interior mutability (`Cell` offsets written during a `&App` draw).**
  What ADR 0085 set out to remove; it hides the write and makes the draw
  order-sensitive.
- **A measure pass before the draw.** The runtime would lay the screen
  out twice to learn the windows before drawing. It duplicates the
  layout code, which is where the old estimators disagreed with the
  draw.
- **Keep the explorer's end-wins rule for tall spans.** It hides the
  cursor line's label, the row that says what is selected.
- **True minimal motion in the list overlays (a stored window per
  overlay).** It would change how menus, the picker, and the search
  lists scroll when moving up from their last row, and those lists move
  to `selection` and `Palette` later; they keep the stateless follow from
  0 until then.
- **`tui-scrollview` or `rat-widget` scrolling.** ADR 0076 and CSP-433
  rejected both; nothing here needs a new dependency.

## Open Questions Answered

- *Does the window hold the cursor?* No. A list keeps its own cursor and
  passes its rows to `follow`; the window is the same for a pager with no
  cursor.
- *Is the scrollbar themed?* Not yet: it uses ratatui's default look,
  as ADR 0076 decided. Thumb and track tokens are a seam in the control
  page.
- *What happens to keys pressed while the Preview loads?* They keep
  counting; the first frame that shows the content clamps the offset.
