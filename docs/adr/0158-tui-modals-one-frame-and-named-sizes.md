# ADR 0158: Modals Share One Frame And Five Named Sizes

## Status

Accepted. Carries out ADR 0152 item 4 (`tui-popup` becomes the
framework's `ModalFrame`, which sizes once), applies house rules 8, 13,
and 14 of ADR 0151, and extends ADR 0150 with the framework's `frame`
module. Amends ADR 0149 item 25: the search modal's rect is the palette
policy, with the same shares and bounds, whose minimums now beat the
margin on a small terminal. Amended by
[ADR 0166](0166-tui-text-prompts-one-overlay-that-explains-its-refusals.md):
a frame may carry a footer on its bottom border, in the title's form, for
a modal whose body has no row to spare for its keys (the one-line
prompt). Amended by
[ADR 0174](0174-tui-palettes-one-control-a-query-over-ranked-results.md):
the picker is a palette and takes the `PALETTE` size, not `WIDE`.
Decision 7 carried out by
[ADR 0189](0189-tui-the-screen-is-an-app-frame-and-the-focused-pane-is-framed-in-the-focus-accent.md):
the pane frame, which shares the title form and carries the focus cue,
and the split pane join the `frame` module.

## Context

The inventory for the framework extraction
(`docs/plans/tui-framework/inventory/`) found the TUI's modals framed and
sized in many ways that shared one function, the centering math:

- **Thirteen sizing rules.** Every modal chose its own width and height:
  `min(cap, w - 4).max(min)` with caps 60, 64, 76, 78, 80, 84, and 100
  (the text appeared ten times, and each form repeated it a second time
  to work out its inner width); the Messages log `(w - 4).clamp(40, 110)`;
  the search overlay 85% and 80% of the terminal inside a one-cell
  margin; the row, mux, worktree, and Clean up menus `min(72, 62, 56,
  68, w)`, flush with the screen's edges on a narrow terminal while every
  other modal kept two cells clear; heights at content plus the border
  with floors of 0, 5, 6, 8, and 10, or the terminal less 2 or 4 rows, or
  28 rows. The picker's 20-row cap could never apply.
- **Four ways to draw the frame.** Eleven modals went through
  `themed_popup` (a `tui_popup::Popup` with an accent border), four built
  the same `Clear` and `Block` by hand, two kept an unthemed fallback no
  caller used, and `LinesBody` had no caller.
- **Each modal sized twice.** `tui-popup` centers a body from its
  `KnownSize`, so each modal computed its rect, stored the inner width
  and height in a body struct, and the popup computed the rect again from
  them: eleven identical `KnownSize` impls. A form laid out for the width
  its rule gave but was drawn clipped to the terminal when the minimum
  was wider than the screen, so at 40 columns its key hints were cut.
- **Titles in several forms.** Padding came from the caller's string, the
  widget, or nowhere (the rename prompt, Clean up's checklist), and the
  value modal and Messages titles were bold with two spaces at each end.
  The border's accent colored every title, bold or not.
- **Two centerings.** The popup put the odd cell left over before the
  modal (ratatui's `Rect::centered`); the hand-built menus put it after.

## Decision

1. **`tuikit::frame` owns modal sizing and the frame.** Its items:
   `Extent`, `Fit`, `ModalSize` and its five named policies, `ModalFrame`,
   the `ModalBody` trait, and `Lines`.

2. **One sizing rule, as parameters.** Each direction is an `Extent`: a
   share of the screen, a cap, a minimum, and a margin. The modal aims for
   the share, takes no more than the cap or the screen less the margin on
   each side, grows to the minimum, and never passes the screen: the
   margin gives way to the minimum, and the screen bounds both. A `Fit`
   says whether the height is the body's rows (with the border and any
   padding) or all the extent allows. The rect is centered with the odd
   cell before it, as ratatui centers. A modal names a policy; it never
   computes a rect.

3. **Five named sizes**, each keeping two columns and one row clear on
   each side of the screen while it can, so a header and a status line
   stay in view around any modal:

   | Policy | Width | Height | Modals |
   | --- | --- | --- | --- |
   | `COMPACT` | up to 60, at least 20 | the body's | the rename, branch, max-age, and contents prompts; the checklists (harness, mux state, PR state, Clean up) |
   | `MENU` | up to 64, at least 40 | the body's | the row, mux, and worktree menus; Clean up when it has nothing to list; Controls |
   | `FORM` | up to 80, at least 44 | the body's | the new mux, mux launch, and pin edit forms |
   | `WIDE` | up to 100, at least 40 | all, at least 10 | the help pages (keymap, viewer, pane viewer); the value modal; Messages; the picker |
   | `PALETTE` | 85%, 60 to 200 | 80%, 12 to 50 | quick select and content search |

   The thirteen rules reduce to these: the caps that differed by a few
   cells (56, 62, 64, 68, 72 for menus; 76, 76, 84 for forms) take one
   value per role; the reading surfaces (help, the value modal, Messages)
   and the picker, a list whose frame must not change size as it is
   filtered, fill the screen like the help; the search keeps its shares.

4. **One frame.** `ModalFrame` clears its rect and paints the theme's
   `surface` (the terminal's background by default), draws a plain border
   on all four sides in `accent`, and writes the title in one form:
   left-aligned, one space on each side (spaces a caller added at the ends
   are dropped), in the border's color, regular weight. Chips a title
   carries (the search's segmented choices) keep their own look. Padding
   between the border and the body is optional and counted by the sizing.

5. **The plain title wins.** Twelve of fourteen titles were plain and two
   bold. The plain form is kept: a bold accent title on an accent border
   separates little more from the line, terminals that draw bold as
   bright change its color, and in the search title the chosen segment (a
   reversed, bold chip) should be the only emphasis.

6. **A body is measured once.** The frame tells a `ModalBody` the inner
   width its policy gives (`measure`), places itself for the rows the body
   reports, and hands the body the inner area (`render`), whose result it
   returns (a scrolled body returns its window). `Lines` is the body for
   lines laid out ahead for `inner_width`, scrolled to keep a cursor's
   rows in view. `tui-popup`, `popup_frame.rs`, `LinesBody`, the
   `KnownSize` impls, `form::modal_rect`, and the per-modal rect functions
   are removed; the prompt and checklist widgets take the theme in their
   constructors, so no frame is drawn unthemed.

7. **The pane frame comes later** in the same module, sharing the border
   and title rule, with the focus cue; this decision does not build it.

## Consequences

- Default frames change as listed in the CSP-726 summary and the
  changelog: menus are 64 columns (the row menu was 72, the mux menu 62,
  the worktree menu 56, Clean up 68) and keep two columns clear on a
  narrow terminal; forms are 80 (76, 76, 84); the picker and the value
  modal are the help's size; the help is two rows taller; menus, forms,
  and Controls on a short terminal stop a row short of the screen's top
  and bottom and scroll; the menus' minimum heights are gone, so a short
  worktree menu hugs its entries; the hand-built menus move a cell when
  the space around them is odd; the Messages and value modal titles lose
  their bold and their extra spaces, and the rename prompt and checklist
  titles gain their spaces; the search overlay keeps its 60-column and
  12-row minimums on terminals narrower than 62 columns or shorter than
  14 rows instead of leaving a one-cell margin there; the empty Clean up
  wraps its sentence instead of cutting it;
  and a form on a terminal narrower than its minimum lays itself out for
  the width it is drawn at, so its key hints wrap instead of being cut.
- A new modal picks a named size and is a `ModalBody`; it cannot drift
  from the others' margins, border, or title.
- The layout consistency checks gain one that every modal draws one
  accent frame with one title form.
- The build loses `tui-popup` and the two crates it pulled in.

## Alternatives Considered

- **Keep `tui-popup` behind `ModalFrame`.** Least change, but the body
  still reports a size the frame then recomputes, and the popup's public
  types would sit in the framework's API. ADR 0152 already decided
  against it.
- **Keep each modal's numbers as its own policy.** No visible change, but
  thirteen policies are the thirteen rules renamed; the story's point is
  that a menu is a menu.
- **The margin beats the minimum.** The search kept its one-cell margin
  at every size, but every other modal would narrow on small terminals
  (the forms at 40 columns to 36), which shows less of each.
- **No vertical margin for content-sized modals.** Menus and forms on a
  short terminal keep two more rows, but cover the header and the status
  line, unlike the readers; one margin for every modal is the simpler
  rule.
- **Accent-bold titles everywhere.** A stronger title, but it changes
  twelve titles instead of two and competes with the search title's
  chosen chip.
- **A centering that puts the odd cell after.** It kept the hand-built
  menus in place but moved the eleven popup modals; ratatui's own rule
  moves fewer.

## Open Questions Answered

- *How many sizes?* Five: compact, menu, form, wide, and palette.
- *Does the frame own the title's look?* Its form and weight, yes; a
  title's chips keep theirs.
- *Can a host size a modal some other way?* Yes, with a `ModalSize` of
  its own built from the same parameters, never a rect of its own.
