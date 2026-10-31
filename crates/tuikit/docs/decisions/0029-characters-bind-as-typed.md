# 0029: A character binds as typed, and every list moves with Ctrl-N and Ctrl-P

| | |
| --- | --- |
| Status | Accepted |
| Page | [controls/keys.md](../controls/keys.md) |

## Decision

- **One rule for characters.** A character binding fires on the character
  alone or with Shift (terminals differ on reporting Shift with `G` or
  `?`), never with Control, Alt, or another modifier. A chord is a binding
  of its own, spelled out (`Ctrl-R`, `Alt-b`). Keys that are not characters
  ignore their modifiers unless the binding names them.
- **Checked.** The `const` builder for non-character keys refuses a
  character at compile time. `keys::loose_characters` finds a table row that
  binds a character but also fires on its chords, and
  `Keymap::loose_characters` does the same for a keymap, by binding id. The
  keymap check reports and does not refuse: a host may keep a rule of its
  own for its main screen and test it.
- **One way to move a list.** Every list with a cursor moves with the
  arrows, `Ctrl-P` and `Ctrl-N`, and `k` and `j` where nothing is typed.
  Pagers, which have no cursor, bind neither chord.

## Rationale

A letter that also fires on its chords takes keys a later binding needs,
and a user who presses `Ctrl-G` is surprised to go to the end. One rule for
every overlay and the main screen is one rule to learn. `Ctrl-N` and
`Ctrl-P` work in lists where letters type.

## Consequences

- A free chord stays free, room for a later binding or a user's remap.

## Alternatives considered

- **Characters ignore every modifier but Control.** Alt chords would still
  fire, keeping an accidental end key; one plain rule is easier to learn
  and frees every chord.
- **Each table keeps the matchers it had.** No key changes, but one chord
  would keep meaning two things.
- **`Ctrl-N` and `Ctrl-P` only where a text field takes letters.** It
  matches readline habits, but a rule that holds in every list is the one
  users can rely on.
- **Refusing loose characters when a keymap is built.** One rule enforced,
  but a host could not keep its own.

## Origin

- host ADR 0160, "Overlay keys share one model, one character rule, and one
  footer grammar": decisions 3 and 4.
- host ADR 0197, "The main screen's characters bind as typed, as the
  overlays' do": decision 3 (the keymap's check), and the rule's reach to a
  program's main screen.
