# 0030: One footer grammar, one key notation, and the keys that leave last

| | |
| --- | --- |
| Status | Accepted |
| Page | [controls/keys.md](../controls/keys.md) |

## Decision

- **A segment** is the keys, a space, and a short imperative verb with no
  "to" and no "=" (`Esc close`, `Enter pick`, `Tab search current/all`).
  Labels that share a verb join with `/`; segments join with ` · `, the only
  place a footer wraps.
- **`Esc` says what leaving costs:** `close` where it loses nothing,
  `cancel` where it drops something pending, `back` where it returns to the
  level below in the same overlay.
- **One notation.** Every label is the key as `key_label` writes it (`↑`,
  `Space`, `S-Tab`, `PgDn`, `Ctrl-R`, `Alt-b`).
- **The keys that leave come last.** `Footer::leaving_last(table)` lists a
  table's other rows and notes in table order, then the rows that leave or
  dismiss, so a host table that includes a standard one and adds rows after
  it still ends with its close key.
- A footer may show only some of a table's actions (`Footer::only`) or be
  composed of a control's own segments.

## Rationale

Footers read differently from overlay to overlay ("Enter/y to confirm",
"Esc=cancel"). One grammar reads as one program. The end of a footer is
where a user looks for the way out, and a footer that is cut keeps it.

## Consequences

- A table's rows are written in the order the footer reads, and the status
  bar names the same footers (record [0066](0066-the-status-bar.md)).

## Alternatives considered

- **Ordering every footer by the full role order** (moves, changes, others,
  apply, leave). Only the place of the keys that leave decides what a short
  frame keeps, so only they move; the rest keep their table's order.
- **Leaving the order to each table.** A table that includes a standard one
  cannot list its own rows before that table's close key, since `Ctrl-C`
  must come first; composing the order in the footer works for any table.

## Origin

- host ADR 0160, "Overlay keys share one model, one character rule, and one
  footer grammar": decision 5.
- host ADR 0193, "Every modal footer is one footer: one look under a blank
  row, and one fit that keeps the keys that leave": decision 5.
- host ADR 0172, "Dialogs are one control, and a menu's questions are steps
  on the stack": decision 3 (footers composed of segments, and a footer of
  some of a table's actions).
