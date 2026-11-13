# ADR 0193: Every Modal Footer Is One Footer: One Look Under A Blank Row, And One Fit That Keeps The Keys That Leave

## Status

Accepted. Applies house rule 8 of
[ADR 0151](0151-tui-framework-control-set-and-house-rules.md) to how a
footer looks and fits, where ADR 0160 settled what it says. Extends
[ADR 0158](0158-tui-modals-one-frame-and-named-sizes.md): the frame module
gains `ModalFooter`, and a footer on a frame's bottom border takes the
footer's look. Amends
[ADR 0160](0160-tui-keys-one-model-one-character-rule-one-footer-grammar.md)
decision 5: a footer lists the keys that leave last. Generalises
[ADR 0178](0178-tui-help-pages-and-long-values-are-pager-contents.md)
decision 3 (a framed pager's footer fit) to every framed control, which
amends [ADR 0166](0166-tui-text-prompts-one-overlay-that-explains-its-refusals.md)
decision 5 (the prompt's border footer keeps its last row),
[ADR 0168](0168-tui-forms-one-ring-submit-from-any-field-fixed-footer.md)
decision 5 (a form keeps its footer's last rows, not its first),
[ADR 0171](0171-tui-menus-one-control-entries-as-data.md) decision 7,
[ADR 0172](0172-tui-dialogs-one-control-and-follow-up-questions-are-steps.md)
decision 4 (a dialog's body keeps a row, and the blank row above its keys
stays while they fit),
[ADR 0173](0173-tui-checklists-one-control-select-all-none-and-a-count.md)
decision 6,
[ADR 0174](0174-tui-palettes-one-control-a-query-over-ranked-results.md)
decision 6 (a palette's footer has no four-row cap and keeps its last
rows),
[ADR 0176](0176-tui-settings-panels-apply-as-they-change-and-editors-are-steps.md)
decisions 6 and 7 (the panel's note comes first, and its keys recede), and
[ADR 0179](0179-tui-log-lists-read-the-hosts-log-over-an-embedded-pager.md)
decision 5. Carried out by CSP-781 (the look) and CSP-784 (the fit).

## Context

After the control extractions every footer was generated from a key table
in one grammar, but drawn and fitted by each control's own code:

- **Two looks.** Menus, dialogs, checklists, and the settings panel drew
  their keys plain under a blank row. Forms, palettes, framed pagers (the
  help pages, the value modal), and the log list drew them receding in
  the theme's `muted`, the forms under a blank row and the others with
  none. The prompt wrote its keys on its bottom border in the border's
  color. The status bar draws its hints `muted`.
- **Three blank-row rules.** A menu kept the blank row above its keys
  while the keys and it fit the rows its entries left. Dialogs,
  checklists, and the settings panel gave it up whenever their body did
  not fit, so at 100x30 Controls, whose sections scroll, had none.
  Palettes, pagers, and the log list had no blank row.
- **Four overflow rules.** When a footer had more rows than its frame
  left it, framed pagers and the log list kept its last rows (the keys
  that leave) and scrolled its first rows with the body (ADR 0178);
  menus, dialogs, checklists, and the settings panel kept its last rows
  and dropped the rest; forms kept its first rows; palettes kept its
  first four rows; the prompt kept its first row. On a short screen the
  same footer lost different keys in different modals.
- **Keys that leave in the middle.** Keeping the last rows keeps the keys
  that leave only where a footer lists them last. The standard tables
  and the pager's and log list's compositions do; quick select and
  content search named `Esc close` third, and the settings panel ended
  with its note.

## Decision

1. **One footer.** `tuikit::frame::ModalFooter` draws every modal's
   footer: a `Footer` of segments, the line a modal keeps above its keys
   (a form's error line, a reader's find prompt or find chip), and the
   blank row that separates them from the body. `keys(width)` lays the
   keys out, broken only between segments; `height(width)` is the rows it
   asks for; `fit(width, room)` and `fit_beside(area, keep, body_rows)`
   fit it to the frame as a `FooterFit` (the rows kept, the rows that
   spill, the width); `border(width)` is the line for a frame's bottom
   border. The action menu, dialog, checklist, settings panel, form
   (`FormBody` takes a `ModalFooter`), palette, framed pager, log list,
   and prompt all go through it; `form::footer_lines` is removed.

2. **One look: receding, under a blank row.** Every segment takes the
   theme's `muted` and no color of its own, so it shows the color of
   where it sits: the body's foreground, or the accent on a bottom
   border. A segment that cannot act now keeps `inert` with `muted`; the
   status line keeps its own style. The keys sit on the frame's last
   rows under a blank row, with the status line between the two. The
   status bar names keys in the same look, so a key reads the same in a
   footer and on the bar, and a footer stands apart from a menu's
   entries and their shortcut column, which are plain.

3. **The blank row gives way first, and only when the footer does not
   fit.** It stays while the footer and it fit the rows the body leaves,
   the menus' rule, so every modal shows it at every size the footer
   fits, a list that scrolls and a reader that fills its frame included.

4. **One fit: the keys that leave stay.** The footer leaves the body at
   least one row (for a palette, the query, the status row, and one row
   of the list). When it wants more rows than that leaves, the blank row
   goes first; then the status line takes a row while there is one, the
   keys keep their last rows, and their first rows spill: behind a blank
   row, after the body's last row, where they scroll with it. No key is
   lost, and the keys that leave are the last to go out of view. When
   the body and the spilled rows overflow beside a scrollbar, the footer
   is laid out a cell narrower so the rows line up beside it. A footer
   spills only when the body is down to the rows it keeps, so a list
   whose window follows its cursor never shows the spilled rows; a body
   that scrolls freely (a reader, a dialog's body, the log's detail)
   brings them into view. A footer on a bottom border keeps its last row.
   A frame eight cells wide or narrower keeps a footer on one row, cut at
   its edge.

5. **A footer lists the keys that leave last** (amending ADR 0160's
   grammar). `Footer::leaving_last(table)` composes a table's footer that
   way: its other rows and its notes in table order, then the rows that
   leave or dismiss. Every framework control draws its table's footer
   through it, as the pager and the log list already composed theirs, so
   a host table that includes a standard one and adds rows after it (a
   palette's scope key after `PALETTE_KEYS`) still ends with `Esc close`.
   The settings panel's note moves to the front of `SETTINGS_KEYS`, where
   it wraps on a row of its own at 40 columns and leaves `Enter apply &
   close · Esc close` on the last row, and quick select's and content
   search's tables list `Esc close` last, so each table reads as its
   footer does. The status bar names the same footer and gives way from
   its end; there a note, which names no key, holds on a step below the
   keys (`KeyHintBar::footer`), so the panel's leading note gives way
   before them.

6. **The conformance suite holds it.** The `Footer` check reads a framed
   modal at 200x50: every segment in the footer's look (the theme's
   `muted`, regular weight, the color of where it sits), on the frame's
   last rows under a blank row (or on its bottom border), and the last
   segment of its last row a key that leaves. At 40x10 the frame's last
   row (the last inner row or the bottom border) must still name a key
   that leaves, so a control that keeps a footer's first rows fails.

## Consequences

- Visible changes (the CSP-781 and CSP-784 summaries list them by
  modal): the menus', dialogs', checklists', and Controls' keys recede;
  Controls, the help pages, the value modal, the palettes (quick select,
  content search, the picker), and Messages gain a blank row above their
  keys, so their bodies are a row shorter; dialogs, checklists, and
  Controls keep that row while their body scrolls on a short screen,
  showing one row of it fewer there; the prompt's border keys recede in
  the accent and, on a screen too narrow for both, show `Esc cancel`
  instead of `Enter confirm`; a form or a palette whose footer does not
  fit keeps its last rows, the keys that leave, where it kept its first;
  a dialog's body keeps a row on the shortest screens, where its keys
  took every row; Controls reads `changes apply as you make them · ↑/↓
  move · …`, and quick select and content search end with `Esc close`,
  so on a status bar too narrow for all of their keys `Esc close` is the
  first to give way there (the footer above it still names it).
- A new control, or a host's own modal, draws its footer with
  `ModalFooter` and gets the look, the blank row, and the fit; the
  conformance suite fails one that hand-writes a footer in another look,
  floats it above the last rows, drops its blank row, keeps its first
  rows, or names the keys that leave first. Such a modal takes its
  table's footer from `Footer::leaving_last`.
- The full-screen pager's one-row footer (the transcript and scrollback
  viewers) is the screen's chrome, not a modal footer: it keeps its
  `inert` look and its drop-by-priority rule (ADR 0177), which also keeps
  the keys that leave.

## Alternatives Considered

- **The plain look under a blank row** (the menus'). It changes fewer
  menus, but a plain footer reads like one more entry or paragraph, and
  the status bar, the forms, the readers, and the palettes would all
  change instead.
- **Receding with no blank row** (the palettes' and readers'). One row
  more for the body, but a cursor row's highlight runs straight into the
  keys, and terminals that draw `muted` (dim) as nothing lose the
  separation altogether.
- **The blank row gives way whenever the body does not fit** (the
  dialogs'). It keeps a row for the body on a short screen, but a list
  that scrolls or a reader that fills its frame would never show the
  row, so the look would depend on how long the content is: the
  inconsistency Controls showed at 100x30.
- **Keep the footer's first rows** (the forms' and palettes'). It drops
  the keys that leave first.
- **Drop the first rows in a list and scroll them only in a reader.** It
  draws the same, since a list's spilled rows never show, but it would
  be two rules to state and to test.
- **Order every footer by the full role order** (moves, changes, others,
  apply, leave), which ADR 0160 set aside. Only the place of the keys that
  leave decides what a short frame keeps, so only they move; the rest
  keep their table's order.
- **Leave the order to each table.** A table that includes a standard one
  cannot list its own rows before that table's `Esc`, since `Ctrl-C` must
  come first; composing the order in the footer works for any table.
- **Keep the palette's four-row cap.** A long footer would never crowd the
  results, but it is a second limit beside the body's one row, and it cut
  the keys that leave.

## Open Questions Answered

- *Plain or receding?* Receding, in the theme's `muted`, as the status bar
  draws hints.
- *Blank row above or not?* Above, giving way first and only when the
  footer does not fit.
- *Which rows of a footer too tall for its frame stay?* Its last, which
  hold the keys that leave; its first rows scroll after the body.
- *Does a dialog's footer still take a frame's only row?* No; the body
  keeps one in every framed control.
