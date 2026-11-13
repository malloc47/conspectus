# 0028: Keys resolve through tables, and footers and help are generated from them

| | |
| --- | --- |
| Status | Accepted |
| Page | [controls/keys.md](../controls/keys.md) |

## Decision

- **One model.** `tuikit::keys` owns `KeyMatcher` and the `const` row
  builders, `KeyRow<A>` (`Bind`, `Note`, and `Include`, which splices in
  another table of the same actions), `Hint` (shown with a label and a verb,
  or an alias that is bound but left out of footers), `resolve` (the first
  matching row's action), footers, `key_label`, and the checks. The
  framework is generic over each table's action enum `A`.
- **Handlers match on actions.** A control resolves a key to its own action
  and matches the action exhaustively, so a row whose action the handler
  ignores fails to compile. Control code never matches raw key codes; the
  model is the one place that names them. A key no row binds falls through
  to the control's text field, if it has one.
- **Standard tables.** Menus, dialogs, pagers, text input, checklists, log
  lists, palettes and their options, settings panels, forms, and path fields
  each have a standard table over a framework enum. A host's table builds on
  them with `Include` rather than copying rows. `Ctrl-C` is bound first in
  every overlay's table.
- **Roles.** Each action enum implements `KeyAction`, saying what each
  action is for (abort, leave, apply, change, move, page, other); the modal
  convention (`Esc` leaves, `Enter` applies, `Space` changes the focused
  item) is checked by role.
- **Checks over every table.** `mislabeled` (a label that does not name the
  key its row binds), `unresolved` (a footer naming a key an earlier row
  takes), `loose_characters`, and `ctrl_c_first` run over every table a
  control or host defines.

## Rationale

Footers written by hand lied: one advertised a key its overlay ignored,
another left out one it accepted, and the strings were copied into status
hints. Programs that rely on footers for discoverability turn a lying footer
into a usability bug. Generating footers and help from the table the handler
resolves makes them unable to disagree (house rule 8).

## Consequences

- A host scans its own overlay code for raw key matches and hand-written
  hint strings.
- The footer's grammar is record [0030](0030-one-footer-grammar.md), the
  help page record [0035](0035-generated-help-pages.md).

## Alternatives considered

- **Labels derived from the key**, dropping the label from a shown hint.
  Simpler rows, but the label is where a table may write a key its own way
  (a range such as `1-6`); `mislabeled` keeps the two in step.
- **Footers ordered by role.** Any composition would read alike, but every
  footer would have to know roles; including standard tables keeps each
  table's order, and only the keys that leave are placed (record
  [0030](0030-one-footer-grammar.md)).

## Origin

- host ADR 0123, "Overlay keymap tables": a table per overlay, footers
  generated from it, `Ctrl-C` first, the fall-through to a text field, and
  the guard on raw key matches.
- host ADR 0160, "Overlay keys share one model, one character rule, and one
  footer grammar": decisions 1, 2, 6, and 8.
