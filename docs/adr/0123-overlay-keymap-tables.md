# ADR 0123: Overlay Keymap Tables

## Status

Accepted. Extends ADR 0085 contract 3 (modal stack and one `Overlay`
contract). Amended by
[ADR 0160](0160-tui-keys-one-model-one-character-rule-one-footer-grammar.md):
the model and the standard tables are the framework's `tuikit::keys`, a
character binding no longer fires on its Control or Alt chords (this ADR
kept each key's old modifier handling), every list moves with
`Ctrl-N`/`Ctrl-P`, and footers take one grammar with no "to".

## Context

Global keys resolve through one declarative table
(`tui::keybindings::KEYBINDINGS`) that both the dispatcher and the
help overlay read, with a drift test between them. Keys inside
overlays did not. Each overlay widget matched `KeyCode` by hand, and
its footer hint was a hand-written string such as
`"Enter/y to confirm · Esc/n to cancel"`. Nothing tied the footer to
the handler, so a footer could advertise a key its overlay ignored, or
leave out one it accepted. The same strings were also copied into
status-bar hints when an overlay opened.

The TUI relies on these footers and menus for discoverability rather
than on memorized keys, so a footer that lies is a usability bug, not
just a style one.

## Decision

Every overlay resolves its keys through a `const` table in
`tui::keybindings::overlay`, and generates its footer from the same
table.

- A table is a slice of `KeyRow<A>`: `Bind { key, action, hint }` or
  `Note(text)`. `A` is the overlay's own action enum. Overlays with the
  same keys share a table (the Controls, pins, mux, and worktree
  menus share `MENU_KEYS`); overlays that differ only in footer wording
  share an enum and keep separate tables (pin create says
  "Enter create", pin edit "Enter save").
- `resolve(table, event)` returns the action of the first matching
  row. The handler matches on that action exhaustively, so adding a
  row whose action the handler ignores fails to compile.
- `footer(table)` builds the hint: rows with `Hint::Shown { label,
  verb }` that share a verb join into one segment (`↑/↓ move`),
  segments join with ` · `, and `Note` rows contribute their text in
  place ("type to edit"). `Hint::Alias` rows are bound but left out on
  purpose: vim-style `j`/`k`, Ctrl-C, alternate close keys.
- Ctrl-C is bound first in every table, to the overlay's `Abort`
  action. Each handler decides what Abort means; in Controls and the
  pins menu it closes the whole overlay even while a sub-editor is open,
  which is why it is a separate action from Esc.
- Keys no row binds fall through to the overlay's text field, if it has
  one. A key that means different things in different fields resolves
  to an action and the handler decides: Space resolves to "next
  option", which toggles an option field and types a space in a text
  field.
- Matchers keep the keys' existing behavior. Code-only matches use
  `KeyMatcher::AnyMod`, as the hand-written `match event.code` arms
  did; modifier-checked keys use `Exact` or `UpperChar`.

The transcript viewer's keys resolve through `VIEWER_KEYS` too, but its
footer stays viewer-owned. The viewer draws state chips (thinking
on/off, tool detail level) behind the ADR 0052 extraction boundary,
which keeps `src/viewer/` free of TUI imports, so all of its rows are
aliases.

Two guards keep this in place. `tests/overlay_keymap_hygiene.rs` fails
when overlay code outside test modules matches `KeyCode` or contains a
key-hint-shaped string literal. A unit test checks that every shown
label names the key its row binds (`↑` for Up, `S-Tab` for BackTab).

## Consequences

- Adding an overlay key is one table row. Its footer entry, its status
  hint, and its handler arm follow from that row, and the compiler
  points at the handler.
- Footers read as before. The one normalization is the help overlay's,
  now "j/k/PgDn/PgUp to scroll · Esc/q/? to close".
- Ctrl-C now backs out of the worktree menu's list and confirms, which
  ignored it before; every overlay treats it the same way.
- Tables live in one module, which grows with each overlay. Splitting
  it by overlay family is cheap if it gets unwieldy.
- The command palette's action registry (backlog `CSP-391`) covers
  global commands. It can read these tables when it needs overlay keys.

## Alternatives Considered

- **One shared action enum for every overlay.** Fewer types, but each
  handler would need a catch-all arm for actions it never sees, which
  gives up the compile-time check that every bound key is handled.
- **Rows inside the existing `KEYBINDINGS` table with an overlay
  mode.** One table, but its rows produce the global `Action`, so each
  overlay would map global actions back to its own meaning. Separate
  typed tables in one module keep a single place to look without that
  indirection.
- **Keep hand-written footers and add a drift test only.** The test
  would have to parse each footer and know each handler's keys, which
  duplicates the table the handler should have been using.
- **Generate the viewer footer from the table as well.** It would mean
  passing TUI-built hint text into `src/viewer/` or importing TUI code
  there, against ADR 0052's extraction boundary, for a footer that is
  mostly state chips rather than key hints.
