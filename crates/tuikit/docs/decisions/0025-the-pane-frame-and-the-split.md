# 0025: A pane is framed with the focus cue, and two panes split by one rule

| | |
| --- | --- |
| Status | Accepted |
| Page | [controls/frame.md](../controls/frame.md) |

## Decision

- **`PaneFrame`** is the bordered box a pane of the screen draws in. Its
  plain border is in `panel_focus_accent` while the pane has focus and in
  the terminal's own look while it does not. A three-cell lead comes before
  the title (a space, the focus marker, a space): the pointer mark in the
  focus accent with focus, a blank cell without, so a title never moves as
  focus does. The title follows in the modal title's form, over a reset
  style so the border's tint never reaches the title's text.
  `title_width(width)` is the room a host lays its title out for. A pane
  frame clears nothing and paints no background: a pane is the screen, not
  a box over it.
- **`SplitPane`** puts two panes in halves of one area: side by side while
  the area is at least a threshold wide, stacked below it, the odd cell to
  the first.
- **The cue follows the pane's focus**, also under an open modal, as the
  pane keeps its marker and its cursor row's active look.

## Rationale

House rule 9. A marker alone was easy to miss; a tinted border shows at a
glance which pane has the keys. Keeping the title untinted and fixed in
place means only the border and the marker change with focus.

## Consequences

- A scrollbar sits inside the border, never on it (record
  [0020](0020-one-scrollbar.md)).
- The modal frame and the pane frame share one title helper.

## Alternatives considered

- **The marker-only cue.** No visible change, but house rule 9 unmet and the
  pane and modal frames speaking different languages.
- **Tinting the title text with the border**, as a modal's title takes its
  border's color. One title rule, but a pane's title carries its host's own
  styled parts, which would change color as focus moves and say nothing the
  marker does not.
- **A heavier border on the focused pane.** It reads without color, but
  needs a border-type token and changes the frame's glyphs; the marker
  already covers a terminal without color.
- **No tint while a modal is open.** Panes would change color as modals
  open and close while their markers did not, and the frame cannot know
  about modals; a host that wants it passes the pane as unfocused. A seam
  on the page.
- **Halving by `width / 2`.** It gives the odd cell to the second pane,
  which moves every frame at an odd width.
- **A split ratio now.** No caller needs one; it becomes a parameter when
  one does.

## Origin

- host ADR 0189, "The screen is an app frame, the panes split by one rule,
  and the focused pane is framed in the focus accent": decisions 1, 2, and
  5.
- host ADR 0076, "Scrollbar widget choice for scrolled panes": its placement
  section (the pane's border carries the focus cue).
