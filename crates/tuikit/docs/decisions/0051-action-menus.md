# 0051: Menus are one control whose entries are data

| | |
| --- | --- |
| Status | Accepted |
| Page | [controls/action_menu.md](../controls/action_menu.md) |

## Decision

- **Entries are data.** An `Entry<M>` is a label, an optional detail, an
  optional shortcut, whether it can run now, how it confirms (`None` or
  `Inline`), and the host's message. The host builds the entries from its
  state each time the menu opens; the menu reports the picked entry's
  message.
- **One contract and one table.** `ActionMenu<M>` implements `Overlay<M, Cx>`
  for every context and reads none. `MENU_KEYS` resolves first: the arrows,
  `k`/`j`, and `Ctrl-P`/`Ctrl-N` move, wrapping; `Enter` picks; `Esc`
  closes and `q` and `Ctrl-C` dismiss, silently. Any other key is an
  entry's shortcut or is swallowed.
- **An entry that cannot run is shown and stepped over.** It is drawn
  receding; the cursor skips it both ways and starts on the first entry that
  can run; neither `Enter` nor its shortcut commits it. Its detail stays
  plain, because that is where the host says why.
- **An armed entry says what `Enter` will do.** An inline confirmation arms
  on the first press; the row then reads its label, `·`, and `press Enter
  again to confirm` in `error` and bold, and the second press commits.
  Moving disarms; the close keys close, armed or not (house rule 3).
- **Shortcuts sit in a key column** after the cursor gutter, as wide as the
  widest key label, so every label starts in one column. A shortcut moves
  the cursor to its entry and picks it. The table resolves first, so a
  shortcut on a key the table binds never fires; `shadowed_shortcuts` finds
  one.
- **One layout.** In the menu frame: a blank row, each entry's label row and
  its detail wrapped under it, a blank row, and the footer. On a short
  screen the footer keeps the bottom rows and the entries scroll in a
  window the menu keeps, with the shared scrollbar.
- An entry that asks a further question opens a step (record
  [0034](0034-steps-and-flows.md)).

## Rationale

The host had several menu widgets, each with its own keys, cursor, and
confirmation. One control with entries as data gives every menu house rules
1 to 5 at once.

## Consequences

- A host's menu is a function that builds entries; nothing else.

## Alternatives considered

- **Disabled entries the cursor can rest on.** `Enter` would do nothing
  there, a silent refusal (house rule 7).
- **Hiding entries that cannot run.** The host still omits what never
  applies; showing one that applies but not now teaches that it exists.
- **`Esc` disarming before it closes.** One more key to leave, for a state
  the footer does not name.
- **The footer saying `Enter confirm` while armed.** A second place, away
  from the eye.
- **The shortcut after the label or at the row's edge.** After the label it
  reads as part of the name; at the edge the highlight would have to run to
  the edge.
- **Shortcuts resolved before the table.** Movement and close keys would
  stop being the same in every menu.
- **No scrollbar.** The menu would not say that entries are off screen.

## Origin

- host ADR 0171, "Menus are one control whose entries are data": decisions
  1 to 7.
