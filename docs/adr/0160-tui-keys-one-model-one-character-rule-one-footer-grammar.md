# ADR 0160: Overlay Keys Share One Model, One Character Rule, And One Footer Grammar

## Status

Accepted. Amends [ADR 0123](0123-overlay-keymap-tables.md): the key-table
model and the standard tables move to the framework's `keys` module, a
character binding no longer ignores its chords (ADR 0123 kept each key's
old modifier handling), and footers take one grammar. Applies house rules
3, 4, 6, 8, and 12 of [ADR 0151](0151-tui-framework-control-set-and-house-rules.md)
and extends [ADR 0150](0150-tui-framework-workspace-crate.md) with the
framework's `keys` module. Amended by
[ADR 0168](0168-tui-forms-one-ring-submit-from-any-field-fixed-footer.md):
`Space` in `FORM_OPTION_KEYS` binds its own `FormKey::Toggle` under the same
`change` verb, so a row of several checkboxes can tell moving from flipping.
Amended by
[ADR 0172](0172-tui-dialogs-one-control-and-follow-up-questions-are-steps.md):
`DIALOG_KEYS` (`DialogKey`) is the one table of every dialog and replaces
`CONFIRM_KEYS` and `ChoiceKey<C>` (a choice's verbs are the dialog's own
keys, so the host's `CLOSE_DOWN_KEYS` is gone), and a footer can keep some
of a table's actions (`Footer::only`) or take a control's own segments.
Amended by
[ADR 0173](0173-tui-checklists-one-control-select-all-none-and-a-count.md):
`CHECKLIST_KEYS` binds `a` (select all) and `n` (select none), shown in its
footer, and `PgUp`/`PgDn`, `Home`/`g`, and `End`/`G` as aliases. Amended by
[ADR 0174](0174-tui-palettes-one-control-a-query-over-ranked-results.md):
`PALETTE_KEYS` (`PaletteKey`) and `PALETTE_OPTION_KEYS`
(`PaletteOptionKey`) are standard tables, and the picker resolves the
former, so the host's `PICKER_KEYS` is gone. Amended by
[ADR 0176](0176-tui-settings-panels-apply-as-they-change-and-editors-are-steps.md):
`SETTINGS_KEYS` (`SettingsKey`) is the standard settings table, with
`PgUp`/`PgDn`, `Home`/`g`, and `End`/`G` as aliases, and the host's
`CONTROLS_KEYS` is gone. Amended by
[ADR 0177](0177-tui-readers-are-one-pager-over-content-the-host-lays-out.md):
`PagerKey` gains half pages, find, next and previous match, and help;
`PAGER_KEYS` binds `q` to `Abort` and `Ctrl-D`/`Ctrl-U` as aliases, with
its scroll rows in the `PAGER_SCROLL_KEYS` fragment; `READER_KEYS`,
`PAGER_HELP_KEYS`, and `FIND_KEYS` are standard tables, and the host's
`PANE_VIEWER_KEYS` is gone.

## Context

ADR 0123 put every overlay's keys in a `const` table that both resolves
keys and generates the footer. The inventory for the framework extraction
(`docs/plans/tui-framework/inventory/06-interaction-conventions.md`,
sections 1 and 5) found the model sound and its use uneven:

- **The same letter, matched two ways.** Overlay characters used a matcher
  that ignores modifiers, so `Ctrl-G` jumped to the top of the help page,
  the value modal, the messages log, and both viewers (while `Ctrl-G` is
  also the main view's grouping key), `Alt-G` jumped to the bottom of the
  first three but not of the viewers, which matched `G` plain or shifted
  only, `Ctrl-D` discarded a branch in the close-down choice, and `Ctrl-Y`
  confirmed a removal. `y` copied in the messages log on `Alt-Y` but not on
  `Ctrl-Y`.
- **`Ctrl-N` and `Ctrl-P` moved some lists.** The search lists, their
  options, and the picker took them; the menus, Controls, the checklists,
  the messages log, and the forms did not.
- **Three footer dialects.** `Esc close`, `Esc to close` (the help page,
  the value modal, the confirmations), and `m = merge & close`; some
  segments named no verb at all (`Tab current/all`, `Ctrl-O options`,
  `←/→/Space option`).
- **Four ways to write a key.** Footer labels (`Ctrl-R`, `↑`), the label
  function (`Ctrl-r`, `Up`), the viewer help's own conversion, and the
  snapshot key language.
- **Copied rows.** The help page and the value modal repeated twelve
  movement rows each; the three forms repeated their movement rows.
- **Brittle checks.** The modal convention test classified an action by the
  spelling of its variant name, so a renamed variant silently left the
  check; four tables were missing from the label check; nothing checked
  that `Ctrl-C` came first, or that a key a footer names is not taken by an
  earlier row.
- **A domain import in the generic file.** The table module imported three
  of the application's message types for the viewer tables, so the generic
  half could not move as it was.

## Decision

1. **`tuikit::keys` owns the model.** `KeyMatcher`, the const row
   builders, `KeyRow<A>` (`Bind`, `Note`, and `Include`, which splices in
   another table of the same actions), `Hint`, `resolve`, `rows`,
   `Footer`/`footer`/`HINT_SEPARATOR`, `key_label`, `typed_char`, the
   checks (`mislabeled`, `unresolved`, `loose_characters`,
   `ctrl_c_first`), the `KeyRole` classifier with its `KeyAction` trait,
   and the modal key `Convention`. The framework is generic over each
   table's action enum `A` and imports nothing from the application.

2. **Standard tables with framework enums.** `MENU_KEYS` (`MenuKey`),
   `CONFIRM_KEYS` (`ConfirmKey`), `ChoiceKey<C>` for choice dialogs over
   the host's choices, `PAGER_KEYS` (`PagerKey`), `TEXT_INPUT_KEYS`
   (`TextInputKey`), `CHECKLIST_KEYS` (`ChecklistKey`), and `FORM_KEYS`
   with `FORM_OPTION_KEYS` (`FormKey`). The application's own tables stay
   in `tui::keybindings::overlay` (search, the picker, Controls, Messages,
   the close-down choice, the omnibox, the viewers and their find prompt)
   and build on the standard ones where they share rows: the help page and
   the value modal include `PAGER_KEYS` and add the key that opened them,
   and the three forms include `FORM_KEYS` (and `FORM_OPTION_KEYS`) and
   add their own `Enter` verb. No table is a copy of another.

3. **One rule for characters.** In an overlay a character binds as typed:
   alone or with Shift (terminals differ on reporting Shift with `G` or
   `?`), never with Control or Alt. Chords are spelled out (`Ctrl-R`,
   `Alt-b`), and keys that are not characters ignore their modifiers. So
   `g` and `G` (and `Home` and `End`) go to the ends of a pager or log and
   no chord of them does. The const builder for non-character keys refuses
   a character at compile time, and `loose_characters` checks every table.
   The main view's keymap keeps its own matchers until the scoped keymap
   replaces it.

4. **`Ctrl-N` and `Ctrl-P` in every list.** Every list with a cursor moves
   with the arrows, `Ctrl-P`/`Ctrl-N`, and, where nothing is typed, `k`/`j`:
   the menus, Controls, the checklists, the messages log, a form's fields,
   the picker, the search lists and options. Pagers, which have no cursor,
   bind neither chord.

5. **One footer grammar.** A segment is the keys, a space, and a short
   imperative verb with no "to" and no "=" (`Esc close`, `Enter pick`,
   `Tab search current/all`); labels that share a verb join with `/`;
   segments join with ` · `, the only place a footer wraps. `Esc` says what
   leaving costs: `close` where it loses nothing, `cancel` where it drops
   something pending, `back` where it returns to the level below in the
   same overlay. Every label is the key as `key_label` writes it, the one
   notation (`↑`, `Space`, `S-Tab`, `PgDn`, `Ctrl-R`, `Alt-b`).

6. **Roles, not names.** Each action enum implements `KeyAction`, saying
   what each action is for (`Abort`, `Leave`, `Apply`, `Change`, `Move`,
   `Page`, `Other`). The modal key convention (`Esc` leaves, `Enter`
   applies, `Space` changes the focused item) is checked by role, with the
   modals' rows kept as application data.

7. **Availability on a row.** A bound row carries `available` (true unless
   set). `with_availability` copies a table with each row's availability
   from a predicate over its action; `Footer::lines` draws an unavailable
   segment, and the generated help pages an unavailable row, in
   `unavailable_style` (the theme's `inert` color with its `muted`
   modifier). `resolve` ignores availability, so the overlay can say why
   nothing happened rather than let the key fall through. This is the seam
   for show, dim, or hide availability; no overlay sets it yet.

8. **Checks over every table.** A registry test runs `mislabeled`,
   `unresolved` (a footer that names a key an earlier row takes),
   `loose_characters`, and `ctrl_c_first` over every table, and a source
   scan fails when a table is defined without being registered.

## Consequences

- **Keys that stop working** (all chords of a character that the old
  matcher let through): `Ctrl-G` and `Alt-G` (top) and `Alt-G` (bottom) in
  the help page, the value modal, and the messages log; `Ctrl-G` (top) in
  both viewers; `Ctrl-`/`Alt-` with `j`, `k`, `Space`, `q`, `?`, `/`, `n`,
  `t`, `a`, `o`, `!`, `J`, and `K` wherever those letters are bound
  (menus, Controls, the checklist, the search options, the pagers, the
  messages log, the viewers); `Ctrl-Y`/`Alt-Y` copy in the value modal and
  `Alt-Y` in the messages log; `Ctrl-Y`/`Alt-Y` and `Ctrl-N`/`Alt-N` in the
  worktree confirmations; `Ctrl-M`/`Alt-M`, `Ctrl-D`/`Alt-D`, and
  `Ctrl-Q`/`Alt-Q` in the close-down choice; `Ctrl-Space`/`Alt-Space`
  where `Space` is bound.
- **Keys that start working:** `Ctrl-N`/`Ctrl-P` in the row, mux, and
  worktree menus, Clean up, Controls and its checklists, the messages log,
  and the three forms.
- **Footers change wording:** `to scroll`/`to close`/`to copy`/`to
  confirm`/`to cancel` lose their "to"; `m = merge & close` becomes `m
  merge & close`; `Tab current/all` becomes `Tab search current/all`,
  `S-Tab sessions/messages` `S-Tab list sessions/messages`, `Ctrl-R earlier
  search` `Ctrl-R recall search`, `Ctrl-O options` `Ctrl-O set options`,
  `Ctrl-S Search view` `Ctrl-S open Search view`, and `←/→/Space option`
  `←/→/Space change`. The value modal's footer takes the help page's
  order and shows `PgDn`/`PgUp` (`j/k/PgDn/PgUp scroll · Esc/q/o close ·
  y copy`). The content search's footer needs a third row at 100 columns.
- A new overlay table is a const of framework rows over its own enum; the
  compiler, the registry, and the checks hold it to the rules.
- The help pages and the overlay stack read the same tables later: the
  help generator moves into the framework with `HelpPage`, and the scoped
  program-wide keymap is designed on `KeyMatcher` and `KeyRow`.

## Alternatives Considered

- **Characters ignore modifiers except Control** (the main view's rule).
  Alt chords would still fire, keeping `Alt-G` as an accidental end key;
  one plain rule is easier to learn and frees every chord.
- **Keep each table's old matchers** (ADR 0123's choice). No key changes,
  but `Ctrl-G` would keep meaning two things and `Ctrl-D` would keep
  discarding a branch.
- **`Ctrl-N`/`Ctrl-P` only where a text field takes letters**, documented
  per control. It matches readline habits, but a rule that holds in every
  list is the one users can rely on, and the chords were free everywhere
  they were added.
- **Footers ordered by role** (moves, changes, others, apply, leave) so any
  composition reads the same. It would reorder the search footers and needs
  every footer to know roles; composing standard tables with `Include` at
  the start keeps today's orders.
- **Labels derived from the key**, dropping `label` from `Hint::Shown`.
  Simpler rows, but a label is the place a table may write a key its own
  way later (a range such as `1-6`); `mislabeled` keeps the two in step.
- **Availability as a filter in `resolve`.** An unavailable key would fall
  through to a text field and type itself; drawing it dimmed while it
  still resolves leaves the overlay to explain itself (house rule 7).
- **Choice dialogs as a standard table.** Their keys and verbs are the
  choices, so only the enum and the leave rows are standard.

## Open Questions Answered

- *Which rule for characters?* Typed alone or with Shift; never a chord.
- *`Ctrl-N`/`Ctrl-P` everywhere or per control?* Everywhere a list has a
  cursor; never in a pager.
- *Arrows shown or aliased?* Shown (`↑/↓ move`) in every list's footer;
  `k`/`j` and the chords are aliases.
- *Which footer grammar?* Keys and a short imperative verb, no "to".
- *How are actions classified?* By a `KeyRole` each enum declares.
