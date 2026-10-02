# ADR 0106: Preview Pane Wrap Modes

## Status

Accepted.

## Context

The Preview pane shows a `tmux capture-pane -J -e` capture of the
selected row's mux (ADR 0025). The capture is laid out for tmux's
width, and the preview is usually narrower. Two problems followed:

- **Blank bottoms hid the output.** The preview kept the capture's last
  N source lines. A quiet pane, like a server that printed a few lines
  and waits, has a screen of blank rows below its output, so the
  preview showed only those blanks. `conspectus serve` previewed as
  empty.
- **Decorations wrapped.** The `Paragraph` wrapped every line at the
  preview width. Agent UIs draw full-width rules (`────…`), box borders
  (`╭─…─╮`, `│ … │`), and padded status lines. In a narrower preview each
  one spilled onto a second row of rule fragments or a lone `│`,
  crowding out the content. Cropping by source lines also ignored those
  extra rows, so the newest lines could be pushed below the visible
  area.

## Decision

1. **Lay the capture out before cropping.** A pure layout step turns
   the parsed capture into display rows that each fit the preview
   width. It first drops trailing blank lines (blank meaning only
   whitespace, whatever the styling), then the preview keeps the
   bottom rows that fit. With a failure banner (ADR 0105) above it,
   the pane body gets the rows the banner leaves.
2. **Three wrap modes**, as `PreviewWrap`:
   - **smart** (the default) wraps content but not formatting. Per
     line it drops trailing padding, then:
     - truncates when everything past the edge is decoration (box
       drawing, block elements, dash and rule characters) or blank;
     - otherwise squeezes interior runs of two or more spaces, widest
       first, when that fits the line on one row;
     - otherwise word-wraps with a hanging indent matching the line's
       leading indent.
   - **plain** word-wraps every line as is: the earlier behavior.
   - **none** keeps tmux's own layout: it re-wraps joined lines at the
     pane width, then clips each row to the preview width.
3. **The capture reports the pane width.** `capture_pane` runs
   `display-message -p '#{pane_width}' ; capture-pane …` as one tmux
   call and returns the width with the text (`PaneCapture`). The width
   is the size of the client that last sized the window. It stays in
   the preview cache, not in the graph: it changes on every resize,
   and the graph cache shouldn't churn for it.
4. **Configured, remembered, menu-first.** `[tui] preview_wrap =
   "smart" | "plain" | "none"` sets the starting mode. The controls
   overlay has a "Preview wrap" section in every view, and the last
   pick persists in `tui-state.json` like the mux recency basis does.
   No single-key binding is added.

## Consequences

- An idle pane previews its last real output.
- Smart wrap is a heuristic. A line whose overflow is only dashes or
  box characters loses that tail, which can drop a closing border or
  the end of an ASCII rule. Squeezing can misalign a table wider than
  the preview. Plain wrap is one menu pick away when the heuristic
  gets it wrong.
- No-wrap needs the pane width. When a backend doesn't report one, it
  clips each joined line instead.
- Pane rows are fitted before the `Paragraph` sees them, so the
  preview's own wrapping only affects banners and text bodies.

## Alternatives Considered

**Capture without `-J` for no-wrap.** tmux would hand back its own
wrapped rows, but smart and plain need joined lines. That means two
captures, or a mode change that triggers a recapture.

**Store the pane width on `MuxSessionNode`.** The width changes with
every client resize. Discovery would see a changed node and rewrite
the graph cache for a value only the preview reads.

**Strip decorations entirely.** Dropping rule and border lines would
lose the visual separation between an agent's output and its prompt.
Truncating keeps it in one row.

**A keybinding to cycle modes.** Faster for operators who know it,
but the project puts new controls in navigable menus first.
