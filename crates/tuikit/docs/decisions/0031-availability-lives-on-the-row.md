# 0031: Availability lives on the row, and resolving a key ignores it

| | |
| --- | --- |
| Status | Accepted |
| Page | [controls/keys.md](../controls/keys.md) |

## Decision

- A bound row carries `available` (true unless set); `with_availability`
  copies a table with each row's availability from a predicate over its
  action. A keymap binding carries an optional predicate over the host's
  context (`available_when`).
- `resolve` ignores availability: a key that cannot act now still reaches
  its control, which can say why nothing happened.
- A footer draws an unavailable segment, and a generated help page an
  unavailable row, in `keys::unavailable_style` (`inert` with `muted`). The
  status bar, which names the keys that act now, leaves an unavailable
  keymap binding out; a host's annotation can still show a key dimmed with
  its reason (record [0066](0066-the-status-bar.md)).

## Rationale

A key that falls through when it cannot act gives no answer, and a
footer that hides it leaves the user wondering where it went. Keeping
availability on the row gives one place to show, dim, or hide it, and the
control the last word on what to say.

## Consequences

- Menu entries and checklist items that cannot act are shown receding and
  stepped over (records [0051](0051-action-menus.md) and
  [0053](0053-checklists.md)).

## Alternatives considered

- **Availability as a filter in `resolve`.** An unavailable key would fall
  through to a text field and type itself; drawing it dimmed while it still
  resolves leaves the control to explain itself (house rule 7).
- **Dimming unavailable keys on the status bar too.** Consistent with
  footers, but the bar names what acts now and has little room.

## Origin

- host ADR 0160, "Overlay keys share one model, one character rule, and one
  footer grammar": decision 7.
- host ADR 0192, "The main screen's keys are one scoped keymap, and the help
  page is generated from it": decision 5.
