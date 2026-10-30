# ADR 0176: Settings Panels Are One Control That Applies Each Change As It Is Made, And Their Editors Are Steps

## Status

Accepted. Carries out the `SettingsPanel<M>` control of
[ADR 0151](0151-tui-framework-control-set-and-house-rules.md), applying
its house rules 1, 2, 4, 5, 8, 10, 12, 13, and 14 to the Controls
overlay, and extends [ADR 0150](0150-tui-framework-workspace-crate.md)
with the framework's `settings_panel` module. Amends
[ADR 0031](0031-tui-filter-view-switching-and-per-view-state.md): the
Controls overlay is the framework's settings panel, and its harness,
max-age, mux-state, contents, and PR-state editors open as steps over it
instead of living inside it. Amends
[ADR 0160](0160-tui-keys-one-model-one-character-rule-one-footer-grammar.md):
`SETTINGS_KEYS` (`SettingsKey`) is the standard settings table and
replaces the host's `CONTROLS_KEYS` and `ControlsKey`, with `PgUp`/`PgDn`,
`Home`/`g`, and `End`/`G` added as aliases. Amends
[ADR 0161](0161-tui-overlays-draw-themselves-on-one-host-owned-stack.md)
and [ADR 0172](0172-tui-dialogs-one-control-and-follow-up-questions-are-steps.md):
`Outcome` gains `Back(M)` (close one level, then the host handles `M`),
which builds the seam ADR 0172 left for a step that applies its answer and
returns to the entry under it, and the `Overlay` contract gains
`render_under_step`, with which the stack draws an entry that has a step
of its own open over it. Amends
[ADR 0166](0166-tui-text-prompts-one-overlay-that-explains-its-refusals.md):
`PromptResult` gains `Back(M)`. Amends
[ADR 0164](0164-tui-test-kit-key-scripts-frame-driver-goldens-conformance.md):
a conformance `Case` can be a `step`, whose entry under it may recede its
cursor row outside the step's frame.
Amended by
[ADR 0193](0193-tui-every-modal-footer-is-one-footer-one-look-under-a-blank-row-and-one-fit.md) (decisions 6 and 7): the note comes first in
`SETTINGS_KEYS`, the footer recedes, and the blank row above it stays
while it fits.

## Context

The inventory for the framework extraction
(`docs/plans/tui-framework/inventory/04-menus-pickers-dialogs.md`,
section 3.7) found the Controls overlay to be a settings panel written by
hand, about a quarter of it panel machinery and the rest Conspectus's
settings:

- **One identity, four lists.** A `ControlsCursor` variant per row, a
  `flatten_rows` list of which rows each view shows, an `activate` match
  that built each row's message, a `done` match that sorted rows into
  choices, editors, and buttons by name, and a `body` that drew the rows
  again, recording the cursor's line as it went. Adding a setting touched
  all of them.
- **Editors as modes.** The five editors (three checklists, two prompts)
  were a `SubEditor` held inside the panel: the panel routed keys to
  them, drew them over itself, and checked `Ctrl-C` before an editor saw
  it, so `Ctrl-C` closed both. The CSP-740 rule (a follow-up question is
  a step on the stack: `Esc` closes the step, a commit or `Ctrl-C` the
  whole flow) had replaced the same host check in the worktree menu; the
  panel was the last overlay with levels the stack could not see.
- **A footer that scrolled away.** The footer was drawn as the last lines
  of the body, which scrolled to the cursor, so at 100x30 it was not on
  screen at all; there was no scrollbar, no paging on a panel of about 30
  rows, and rows were clipped at the frame rather than cut with `…`.
- **Apply as you go, stated once.** ADR 0031 and `MODAL_CONVENTION` say
  the panel's changes apply as they are made, so `Esc` keeps them; that
  is the behavior to keep first-class, not an accident of `ApplyAndStay`.
- **A commit that should not close.** The editors' answers apply and
  leave the panel open. Under the step rule a step's `Commit` closes its
  whole flow; ADR 0172 recorded the seam for a step that applies and goes
  back (`Stay`, then the host pops it) without building it.

## Decision

1. **`tuikit::settings_panel` owns the panel.** Its items:
   `SettingsPanel<Id, M, Cx>`, `Section<Id, M>`, `Row<Id, M>`, `Kind`, and
   `SETTINGS_SIZE` (`ModalSize::MENU`). A panel is a title and the host's
   row model, a closure that builds the sections from the host's context
   `&Cx`. The panel calls it on every key and every draw, so a row shows
   the value in force, and keeps only its cursor (the host's row `Id`, so
   it stays on its row when rows around it come and go) and its scroll
   window. It implements `Overlay<M, Cx>`.

2. **Five kinds of row, as data.** A row is the host's id, a `Kind` with
   the state it shows, a label, an optional host message (`None` when
   acting on the row changes nothing), and whether acting on it leaves
   the panel:

   | Kind | Drawn as | `Space` | `Enter` |
   | --- | --- | --- | --- |
   | Choice | the label, bold with `  active` when in force | reports its message and stays | reports it and closes; closes on the choice in force |
   | Checkbox | `[x] label` (bold) or `[ ] label` | reports its message and stays | closes |
   | Editor | name column, value, `[ Edit ]` | asks the host to open the editor | as `Space` |
   | Cycle | name column, value, `[ Cycle ]` | reports its message and stays | closes |
   | Button | the label | reports its message and stays | as `Space` |

   A row that leaves (`Row::leaves`: an editor or a button that hands over
   to an overlay of its own, such as the content search) reports its
   message with `Commit`, which closes the panel.

3. **Changes apply as they are made.** Acting on a row reports the host's
   message at once with `Outcome::Stay`; the host applies it and the next
   draw shows the new value. `Enter` on a choice is `Commit`. `Esc`
   (`Close`), and `q` and `Ctrl-C` (`Dismiss`), close with nothing to
   report and nothing to undo, saying nothing (house rule 5). A staged
   panel (pending changes, *Apply* and *Cancel*) is a seam on the
   control's page, not built.

4. **Editors are steps that step back.** An editor row's message asks the
   host to open its editor; the host builds it from its state and opens it
   as a step over the panel (`ModalStack::push_step`). The editor answers
   with the new `Outcome::Back(M)`: the stack closes the editor only, as
   for `Close`, and the host handles `M`, so the change applies and the
   panel has the keys again. A checklist's `on_confirm` returns it; a
   prompt's returns the new `PromptResult::Back(M)`. Inside an editor
   `Esc` steps back to the panel, and `Ctrl-C` (and `q` in a checklist)
   dismisses the editor and the panel, through the stack's flow rule
   rather than a check in the panel. For an overlay that is no step,
   `Back` is the same as `Commit`.

5. **The entry under a step can recede.** The stack draws an entry whose
   next entry is a step of it with `Overlay::render_under_step`, which is
   `render` unless an overlay overrides it. The panel draws its cursor row
   in `selection_inactive` there, as a list without focus does (house rule
   10), which is how it looked behind its editors before. An overlay
   opened on its own over another is no step of it and leaves it drawn as
   it is. The conformance suite's `Case::step` says a case is a step: its
   `Contained` check accepts a cell outside the step's frame whose only
   change is the theme's selection modifiers.

6. **One table.** `SETTINGS_KEYS` binds `Ctrl-C` and `q` to `Abort`, the
   arrows, `k`/`j`, and `Ctrl-P`/`Ctrl-N` to moving (wrapping, house rule
   2), `PgUp`/`PgDn`, `Home`/`g`, and `End`/`G` to paging and the ends
   (aliases that stop, house rule 12), `Space` to `Change`, `Enter` to
   `Apply`, and `Esc` to `Close`, with the note `changes apply as you make
   them`. Its footer reads `↑/↓ move · Space select/toggle · Enter apply &
   close · Esc close · changes apply as you make them`, the words Controls
   had.

7. **One look.** The menu-sized frame titled with the panel's title;
   section headings bold, a blank line between sections; rows through
   `selection::row`, each cut with `…` in cells; editor and cycle names in
   a name column the host can fix (`name_width`) so values line up on every
   view. The footer, generated from the table under a blank row, keeps the
   bottom rows; the sections scroll in a `ScrollWindow` the panel keeps,
   moving no further than the cursor needs, with the shared scrollbar, and
   moving onto a section's first row brings its heading into view. The
   height comes from the rows.

8. **The host keeps the settings.** Controls is
   `SettingsPanel<ControlsCursor, Msg, App>`, built by
   `controls::panel(App::controls_context)` and opened on the active
   view's row. `ControlsContext`, `ControlsCursor` (the row ids),
   `controls::sections` (which rows each view shows and every `Msg`), the
   filter mappings, and the editors stay in Conspectus: an editor row
   reports `Msg::OpenControlsEditor`, whose reducer arm opens
   `controls::editor` as a step, a `Modal::Checklist` (a new generic step
   modal) or a `Modal::Prompt`. `ControlsOverlayState`,
   `ControlsOverlayWidget`, `ControlsOutcome`, `SubEditor`, the body's line
   counter, `CONTROLS_KEYS`, and `ControlsKey` are gone.

## Consequences

- **Visible changes** (the CSP-744 summary lists them): the panel's keys
  stay at the bottom while its sections scroll (at 100x30 the footer was
  not on screen); a scrollbar shows beside the sections when they do not
  fit; `PgUp`/`PgDn`, `Home`/`End`, and `g`/`G` move the cursor; the
  window scrolls only as far as the cursor needs instead of keeping the
  cursor on its last line, and a section's first row brings its heading
  into view; on a short screen the footer takes its rows, so fewer rows
  show at 40x10 (three, against six with no keys); a row too long for the
  panel ends in `…` instead of being clipped at the border; the footer
  is drawn in the plain style of the menus and checklists, not dimmed;
  leaving an editor clears the status line, as stepping back from the
  worktree menu's steps does. Everything else, the rows, their words and
  order, every message, the accelerators, and the editors' looks, is as
  it was.
- Adding a setting is a `ControlsCursor` variant and a row in
  `controls::sections`; drawing, the cursor, the footer, the height, and
  the key handling follow.
- The conformance suite checks the panel's selection like any list's
  (arrow keys change only the cursor), and its editors as steps of it.
- `Outcome` has six variants; code that matches it exhaustively takes
  `Back` (the two such matches in the host already do).

## Alternatives Considered

- **Editors as levels inside the panel**, the inventory's sketch (a
  `SubEditor` the panel routes to). Self-contained and no contract change,
  but it keeps a second, invisible stack inside one overlay and its own
  `Ctrl-C` rule, which ADR 0172 removed from the worktree menu.
- **`Stay`, then the host pops the step** (the seam ADR 0172 recorded).
  It works with no new outcome, but every host would need a message that
  means "and close the step", and the stack could not say what happened;
  `Back` names it once.
- **A step's `Commit` returns to the entry under it.** Rejected again for
  the reason ADR 0172 gave: a flow that is done would leave its menu open.
- **Staged apply** (changes pending until *Apply*). It would change what
  `Esc` means for every setting and the panel's whole contract; Controls
  has applied as it goes since ADR 0031. Kept as a seam.
- **Telling every covered entry it lost focus.** One rule for all, but an
  unrelated overlay opened over a panel would restyle it, and the layout
  suite's check that a lower modal draws as it does alone would no longer
  hold; the step relation is the one that says a level of the overlay has
  the keys.
- **A trait for the row model**, implemented by the host's context. A
  closure lets one context feed several panels and lets a test feed a
  fixture; the panel needs only one function.
- **A positional cursor.** Simpler, but a row appearing above the cursor
  (another view's rows, a filter that adds a row) would move it to a
  different setting; the host's id keeps it.
- **The dimmed footer** Controls drew. The forms and palettes dim theirs;
  the menus and checklists, whose layout the panel shares, do not. One
  look for every footer is CSP-781's to decide.

## Open Questions Answered

- *Does `Esc` undo what the panel changed?* No; every change is applied
  when it is made, and `Esc` keeps it.
- *What closes when an editor confirms?* The editor only; the panel stays
  open with the new value.
- *Does `q` close the panel?* Yes, and from a checklist editor too; it
  types in the prompts.
- *Are the page keys in the footer?* No; they are aliases, as in the
  checklists.
