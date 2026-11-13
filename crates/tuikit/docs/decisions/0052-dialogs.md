# 0052: Dialogs are one control in three forms

| | |
| --- | --- |
| Status | Accepted |
| Page | [controls/dialog.md](../controls/dialog.md) |

## Decision

- **Three forms.** A `Dialog<M>` is a title, body lines (each wrapped; an
  empty line is a blank row), an optional warning list (a heading and
  items), and a `Kind`: `Confirm(M)`, which commits the host's message;
  `Choice(Vec<Verb<M>>)`, whose named verbs each commit one on their own
  key; or `Notice`, which commits nothing. It implements `Overlay<M, Cx>`
  for every context and reads none; its answers are data, so it is `Clone`
  and `PartialEq` when `M` is.
- **One table for every form.** `DIALOG_KEYS` binds `Ctrl-C` and `q` to
  dismiss, the arrows and `k`/`j` to scroll an overflowing body, `Enter`
  and `y` to confirm, and `Esc` and `n` to cancel. A notice closes on either.
  A choice commits a verb on the verb's key, resolved after the table, and
  has no default, so `Enter` and `y` do nothing in it; `shadowed_verbs`
  finds a verb on a key the table takes.
- **One generated footer per form:** `Enter/y confirm · Esc/n cancel`; each
  verb and then cancel; or `Enter/Esc close` for a notice, since leaving a
  notice loses nothing.
- **One look.** The menu-sized frame with the title on its top border; a
  blank row, the body, a blank row and the warning list (the warning mark
  and heading in `warning` and bold, each item after `-`, wrapped under its
  first word), a blank row, and the footer. On a short screen the blank
  rows give way first, the footer keeps the bottom rows, and the body
  scrolls in a window the dialog keeps, with the shared scrollbar.
- **Which idiom confirms** (house rule 3): an inline second `Enter` on a
  menu entry when there is nothing more to say, a dialog when a body helps,
  and nothing for an action that can be undone.

## Rationale

The host had four confirmation idioms and three tables for one shape. One
dialog with one table, and footers composed from it, keeps one set of keys.

## Consequences

- A menu entry opens a dialog as a step of the menu (record
  [0034](0034-steps-and-flows.md)).

## Alternatives considered

- **Three standard tables, one per form.** Each reads right alone, but a
  choice's keys are its verbs, and the three would drift.
- **A default verb in a choice.** It would put a destructive verb two
  `Enter`s from a list; kept as a seam.
- **A cursor over a choice's verbs.** A menu inside a dialog; the verbs'
  keys answer it.
- **The compact frame.** A dialog over a menu would leave the menu's border
  and cursor showing beside it.
- **Titles that name the action.** They repeat the body's first line; the
  flow's title is what it had.

## Origin

- host ADR 0172, "Dialogs are one control, and a menu's questions are steps
  on the stack": decisions 1 to 4, and its alternatives.
- host ADR 0151, "The framework control set and its house rules": house rule
  3 and the open question it answered (two confirmation idioms).
