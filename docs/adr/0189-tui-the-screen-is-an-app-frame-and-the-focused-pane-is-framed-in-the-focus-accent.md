# ADR 0189: The Screen Is An App Frame, The Panes Split By One Rule, And The Focused Pane Is Framed In The Focus Accent

## Status

Accepted. Applies house rule 9 of
[ADR 0151](0151-tui-framework-control-set-and-house-rules.md), carries
out [ADR 0158](0158-tui-modals-one-frame-and-named-sizes.md) decision 7
(the pane frame joins the framework's `frame` module, with the split
pane), and extends [ADR 0150](0150-tui-framework-workspace-crate.md) with
the framework's `app_frame` module. Amends
[ADR 0076](0076-scrollbar-widget-choice.md): its placement section said
the pane border is highlighted on the focused pane, which the code never
did; now it is. Amends
[ADR 0153](0153-tui-theme-roles-aliases-and-named-styles.md) decision 4:
`panel_focus_accent` colors the focused pane's border as well as its
marker.

## Context

The inventory for the framework extraction
(`docs/plans/tui-framework/inventory/07-chrome-and-explorer.md`, sections
1 to 3 and 8) found the frame around the screen written by hand:

- **The screen shell.** `ui::draw` split the screen into a header row,
  the body, and a status row, skipped all three while a modal filled the
  screen, drew the body, the status bar, the modal stack, and the toast,
  in that order, by hand. The draw already returned its measured windows
  in a `ViewportReport` (ADR 0156), and the three-way switch the
  inventory found in the runtime's `draw_screen` (transcript viewer, pane
  viewer, else the main screen) was already gone: the viewers became
  modal stack entries that fill the screen (ADR 0161, ADR 0177, ADR
  0180), so `draw_screen` was `ui::draw` and keeping the report.
- **The body split, copied.** `draw_body` halved the body side by side
  at or above `[tui] narrow_layout_threshold` and stacked it below. The
  snapshot tool's `pane_rect` rebuilt the same rows and halves "kept in
  sync by hand" for `--snapshot-pane`; six tests rebuilt the halves with
  their own `Layout`, and four consistency checks took the left pane to
  be the left `width / 2` cells.
- **The pane frames.** Both panes drew `Block::default().borders(ALL)`
  with a plain border; the tree's pane drew its block inside the
  framework's `TreeView`, the right pane its own. Each title began with a
  space and a focus marker span the host spliced in (`► ` in
  `panel_focus_accent` with focus, two blanks without), and each title
  carried its own rule for the trailing pad.
- **The focus cue was the marker alone.** Focus showed as the `►` in one
  title, the `[left]` or `[right]` tag in the status bar, and the cursor
  row's highlight. Every modal's border was the accent, every pane's
  plain. ADR 0076 described the focused pane's border as highlighted;
  house rule 9 asks for the border and the title marker in
  `panel_focus_accent`.

## Decision

1. **`tuikit::frame::PaneFrame`.** The bordered box a pane of the screen
   draws in, with the focus cue:
   - plain lines on all four sides, in `panel_focus_accent` while the pane
     has focus and in the terminal's own look while it does not;
   - a lead of three cells before the title (`LEAD`): a space, the focus
     marker (`marks::POINTER` in `panel_focus_accent` with focus, a blank
     cell in the same style without), and a space, so a title never moves
     as focus does;
   - the host's title after it in the modal title's form: its spans keep
     their styles, the spaces at its ends are dropped, and the frame
     closes it with one space when the border has a cell left. A reset
     style sits under the title, so the border's tint never reaches the
     title's text: only the border and the marker carry the cue;
   - `title_width(width)`, the cells a host lays its title out for (the
     border between its corners less the lead);
   - no title text: the lead while focused, an unbroken border otherwise;
   - no clearing and no background: a pane is the screen, not a box over
     it.
   The modal frame and the pane frame share the title-form helper.
   `TreeView` draws its pane in a `PaneFrame` given the tree's focus.

2. **`tuikit::frame::SplitPane`.** Two panes in halves of one area: side
   by side while the area is at least the threshold wide, the first above
   the second below it, an odd cell going to the first (ratatui's
   percentage split, so no frame moves). `stacks(area)` and
   `areas(area)` are the whole interface; a host keeps one split and asks
   it wherever it needs the panes.

3. **`tuikit::app_frame::AppFrame` and `Screen`.** The frame a screen is
   drawn in. `AppFrame` lays the screen out as a header (one row unless
   set), the body (the rest, at least `MIN_BODY` rows, which the header
   and footer give way to), and a footer (one row unless set), and
   `areas` returns that layout. `render` calls the parts a host supplies
   through the `Screen` trait in one order: the header, the body, and the
   footer unless `overlays_fill_screen`; then `overlays` over the whole
   area (the host draws its `ModalStack` there, bottom to top); then
   `toasts`. The body returns the host's `Report` (the measured windows),
   which `render` hands back, or the report's default when an overlay
   hid the body. Only `body` is required. The frame sends nothing and
   keeps nothing.

4. **Conspectus draws through them.** `ui::draw` is `AppFrame::render`
   over a `MainScreen` adapter that borrows the App: the header line,
   the panes, the status bar, `App::draw_modals`, and the toast. The body
   splits through `SplitPane` at the configured threshold; the tree draws
   through `TreeView` and the right pane through `PaneFrame`, each given
   its focus. The titles (the tab strip and the node title with its
   breadcrumb) lose their lead and their trailing pad, which the frame
   draws, and are laid out for `PaneFrame::title_width`. The host's draw
   helpers take a `Buffer`, as the framework's controls do.
   `ui::screen_areas` (the frame's rows and the split's halves) is the
   one layout `--snapshot-pane` and the tests find a pane by; the copies
   in `snapshot.rs`, `ui_tests.rs`, and the consistency fixtures are gone.

5. **The cue follows the pane's focus.** Under an open modal the focused
   pane keeps its accent border, as it keeps its marker and its cursor
   row's active highlight. The test kit's `frame_rect`, which finds a
   modal's frame by the corners that differ from the screen without it,
   completes a frame whose corners on one side match a tinted pane border
   cell for cell (a modal that reaches a focused pane's edges on a small
   screen).

## Consequences

- **Visible changes** (the CSP-757 summary and the changelog list them):
  the focused pane's border is drawn in `panel_focus_accent` (cyan by
  default), on all four sides and the top border's rule, beside the
  marker that was already that color; the other pane's border stays
  plain. Everything else draws as before: of 584 before-and-after
  snapshot frames (every view, each pane focused, side by side at 160x40
  and 100x30 and stacked at 80x24 and 60x40, each pane region, a drilled
  title, menus, Controls, help, Messages, quick select, the transcript
  viewer, a toast, and a status message), 296 are byte-identical and the
  other 288 differ only by that tint on the focused pane's border cells
  (some partly under a modal).
- A theme that sets `panel_focus_accent`, or only `accent` (which it
  follows), now colors the focused pane's border too.
- The screen's layout is stated once, in the framework, and found by the
  snapshot tool and the tests through one host function.
- A new pane, view, or tool frames itself with `PaneFrame`, splits with
  `SplitPane`, and reads areas from `screen_areas`; nothing re-derives
  the halves.

## Alternatives Considered

- **Keep the marker-only cue.** No visible change, but it leaves house
  rule 9 and ADR 0076 unmet and the pane and modal frames speaking
  different languages. Rejected by the house rules.
- **Tint the title text with the border, as a modal's title takes its
  border's color.** One title rule for both frames, but the right pane's
  bold kind label, the breadcrumb's plain parts, and the spaces would
  turn cyan as focus moves, which says nothing the marker does not and
  changes far more of the frame. The cue is the border and the marker.
- **A heavier border (thick or double) on the focused pane.** It reads
  without color, but needs a border-type token the theme does not have
  and changes the frame's glyphs; the marker already covers a terminal
  without color.
- **No tint while a modal is open**, so the only accent frame on screen
  is the one taking keys. Panes would change color as modals open and
  close while their markers and cursor rows did not, and the frame cannot
  know about modals; a host that wants it passes `focused(false)` while
  one is open. Left as a seam on the control page.
- **Closures instead of a `Screen` trait.** Five closures would each need
  the host's state mutably, so each would take it as an argument and the
  frame would carry five type parameters. A trait on a small adapter
  reads plainly, and its defaults leave every part but the body
  optional.
- **The frame owning the modal stack and the toasts.** The App lends its
  stack out with itself as the context (ADR 0161), so the frame cannot
  hold the stack and the App at once; the trait's `overlays` and `toasts`
  are where the host draws them.
- **Halve by `width / 2`.** What the old tests assumed; it gives the odd
  cell to the second pane, which moves every frame at an odd width.
  The split keeps ratatui's percentage split.
- **A split ratio or a focus-weighted split now.** No caller needs one;
  it is a parameter of `SplitPane` when one does.

## Open Questions Answered

- *Does the focus cue change?* Yes: the focused pane's border joins the
  marker in `panel_focus_accent`; the unfocused pane keeps the plain
  border, as ADR 0151 decided and ADR 0076 described.
- *Does a pane's title read like a modal's?* In form, yes: trimmed, with
  a space on each side, after the lead. In color, no: a pane's title text
  keeps its own looks.
- *Does the full-screen switch go through the frame?* Yes. The viewers
  were already stack entries that fill the screen; `overlays_fill_screen`
  is the switch, so `draw_screen` stays the runtime's wrapper that keeps
  the report.
