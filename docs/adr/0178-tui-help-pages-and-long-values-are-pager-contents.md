# ADR 0178: Help Pages And Long Values Are Pager Contents, And A Framed Footer Scrolls What Does Not Fit

## Status

Accepted. Carries out what
[ADR 0177](0177-tui-readers-are-one-pager-over-content-the-host-lays-out.md)
left for the value modal and the help page's body, applying house rules 2,
4, 8, 12, and 18 of
[ADR 0151](0151-tui-framework-control-set-and-house-rules.md). Amends
[ADR 0162](0162-tui-help-pages-one-page-generated-from-key-tables.md)
decisions 1, 4, 5, and 6: `HelpPage` is a pager's content, `HelpScroll`
and the page's own drawing are removed, and the page's frame, scrollbar,
footer, and scrolling are the pager's. Amends ADR 0177 decisions 3 and 7:
a framed pager fits a footer too tall for its frame by the help page's
rule, and `PAGER_HELP_KEYS` binds `?` as another `Close`, since a pager's
help level is now a pager over that table.
Amended by
[ADR 0180](0180-tui-the-transcript-viewer-is-a-pager-content.md) (decision 6):
the transcript viewer is on the
pager, so its interim help pager is gone.
Decision 3 generalised by
[ADR 0193](0193-tui-every-modal-footer-is-one-footer-one-look-under-a-blank-row-and-one-fit.md): every framed control fits its footer by
that rule, the spilled rows behind a blank row.

## Context

After CSP-745 the pane viewer was the framework's pager, but three readers
still scrolled on their own (inventory
`docs/plans/tui-framework/inventory/05-search-help-viewers.md`, sections
2.3, 2.4, and 3.2):

- **The keymap help** kept a `HelpScroll` and drew through
  `HelpPage::render`, which split the page from its footer, laid it out a
  cell narrower beside its own scrollbar, and clamped the offset it was
  given.
- **The value modal** kept a `u16` offset, matched its own keys, took the
  window back with `fit`, and drew one footer row in the `inert` color,
  cut by the frame (at 40 columns `y copy` was lost), in table order
  (`Esc/q/o close · y copy`) where a pager puts the keys that leave last.
  It had no scrollbar.
- **Every pager's `?` level** and the transcript viewer's help page kept a
  `HelpScroll` drawn by `HelpPage::render` as well.

CSP-745 left one question open: a framed pager whose footer is taller than
its frame kept the footer's first rows and dropped the rest, which are the
keys that leave, while the help page kept the footer's last rows and
scrolled its first rows with the body.

## Decision

1. **A help page is a pager's content.** `HelpPage<M = ()>` owns its
   sections and title and lays them out on the grid of ADR 0162 decision 4
   for the width its pager gives, keeping the layout while the width is
   the same. It implements `PagerContent` with no keys of its own
   (`NoKeys`) and its title as the header; `M` is the message type of the
   pager it is read in, so a host can put the page on its own stack.
   `HelpPage::pager(table)` reads it in a pager framed at `HelpPage::SIZE`
   (`ModalSize::WIDE`) that resolves `table`, from which the footer is
   generated. `lines(theme, width)` is the layout. `HelpScroll`,
   `HelpPage::render`, `scrolled`, `footer`, and `footer_lines` are
   removed.

2. **A pager's help level is a pager.** `?` opens the page of the pager's
   tables (`help_sections`) in a pager of its own over `PAGER_HELP_KEYS`.
   That pager's `Close` (`Esc`, `?`) goes back to the reader and its
   `Dismiss` (`q`, `Ctrl-C`) leaves it. `PAGER_HELP_KEYS` binds `?` as
   another `Close`, the key that opened the page, so its footer is the
   same (`j/k/PgDn/PgUp scroll · Esc/? back · q close`). `Pager::help_page`
   replaces `help_scroll`.

3. **One footer-fit rule for every framed pager: the help page's.** Under
   its body a framed pager draws the find prompt (or the find chip and the
   note) and then its footer, laid out at the body's text width (a cell
   narrower beside the scrollbar when the body overflows), on the frame's
   last rows, leaving the body at least one row. When they do not fit,
   the prompt or chip row stays, the footer keeps its last rows (the keys
   that leave), and its first rows (the keys that scroll) go after the
   content's last row and scroll with it. No key is lost, and the keys
   that leave are the last to go out of view, the priority the full-screen
   footer keeps when it drops groups. A pager framed at a size that fills
   its extent lays its rows out in the draw only, not in the frame's
   measure.

4. **The value modal is a content.** `ValueModal` is `Pager<ValueText>` in
   `Modal(ModalSize::WIDE)` over `VALUE_MODAL_KEYS` (`PAGER_KEYS`, `o` as
   another close, `y` bound to `PagerKey::Copy`). `ValueText` wraps the
   value at the width and draws a rebuild plan's command lines (`$ …`) in
   `accent`, a hook that stays in the host's layout; its header is the
   label; its `copy` hook returns `Msg::Copy` with the plan's script or the
   text, which the runtime copies and toasts. It binds no find and no help.
   `ValueModalState`'s offset, key handling, and `fit`, `ValueModalOutcome`,
   `ValueModalWidget`, and its `Overlay` impl are removed.

5. **The keymap help is a page in a pager.** `KeymapHelp` is
   `Pager<HelpPage<Msg>>`; `help::open(theme)` builds the hand-written
   sections and the legends in the App's theme when it opens and resolves
   `HELP_KEYS` (`PAGER_KEYS`, and `?` closing as it opened).
   `HelpOverlayState`, `HelpOverlayWidget`, `HelpOutcome`, and its
   `Overlay` impl are removed.

6. **The transcript viewer's help page holds a pager** until the viewer
   itself moves onto the pager (CSP-748): its reducer routes the movement
   messages to the pager's scroll calls, and the draw generates the page
   from the viewer's table the first time, since the viewer's state names
   no program table.

## Consequences

- One scroll implementation sits under every reader: the pager's window,
  clamped on every draw and kept, paging by the rows shown, with `G`
  keeping to the end.
- **Visible changes, all in the value modal** (the CSP-746 summary lists
  them): the footer reads `j/k/PgDn/PgUp scroll · y copy · Esc/q/o close`;
  it takes the help page's `muted` look instead of the `inert` color; on
  a narrow frame it wraps between segments onto a second row instead of
  being cut, so the body is a row shorter there; a value taller than the
  frame gets the scrollbar and wraps a cell narrower beside it; an empty
  value says `nothing to show`. The keymap help's frames are unchanged
  cell for cell, and so are the viewers' help pages.
- `q` and `Ctrl-C` answer `Dismiss` in the value modal and the keymap help
  where they answered `Close`; neither is a step, so they close the same.
- A help page a host adds is `HelpPage::new(sections).pager(table)`; a
  reader whose footer is long cannot lose its leaving keys on a short
  screen.
- Forms keep their footer's first rows and dialogs their last when a
  footer does not fit; that difference outside the pager is follow-up
  work (backlog `CSP-784`).

## Alternatives Considered

- **Keep the framed pager's rule** (keep the footer's first rows, drop the
  rest). Simpler, but on a short screen it drops the keys that leave
  before the keys that scroll, and the help page would lose footer rows it
  shows today.
- **Drop by priority**, as the full-screen footer does (the scroll keys
  first). It keeps the keys that leave in view, but the dropped keys are
  gone; under the chosen rule they are one scroll away.
- **Keep `HelpScroll` for the pager's help level and the transcript
  viewer.** Fewer changes, but two scroll implementations again, one of
  them the help page's own split and clamp.
- **A host content wrapping `HelpPage`** with the host's message, as the
  pager page's Ports sketched. It delegates every content method for no
  behavior of its own; the type parameter says the same.
- **A find in the value modal.** Cheap (the lines that hold the query),
  but it adds `/ find` and `? help` to the footer of a modal opened from
  one field, and nothing asks for it yet.

## Open Questions Answered

- *A framed footer taller than its frame: drop or scroll?* Scroll its
  first rows with the body and keep its last rows, the help page's rule,
  for every framed pager.
- *Does the value modal find?* No; its table binds no find and no help.
- *Is a pager's help level still the help page's own scroll?* No, a pager
  over `PAGER_HELP_KEYS`.
