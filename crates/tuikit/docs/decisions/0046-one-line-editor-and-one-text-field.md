# 0046: One line editor over tui-input, and one text field that draws it

| | |
| --- | --- |
| Status | Accepted |
| Page | [controls/text_edit.md](../controls/text_edit.md) |

## Decision

- **`LineEdit` is the only edit engine.** It wraps `tui-input`'s `Input` and
  drives it with requests, so the engine still owns graphemes, word
  boundaries, and the kill buffer. Keys resolve through `LINE_EDIT_KEYS`,
  readline's keys with Meta mapped to Alt: `←`/`→` (`Ctrl-B`/`Ctrl-F`),
  `Home`/`End` (`Ctrl-A`/`Ctrl-E`), `Backspace` (`Ctrl-H`), `Delete`, word
  motion on `Alt-b`/`Alt-f`, `Ctrl-←`/`Ctrl-→`, and `Alt-←`/`Alt-→`,
  `Ctrl-W` and `Alt-Backspace` (the word before), `Alt-d` and
  `Ctrl-Delete` (the word after), `Ctrl-U` (the line), `Ctrl-K` (to the
  end), and `Ctrl-Y` (yank). A character typed alone, with Shift, or with
  Control and Alt together (AltGr) inserts. The editor has no `Enter`,
  `Esc`, or `Tab`: those are its host control's.
- **An edit says what it did.** `handle` returns `Edit { changed, consumed
  }`, and `EditKey::of` names the edit a key asks for, so a host that turns
  keys into messages carries the edit as a value.
- **Paste is one rule.** `single_line_paste` flattens a paste into one line
  (line breaks at the ends dropped, each inner run of them one space, a tab
  a space, other control characters dropped), and `LineEdit::paste` types
  it (house rule 15).
- **`TextField` draws a line editor in one row.** The window starts at the
  line's start while the text before the cursor and the cell under it fit,
  and otherwise at the first grapheme that lets them fit; a wide character
  never straddles the edge, and one wider than the field draws as `…`. The
  caret is the cell under the cursor in `field_cursor` while the field has
  focus, and none without it. Options: `<` and `>` marks in two-cell slots
  at the ends, a prefix that never scrolls, a placeholder for an empty
  value, and a fill to the full width.

## Rationale

The host had three editors with three windowing rules and word keys that
did nothing (the engine binds Meta, terminals report Alt). One engine and
one field make every text entry edit, paste, and scroll the same.

## Consequences

- Every control with a text field (prompt, path field, form, palette, a
  reader's find) edits with `LineEdit` and draws with `TextField`.
- A whitespace-only value is text, not empty.

## Alternatives considered

- **The engine's own key handler with Alt rows in each host table.** The
  editing keys would be spread over tables that mean other things, and a
  message-driven host could not carry an edit as a value.
- **A separate editor for find prompts.** Two engines keep two key sets.
- **A stateful window that scrolls by minimal motion.** Steadier, but the
  renderer keeps nothing; it can be added behind the same API.
- **The terminal's own cursor.** Only one per frame, invisible in goldens,
  and a form shows several fields; the position is a seam.
- **Reverse video for the caret.** No raw modifiers (house rule 13).

## Origin

- host ADR 0030, "A text-input primitive": one shared single-line primitive
  over `tui-input`, rather than an editor per caller.
- host ADR 0165, "Text entry has one line editor and one text field":
  decisions 1 to 6.
