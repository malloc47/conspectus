# ADR 0194: A Step Drawn Exactly Over Its Entry Is Found By What Changed Inside Its Frame

## Status

Accepted. Amends
[ADR 0164](0164-tui-test-kit-key-scripts-frame-driver-goldens-conformance.md)
decisions 4 and 7 (the buffer probes and the conformance suite) as amended by
[ADR 0176](0176-tui-settings-panels-apply-as-they-change-and-editors-are-steps.md):
the checks find the frame of a `Case::step` that covers the entry under it
exactly. Carried out by CSP-780.

## Context

The conformance suite finds a modal's frame by the border corners that
differ between the screen with the modal and the screen after
`Rig::close_top` (`buffer::frame_rect`). A step opened over a menu
(`ModalStack::push_step`, ADR 0172) takes the menu's frame size and can
sit at exactly the menu's rect: when it has as many rows as the menu, or
when both fill a short screen. Then no corner differs, and every frame
check reported "draws no frame". The host worked around it with facts
chosen to size its dialogs apart from their menu (a worktree with three
live sessions), and its 40x10 body check compared the screen with the
whole flow closed.

The overlay page left a seam open for this: an entry could report the
rect it drew.

## Decision

1. **`buffer::covering_frame(with, without)`** is the smallest frame
   `with` draws around every cell that differs from `without`. A modal
   drawn exactly over another frame changes no corner, but what changed
   (its body, its title, its keys) lies inside its own frame, and no
   smaller frame holds all of it.
2. **Only a step gets that second look.** The checks take a case's frame
   from `frame_rect`, and for a `Case::step` that finds none, from
   `covering_frame`. Any other modal whose frame cannot be told from the
   screen under it draws no frame of its own, so a modal that draws no
   frame inside a pane still fails rather than passing on the pane's
   border.
3. **The host drops its workaround.** Its worktree step cases use a
   worktree with one live session, whose remove confirmation covers the
   menu exactly at most of the suite's sizes, and its 40x10 body check
   compares with the top modal closed (and the status bar, which names
   the top modal's keys, left as it is).

## Consequences

- Any step at any size is checked, with no fact of the host's chosen to
  keep it apart from the entry under it.
- A step identical to the entry under it, cell for cell, is still found
  by nothing; it would draw nothing new.
- The suite needs nothing from the overlay contract: no entry reports a
  rect. The rendered-areas seam stays open for mouse hit testing.

## Alternatives Considered

- **Every entry reports the rect it drew** (`Overlay::rect`, kept by the
  stack, read through a `Rig` method). Exact, and the seam mouse hit
  testing wants, but every control and every host overlay would implement
  it, and a rect computed ahead of the draw can drift from the one drawn.
  The suite needs only what the screen shows.
- **The covering frame for every case.** Simpler, but a modal that draws
  no frame of its own inside a pane would pass with the pane's border as
  its frame.
- **Keep comparing with the whole flow closed.** It finds the step's frame
  only while the step is no smaller than the entry under it, and it reads
  the entry's own drawing as the step's.

## Open Questions Answered

- *How is a step's frame found when it covers its entry exactly?* As the
  smallest frame around what changed inside it.
- *Does the overlay contract report rendered areas now?* No; the seam is
  still open.
